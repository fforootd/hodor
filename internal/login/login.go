// Package login provides API handlers for the login flow.
// These handlers support the <zitadel-login> web component.
package login

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/captcha"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/loginflow"
	"github.com/zitadel/zitadel/internal/notify"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/risk"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// Handler provides login-flow API endpoints.
type Handler struct {
	db             *database.DB
	passwords      *auth.Passwords
	api            SessionCreator
	notifier       *notify.Service
	baseURL        string
	flows          *FlowStore
	cookies        *session.CookieConfig
	captchaHMACKey string
	captchaHTTP    *http.Client
	resolver       *loginflow.Resolver
	risk           risk.Evaluator
}

// New creates a new login API handler.
func New(db *database.DB, passwords *auth.Passwords, restAPI SessionCreator, cookies *session.CookieConfig, resolver *loginflow.Resolver, notifier *notify.Service, baseURL string) *Handler {
	// Generate a random HMAC key for Altcha PoW challenges.
	// In production, this should come from config/secrets.
	hmacKey, _ := captcha.GenerateHMACKey()

	return &Handler{
		db:             db,
		passwords:      passwords,
		api:            restAPI,
		notifier:       notifier,
		baseURL:        baseURL,
		flows:          NewFlowStore(),
		cookies:        cookies,
		captchaHMACKey: hmacKey,
		captchaHTTP:    &http.Client{Timeout: 10 * time.Second},
		resolver:       resolver,
		risk:           risk.NewEvaluator(db.SQL()),
	}
}

// Register mounts the login API routes onto the given mux.
// ADR-019: All login UI is driven by the flow API. Legacy routes removed.
func (h *Handler) Register(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/branding", h.handleBranding)
	mux.HandleFunc("GET /v1/auth/settings", h.handleAuthSettings)

	// Flow API (schema-driven) — the sole interface for login UI.
	mux.HandleFunc("POST /v1/login/flows", h.handleFlowCreate)
	mux.HandleFunc("GET /v1/login/flows/{id}/captcha/challenge", h.handleFlowCaptchaChallenge)
	mux.HandleFunc("POST /v1/login/flows/", h.handleFlowSubmit)
	mux.HandleFunc("GET /v1/login/flows/", h.handleFlowGet)

	// Magic Link (verification endpoint — used by email links).
	mux.HandleFunc("POST /v1/auth/magic-link", h.handleMagicLinkRequest)
	mux.HandleFunc("GET /v1/auth/magic-link/verify", h.handleMagicLinkVerify)

	// SSO / OIDC
	h.RegisterSSORoutes(mux)
}

// --- Branding (schema-driven) ---

func (h *Handler) handleBranding(w http.ResponseWriter, r *http.Request) {
	cfg := h.getResolvedConfig(r, r.URL.Query().Get("flow"))
	b := cfg.Branding
	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"org_id":                "",
		"org_name":              b.OrgName,
		"logo_url":              b.LogoURL,
		"heading":               b.Heading,
		"description":           b.Description,
		"colors":                b.Colors,
		"font_family":           b.FontFamily,
		"font_url":              b.FontURL,
		"texts":                 b.Texts,
		"custom_css":            b.CustomCSS,
		"hide_zitadel_branding": b.HideZitadel,
	})
}

// --- Auth Settings (schema-driven) ---

func (h *Handler) handleAuthSettings(w http.ResponseWriter, r *http.Request) {
	cfg := h.getResolvedConfig(r, r.URL.Query().Get("flow"))
	ssoProviders := h.loadSSOProviders(r, cfg)

	// Build auth_methods from schema config.
	authMethods := make(map[string]any)
	for name, m := range cfg.AuthMethods {
		entry := map[string]any{"enabled": m.Enabled, "position": m.Position}
		if m.Preferred {
			entry["preferred"] = true
		}
		authMethods[name] = entry
	}
	// Inject SSO providers into auth_methods.
	if ssoEntry, ok := authMethods["sso"]; ok {
		if ssoMap, ok := ssoEntry.(map[string]any); ok {
			ssoMap["providers"] = ssoProviders
			if len(ssoProviders) == 0 {
				ssoMap["enabled"] = false
			}
		}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"strategy":             cfg.Login.Strategy,
		"auth_methods":         authMethods,
		"mfa_required":         cfg.Login.MFARequired,
		"registration_allowed": cfg.Login.RegistrationAllowed,
		"identifier_fields":    cfg.Identifiers,
	})
}

