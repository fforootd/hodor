// Package login provides API handlers for the login flow.
// These handlers support the <zitadel-login> web component.
package login

import (
	"database/sql"
	"encoding/json"
	"log"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/notify"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// Handler provides login-flow API endpoints.
type Handler struct {
	db        *database.DB
	passwords *auth.Passwords
	api       *api.API
	notify    notify.Channel
	baseURL   string
	flows     *FlowStore
	cookies   *session.CookieConfig
}

// New creates a new login API handler.
func New(db *database.DB, passwords *auth.Passwords, restAPI *api.API, cookies *session.CookieConfig) *Handler {
	return &Handler{
		db:        db,
		passwords: passwords,
		api:       restAPI,
		notify:    notify.NewStdout(),
		baseURL:   "http://localhost:8080",
		flows:     NewFlowStore(),
		cookies:   cookies,
	}
}

// Register mounts the login API routes onto the given mux.
func (h *Handler) Register(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/branding", h.handleBranding)
	mux.HandleFunc("GET /v1/auth/settings", h.handleAuthSettings)

	// Flow API (schema-driven).
	mux.HandleFunc("POST /v1/login/flows", h.handleFlowCreate)
	mux.HandleFunc("POST /v1/login/flows/", h.handleFlowSubmit)
	mux.HandleFunc("GET /v1/login/flows/", h.handleFlowGet)

	// Legacy routes (thin wrappers around flow API).
	mux.HandleFunc("POST /v1/login/start", h.handleLoginStart)
	mux.HandleFunc("POST /v1/login/password", h.handleLoginPassword)
	mux.HandleFunc("POST /v1/login/complete", h.handleLoginComplete)
	mux.HandleFunc("POST /v1/auth/magic-link", h.handleMagicLinkRequest)
	mux.HandleFunc("GET /v1/auth/magic-link/verify", h.handleMagicLinkVerify)

	// SSO / OIDC
	h.RegisterSSORoutes(mux)
}

// --- Branding (schema-driven) ---

func (h *Handler) handleBranding(w http.ResponseWriter, r *http.Request) {
	cfg := h.getDefaultSchemaConfig(r)
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
	cfg := h.getDefaultSchemaConfig(r)
	ssoProviders := h.loadSSOProviders(r)

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
		"preset":               cfg.Login.Preset,
		"auth_methods":         authMethods,
		"mfa_required":         cfg.Login.MFARequired,
		"registration_allowed": cfg.Login.RegistrationAllowed,
		"identifier_fields":    cfg.Identifiers,
	})
}

// loadSSOProviders reads enabled SSO providers from the database.
func (h *Handler) loadSSOProviders(r *http.Request) []map[string]any {
	var ssoProviders []map[string]any
	rows, err := h.db.SQL().QueryContext(r.Context(),
		`SELECT id, name, template, protocol FROM providers WHERE enabled = 1 ORDER BY display_order, name`)
	if err != nil {
		return ssoProviders
	}
	defer rows.Close()
	for rows.Next() {
		var pid, pname, ptemplate, pprotocol string
		if rows.Scan(&pid, &pname, &ptemplate, &pprotocol) == nil {
			ssoProviders = append(ssoProviders, map[string]any{
				"id": pid, "name": pname, "template": ptemplate, "protocol": pprotocol,
			})
		}
	}
	_ = rows.Err()
	if ssoProviders == nil {
		ssoProviders = []map[string]any{}
	}
	return ssoProviders
}