// loadSSOProviders reads enabled SSO providers from the dedicated providers table.
func (h *Handler) loadSSOProviders(r *http.Request, cfg *SchemaAuthConfig) []map[string]any {
	repo := providers.NewRepository(h.db.SQL())
	list, err := repo.ListEnabled(r.Context())
	if err != nil {
		return []map[string]any{}
	}

	allowlist := map[string]struct{}{}
	if cfg != nil && cfg.SSOProviderMode == "allowlist" {
		for _, providerID := range cfg.SSOProviderIDs {
			allowlist[providerID] = struct{}{}
		}
	}

	targetSchemaType := "human_user"
	if cfg != nil && cfg.RegistrationSchemaType != "" {
		targetSchemaType = cfg.RegistrationSchemaType
	}

	ssoProviders := make([]map[string]any, 0, len(list))
	for _, prov := range list {
		if prov.Protocol != "oidc" && prov.Protocol != "oauth2" && prov.Protocol != "saml" {
			continue
		}
		if len(allowlist) > 0 {
			if _, ok := allowlist[prov.ID]; !ok {
				continue
			}
		}
		if prov.Target.SchemaType != "" && prov.Target.SchemaType != targetSchemaType && prov.Target.SchemaID == "" {
			continue
		}
		ssoProviders = append(ssoProviders, map[string]any{
			"id":       prov.ID,
			"name":     prov.DisplayName,
			"template": prov.CatalogRef.TemplateID,
			"kind":     prov.Kind,
			"protocol": prov.Protocol,
		})
	}
	return ssoProviders
}

// getDefaultSchemaConfig loads the default identity schema and extracts auth config.
// This is the fallback when no login flow is resolved.
func (h *Handler) getDefaultSchemaConfig(r *http.Request) *SchemaAuthConfig {
	cfg, err := h.getDefaultSchemaConfigStrict(r.Context())
	if err != nil {
		logging.Printf("[login] default schema fallback failed: %v", err)
		return ExtractAuthConfig(`{}`)
	}
	return cfg
}

func (h *Handler) getDefaultSchemaConfigStrict(ctx context.Context) (*SchemaAuthConfig, error) {
	schemaRec, err := schema.ResolveDefaultHumanUserSchema(ctx, h.db.SQL())
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, errors.New("no default schema configured")
		}
		return nil, err
	}
	cfg := ExtractAuthConfig(schemaRec.Schema)
	cfg.SchemaID = schemaRec.ID
	cfg.SchemaType = schemaRec.Type
	cfg.RegistrationSchemaType = schemaRec.Type
	return cfg, nil
}

// getResolvedConfig resolves the best login flow for the request context,
// then merges the flow's config with the user schema's auth methods.
// Falls back to getDefaultSchemaConfig if no login flow matches.
func (h *Handler) getResolvedConfig(r *http.Request, flowIDOverride string) *SchemaAuthConfig {
	cfg, _, err := h.getResolvedConfigStrict(r, flowIDOverride)
	if err != nil {
		logging.Printf("[login] flow resolution failed: %v, falling back to schema config", err)
		return h.getDefaultSchemaConfig(r)
	}
	return cfg
}

func (h *Handler) altchaVerifierForConfig(cfg *SchemaAuthConfig) *captcha.AltchaVerifier {
	algorithm := "SHA-256"
	maxNumber := 100000
	if cfg != nil && cfg.Captcha != nil {
		if cfg.Captcha.Algorithm != "" {
			algorithm = cfg.Captcha.Algorithm
		}
		if cfg.Captcha.MaxNumber > 0 {
			maxNumber = cfg.Captcha.MaxNumber
		}
	}
	return captcha.NewAltchaVerifier(h.captchaHMACKey, algorithm, maxNumber)
}

func remoteIPFromAddr(addr string) string {
	host, _, err := net.SplitHostPort(addr)
	if err == nil {
		return host
	}
	return addr
}