// getDefaultSchemaConfig loads the default identity schema and extracts auth config.
// Resolution order: is_default=true for the type, fallback to oldest schema.
func (h *Handler) getDefaultSchemaConfig(r *http.Request) *SchemaAuthConfig {
	var schemaJSON string
	// Try is_default first.
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT schema FROM schemas WHERE is_default = true ORDER BY created_at ASC LIMIT 1`,
	).Scan(&schemaJSON)
	if err != nil || schemaJSON == "" {
		// Fallback to oldest schema (pre-migration compatibility).
		err = h.db.SQL().QueryRowContext(r.Context(),
			`SELECT schema FROM schemas ORDER BY created_at ASC LIMIT 1`,
		).Scan(&schemaJSON)
		if err != nil || schemaJSON == "" {
			return ExtractAuthConfig(`{}`)
		}
	}
	return ExtractAuthConfig(schemaJSON)
}

// --- Login Start ---

type loginSession struct {
	ID         string
	IdentityID string
	Identifier string
	Display    string
	Verified   bool
	CreatedAt  time.Time
}

var loginSessions = map[string]*loginSession{}

func (h *Handler) handleLoginStart(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Identifier string `json:"identifier"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	identifier := strings.TrimSpace(req.Identifier)
	if identifier == "" {
		httputil.WriteError(w, http.StatusBadRequest, "identifier is required")
		return
	}

	// Resolve identifier via unique_fields (ADR-016).
	orgID := httputil.ResolveOrgID(r, "")

	resolved, err := uniqueness.ResolveIdentifier(r.Context(), h.db.SQL(), identifier, orgID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "internal error")
		return
	}
	if resolved == nil {
		httputil.WriteError(w, http.StatusNotFound, "account not found")
		return
	}

	identityID := resolved.EntityID
	displayName := resolved.DisplayName

	sid := id.NewLoginSession()
	loginSessions[sid] = &loginSession{
		ID:         sid,
		IdentityID: identityID,
		Identifier: identifier,
		Display:    displayName,
		CreatedAt:  time.Now(),
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"login_session_id": sid,
		"entity_id":        identityID,
		"org_id":           "",
		"display_name":     displayName,
		"auth_methods":     []string{"password", "magic_link"},
		"next_step":        "password",
	})
}

// --- Password Verification ---

func (h *Handler) handleLoginPassword(w http.ResponseWriter, r *http.Request) {
	var req struct {
		LoginSessionID string `json:"login_session_id"`
		Password       string `json:"password"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	sess, ok := loginSessions[req.LoginSessionID]
	if !ok {
		httputil.WriteError(w, http.StatusNotFound, "login session not found")
		return
	}

	valid, err := h.passwords.CheckPassword(r.Context(), sess.IdentityID, req.Password)
	if err != nil || !valid {
		h.api.EmitAuthEvent(r.Context(), "auth.login_failed", sess.IdentityID, map[string]any{
			"reason":           "invalid_password",
			"login_session_id": sess.ID,
		})
		httputil.WriteJSON(w, http.StatusOK, map[string]any{"error": "invalid_password"})
		return
	}

	sess.Verified = true
	httputil.WriteJSON(w, http.StatusOK, map[string]any{"next_step": "complete"})
}

// --- Login Complete ---

func (h *Handler) handleLoginComplete(w http.ResponseWriter, r *http.Request) {
	var req struct {
		LoginSessionID string `json:"login_session_id"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	sess, ok := loginSessions[req.LoginSessionID]
	if !ok {
		httputil.WriteError(w, http.StatusNotFound, "login session not found")
		return
	}
	if !sess.Verified {
		httputil.WriteError(w, http.StatusForbidden, "login not verified")
		return
	}

	// Create a real session via the existing API (emits session.created event).
	sessResp, err := h.api.CreateSessionInternal(r.Context(), sess.IdentityID, r.UserAgent(), r.RemoteAddr)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create session")
		return
	}

	// Set the session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, h.cookies)

	delete(loginSessions, req.LoginSessionID)
	log.Printf("[login] completed for %s (identity=%s, session=%s)", sess.Identifier, sess.IdentityID, sessResp.Session.ID)

	// Emit auth event.
	h.api.EmitAuthEvent(r.Context(), "auth.login_success", sess.IdentityID, map[string]any{
		"session_id": sessResp.Session.ID,
		"method":     "password",
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"session_id":   sessResp.Session.ID,
		"redirect_uri": "/console",
	})
}

// --- Magic Link ---

func (h *Handler) handleMagicLinkRequest(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Email string `json:"email"`
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

	// Look up existing identity.
	var identityID string
	var purpose string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT id FROM entities WHERE identifier = ?`, email,
	).Scan(&identityID)

	if err == sql.ErrNoRows {
		// REGISTRATION: create identity in pending state.
		purpose = "register"
		newID := id.New()
		_, err = h.db.SQL().ExecContext(r.Context(),
			`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, created_at, updated_at)
			 VALUES (?, 1, ?, ?, 'pending', '{}', '{}', datetime('now'), datetime('now'))`,
			newID, email, email,
		)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "failed to create identity")
			return
		}
		identityID = newID
		log.Printf("[magic-link] created pending identity %s for %s", identityID, email)
	} else if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "internal error")
		return
	} else {
		purpose = "login"
	}

	// Generate token.
	token, err := crypto.RandomBase64URL(32)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "token generation failed")
		return
	}
	expiresAt := time.Now().Add(15 * time.Minute)

	// Store token.
	_, err = h.db.SQL().ExecContext(r.Context(),
		`INSERT INTO magic_tokens (token, entity_id, expires_at) VALUES (?, ?, ?)`,
		token, identityID, expiresAt.Format(time.RFC3339),
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to store token")
		return
	}

	// Send notification via channel (stdout by default).
	subject := "Sign in to Zitadel"
	if purpose == "register" {
		subject = "Complete your Zitadel registration"
	}
	body := notify.FormatMagicLink(h.baseURL, token, expiresAt)
	if err := h.notify.Send(email, subject, body); err != nil {
		log.Printf("[magic-link] notification send error: %v", err)
	}

	// Emit event.
	h.api.EmitAuthEvent(r.Context(), "auth.magic_link_sent", identityID, map[string]any{
		"email":   email,
		"purpose": purpose,
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"status":  "sent",
		"purpose": purpose,
		"message": "Check your email for a sign-in link.",
	})
}

func (h *Handler) handleMagicLinkVerify(w http.ResponseWriter, r *http.Request) {
	token := r.URL.Query().Get("token")
	if token == "" {
		httputil.WriteError(w, http.StatusBadRequest, "token is required")
		return
	}

	// Load and validate token.
	var identityID string
	var expiresAt, identifier string
	var usedAt sql.NullString
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT mt.entity_id, mt.expires_at, mt.used_at, i.identifier
		 FROM magic_tokens mt
		 JOIN entities e ON i.id = mt.entity_id
		 WHERE mt.token = ?`, token,
	).Scan(&identityID, &expiresAt, &usedAt, &identifier)

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
		log.Printf("[magic-link] expired token used for %s (identity=%s)", identifier, identityID)
		h.api.EmitAuthEvent(r.Context(), "auth.magic_link_failed", identityID, map[string]any{
			"reason":     "expired",
			"identifier": identifier,
			"ip":         r.RemoteAddr,
		})
		httputil.WriteError(w, http.StatusGone, "link has expired")
		return
	}

	// Check single-use.
	if usedAt.Valid {
		log.Printf("[magic-link] already-used token for %s (identity=%s, used_at=%s)", identifier, identityID, usedAt.String)
		h.api.EmitAuthEvent(r.Context(), "auth.magic_link_failed", identityID, map[string]any{
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
		`UPDATE magic_tokens SET used_at = datetime('now') WHERE token = ?`, token)

	// Activate identity if pending (registration flow).
	_, _ = h.db.SQL().ExecContext(r.Context(),
		`UPDATE entities SET state = 'active' WHERE id = ? AND state = 'pending'`, identityID)

	// Create session.
	sessResp, err := h.api.CreateSessionInternal(r.Context(), identityID, r.UserAgent(), r.RemoteAddr)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create session")
		return
	}

	// Link session to token.
	_, _ = h.db.SQL().ExecContext(r.Context(),
		`UPDATE magic_tokens SET session_id = ? WHERE token = ?`, sessResp.Session.ID, token)

	// Set session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, h.cookies)

	log.Printf("[magic-link] verified for %s (identity=%s, session=%s)", identifier, identityID, sessResp.Session.ID)

	h.api.EmitAuthEvent(r.Context(), "auth.magic_link_verified", identityID, map[string]any{
		"session_id": sessResp.Session.ID,
		"method":     "magic_link",
	})

	// Redirect to console.
	http.Redirect(w, r, "/console", http.StatusFound)
}