func (h *Handler) loadFlowConfigStrict(ctx context.Context, flowID string, r *http.Request) (*SchemaAuthConfig, error) {
	var configJSON, authMethodsJSON string
	var strategy string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT COALESCE(strategy,'identifier_first'), COALESCE(config,'{}'), COALESCE(auth_methods,'{}')
		 FROM login_flows WHERE id = ?`, flowID,
	).Scan(&strategy, &configJSON, &authMethodsJSON)
	if err != nil {
		return nil, err
	}

	lf := &loginflow.LoginFlow{
		ID:          flowID,
		Strategy:    strategy,
		Config:      json.RawMessage(configJSON),
		AuthMethods: json.RawMessage(authMethodsJSON),
	}
	return h.buildConfigFromFlowStrict(ctx, lf, r)
}

func (h *Handler) buildConfigFromFlowStrict(ctx context.Context, lf *loginflow.LoginFlow, r *http.Request) (*SchemaAuthConfig, error) {
	// Start with the user schema config as the base for field definitions.
	base, err := h.getDefaultSchemaConfigStrict(ctx)
	if err != nil {
		return nil, err
	}

	// Parse the login flow's config JSON to extract branding, captcha, etc.
	flowCfg := ExtractLoginFlowConfig(string(lf.Config))
	if flowCfg != nil {
		if flowCfg.Ref.UserSchema != "" {
			base.RegistrationSchemaType = flowCfg.Ref.UserSchema
		}
		base.SSOProviderMode = flowCfg.SSO.Providers.Mode
		base.SSOProviderIDs = append([]string(nil), flowCfg.SSO.Providers.IDs...)
		// Apply flow's login config (strategy, mfa, registration).
		if flowCfg.Login.Strategy != "" {
			base.Login = flowCfg.Login
		}
		// Apply flow's branding (heading, colors, layout, etc.).
		if flowCfg.Branding.Heading != "" || flowCfg.Branding.Layout != "" {
			base.Branding = mergeBrandingDefaults(flowCfg.Branding)
		}
		// Apply captcha config from flow.
		if flowCfg.Captcha != nil {
			base.Captcha = flowCfg.Captcha
		}
		// Apply fingerprint config from flow.
		if flowCfg.Fingerprint != nil {
			base.Fingerprint = flowCfg.Fingerprint
		}
		// Apply rate limit from flow.
		if flowCfg.RateLimit != nil {
			base.RateLimit = flowCfg.RateLimit
		}
	}

	// Apply auth methods from the login flow as the base,
	// then let the user schema narrow them.
	var flowAuthMethods map[string]*AuthMethodEntry
	if len(lf.AuthMethods) > 0 && string(lf.AuthMethods) != "{}" {
		if json.Unmarshal(lf.AuthMethods, &flowAuthMethods) == nil && len(flowAuthMethods) > 0 {
			// Login flow is the base. User schema can only narrow (disable methods).
			merged := make(map[string]*AuthMethodEntry)
			for method, flowEntry := range flowAuthMethods {
				if schemaEntry, ok := base.AuthMethods[method]; ok {
					// Schema can disable a method the flow enabled, but not enable one the flow disabled.
					if flowEntry.Enabled && !schemaEntry.Enabled {
						merged[method] = schemaEntry // schema narrowed
					} else {
						merged[method] = flowEntry
					}
				} else {
					merged[method] = flowEntry
				}
			}
			base.AuthMethods = merged
		}
	}

	// Apply flow strategy.
	if lf.Strategy != "" {
		base.Login.Strategy = lf.Strategy
	}

	base.LoginFlowID = lf.ID
	if base.RegistrationSchemaType == "" {
		base.RegistrationSchemaType = base.SchemaType
	}
	return base, nil
}

type resolvedConfigMeta struct {
	Host              string
	OrgID             string
	ResolutionMode    string
	ResolvedFlowID    string
	UsedDefaultSchema bool
}

func (h *Handler) getResolvedConfigStrict(r *http.Request, flowIDOverride string) (*SchemaAuthConfig, resolvedConfigMeta, error) {
	ctx := r.Context()
	meta := resolvedConfigMeta{
		Host:  r.Host,
		OrgID: httputil.ResolveOrgID(r, ""),
	}

	// Preview path: if a specific flow ID is provided, load it directly.
	if flowIDOverride != "" {
		meta.ResolutionMode = "preview"
		meta.ResolvedFlowID = flowIDOverride
		cfg, err := h.loadFlowConfigStrict(ctx, flowIDOverride, r)
		return cfg, meta, err
	}

	meta.ResolutionMode = "resolver"
	lf, err := h.resolver.Resolve(ctx, loginflow.UserContext{OrgID: meta.OrgID})
	if err != nil {
		return nil, meta, err
	}

	meta.ResolvedFlowID = lf.ID
	cfg, err := h.buildConfigFromFlowStrict(ctx, lf, r)
	return cfg, meta, err
}

// Legacy login routes (handleLoginStart, handleLoginPassword, handleLoginComplete)
// and the loginSessions map have been removed per ADR-019.
// All login state is now managed by the Flow API.

// --- Magic Link ---

func (h *Handler) handleMagicLinkRequest(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Email   string `json:"email"`
		Purpose string `json:"purpose,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	email := strings.TrimSpace(req.Email)
	if email == "" {
		httputil.WriteError(w, http.StatusBadRequest, "email is required")
		return
	}
	purpose := strings.TrimSpace(req.Purpose)
	if purpose == "" {
		purpose = "auto"
	}

	userID, resolvedPurpose, err := h.queueMagicLink(r.Context(), email, purpose)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, err.Error())
		return
	}
	h.api.EmitAuthEvent(r.Context(), "auth.magic_link_sent", userID, map[string]any{
		"email":   email,
		"purpose": resolvedPurpose,
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"status":  "sent",
		"purpose": resolvedPurpose,
		"message": "Check your email for a sign-in link.",
	})
}

func (h *Handler) queueMagicLink(ctx context.Context, email, requestedPurpose string) (string, string, error) {
	var userID string
	err := h.db.SQL().QueryRowContext(ctx, `SELECT id FROM users WHERE identifier = ?`, email).Scan(&userID)
	if err != nil && err != sql.ErrNoRows {
		return "", "", errors.New("internal error")
	}

	purpose := requestedPurpose
	if purpose == "" || purpose == "auto" {
		if err == sql.ErrNoRows {
			purpose = "register"
		} else {
			purpose = "login"
		}
	}

	tx, txErr := h.db.SQL().BeginTx(ctx, nil)
	if txErr != nil {
		return "", "", errors.New("failed to create notification request")
	}
	defer tx.Rollback()

	if err == sql.ErrNoRows {
		newID := id.New()
		schemaRec, schemaErr := schema.ResolveDefaultHumanUserSchema(ctx, h.db.SQL())
		if schemaErr != nil {
			logging.Printf("[magic-link] default human user schema unavailable: %v", schemaErr)
			return "", "", errors.New("registration is not available")
		}
		payload := schema.MaterializeUserData(schemaRec.Schema, email, email, map[string]any{})
		if validateErr := schema.ValidateData(schemaRec.Schema, payload); validateErr != nil {
			logging.Printf("[magic-link] default schema rejected pending identity for %s: %v", email, validateErr)
			return "", "", errors.New("registration is not available")
		}
		if _, execErr := tx.ExecContext(ctx,
			`INSERT INTO users (id, org_id, identifier, display_name, state, schema_id, metadata, created_at, updated_at)
			 VALUES (?, 1, ?, ?, 'pending', ?, '{}', datetime('now'), datetime('now'))`,
			newID, email, email, schemaRec.ID,
		); execErr != nil {
			return "", "", errors.New("failed to create identity")
		}
		if uniqErr := uniqueness.EnforceFromIdentifier(ctx, tx, newID, "1", email); uniqErr != nil {
			return "", "", errors.New("identifier already exists")
		}
		if uniqErr := uniqueness.Enforce(ctx, tx, newID, "1", uniqueness.ExtractConstraints(schemaRec.Schema), payload); uniqErr != nil {
			return "", "", errors.New("identifier already exists")
		}
		userID = newID
		logging.Printf("[magic-link] created pending identity %s for %s", userID, email)
	}

	token, err := crypto.RandomBase64URL(32)
	if err != nil {
		return "", "", errors.New("token generation failed")
	}
	expiresAt := time.Now().UTC().Add(15 * time.Minute)
	if _, err := tx.ExecContext(ctx,
		`INSERT INTO tokens (id, type, token_hash, user_id, expires_at) VALUES (?, 'magic_link', ?, ?, ?)`,
		token, token, userID, expiresAt.Format(time.RFC3339),
	); err != nil {
		return "", "", errors.New("failed to store token")
	}

	if h.notifier != nil {
		templateKey := "magic_link_login"
		switch purpose {
		case "register":
			templateKey = "magic_link_register"
		case "invite":
			templateKey = "invite"
		case "reset":
			templateKey = "password_reset"
		case "verification":
			templateKey = "email_verification"
		}
		link := fmt.Sprintf("%s/v1/auth/magic-link/verify?token=%s", strings.TrimRight(h.baseURL, "/"), token)
		if _, err := h.notifier.EnqueueTx(ctx, tx, notify.RequestSpec{
			OrgID:         "1",
			AggregateID:   userID,
			AggregateType: "user",
			Medium:        notify.MediumEmail,
			Recipient:     email,
			TemplateKey:   templateKey,
			Payload: map[string]any{
				"email":      email,
				"identifier": email,
				"purpose":    purpose,
				"link":       link,
				"expires_at": expiresAt.Format(time.RFC3339),
			},
		}); err != nil {
			return "", "", errors.New("failed to queue notification")
		}
	}

	if err := tx.Commit(); err != nil {
		return "", "", errors.New("failed to create notification request")
	}
	return userID, purpose, nil
}

func (h *Handler) handleMagicLinkVerify(w http.ResponseWriter, r *http.Request) {
	token := r.URL.Query().Get("token")
	if token == "" {
		httputil.WriteError(w, http.StatusBadRequest, "token is required")
		return
	}

	// Load and validate token.
	var userID string
	var expiresAt, identifier string
	var usedAt sql.NullString
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT t.user_id, t.expires_at, t.last_used, u.identifier
		 FROM tokens t
		 JOIN users u ON u.id = t.user_id
		 WHERE t.token_hash = ? AND t.type = 'magic_link'`, token,
	).Scan(&userID, &expiresAt, &usedAt, &identifier)

	if err == sql.ErrNoRows {
		h.api.EmitAuthEvent(r.Context(), "auth.magic_link_failed", "", map[string]any{
			"reason": "invalid_token",
			"ip":     r.RemoteAddr,
		})
		httputil.WriteError(w, http.StatusNotFound, "invalid or expired link")
		return
	}
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "internal error")
		return
	}

	// Check expiry.
	expiry, _ := time.Parse(time.RFC3339, expiresAt)
	if time.Now().After(expiry) {
		logging.Printf("[magic-link] expired token used for %s (identity=%s)", identifier, userID)
		h.api.EmitAuthEvent(r.Context(), "auth.magic_link_failed", userID, map[string]any{
			"reason":     "expired",
			"identifier": identifier,
			"ip":         r.RemoteAddr,
		})
		httputil.WriteError(w, http.StatusGone, "link has expired")
		return
	}

	// Check single-use.
	if usedAt.Valid {
		logging.Printf("[magic-link] already-used token for %s (identity=%s, used_at=%s)", identifier, userID, usedAt.String)
		h.api.EmitAuthEvent(r.Context(), "auth.magic_link_failed", userID, map[string]any{
			"reason":     "already_used",
			"identifier": identifier,
			"used_at":    usedAt.String,
			"ip":         r.RemoteAddr,
		})
		httputil.WriteError(w, http.StatusGone, "link has already been used")
		return
	}

	// Mark as used.
	_, _ = h.db.SQL().ExecContext(r.Context(),
		`UPDATE tokens SET last_used = datetime('now') WHERE token_hash = ? AND type = 'magic_link'`, token)

	// Activate identity if pending (registration flow).
	_, _ = h.db.SQL().ExecContext(r.Context(),
		`UPDATE users SET state = 'active' WHERE id = ? AND state = 'pending'`, userID)

	// Create session.
	sessResp, err := h.api.CreateSessionForLogin(r.Context(), userID, r.UserAgent(), r.RemoteAddr, nil, &SessionProvenance{
		AuthMethod: "magic_link",
	})
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create session")
		return
	}

	// Link session to token.
	_, _ = h.db.SQL().ExecContext(r.Context(),
		`UPDATE tokens SET session_id = ? WHERE token_hash = ? AND type = 'magic_link'`, sessResp.Session.ID, token)

	// Set session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, h.cookies)

	logging.Printf("[magic-link] verified for %s (identity=%s, session=%s)", identifier, userID, sessResp.Session.ID)

	h.api.EmitAuthEvent(r.Context(), "auth.magic_link_verified", userID, map[string]any{
		"session_id": sessResp.Session.ID,
		"method":     "magic_link",
	})

	// Redirect to console.
	http.Redirect(w, r, "/console", http.StatusFound)
}
