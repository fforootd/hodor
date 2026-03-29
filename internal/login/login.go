// Package login provides API handlers for the login flow.
// These handlers support the <zitadel-login> web component.
package login

import (
	"database/sql"
	"encoding/json"
	"github.com/zitadel/zitadel/internal/logging"
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
// ADR-019: All login UI is driven by the flow API. Legacy routes removed.
func (h *Handler) Register(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/branding", h.handleBranding)
	mux.HandleFunc("GET /v1/auth/settings", h.handleAuthSettings)

	// Flow API (schema-driven) — the sole interface for login UI.
	mux.HandleFunc("POST /v1/login/flows", h.handleFlowCreate)
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

// Legacy login routes (handleLoginStart, handleLoginPassword, handleLoginComplete)
// and the loginSessions map have been removed per ADR-019.
// All login state is now managed by the Flow API.

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
		logging.Printf("[magic-link] created pending identity %s for %s", identityID, email)
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
		logging.Printf("[magic-link] notification send error: %v", err)
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
		logging.Printf("[magic-link] expired token used for %s (identity=%s)", identifier, identityID)
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
		logging.Printf("[magic-link] already-used token for %s (identity=%s, used_at=%s)", identifier, identityID, usedAt.String)
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

	logging.Printf("[magic-link] verified for %s (identity=%s, session=%s)", identifier, identityID, sessResp.Session.ID)

	h.api.EmitAuthEvent(r.Context(), "auth.magic_link_verified", identityID, map[string]any{
		"session_id": sessResp.Session.ID,
		"method":     "magic_link",
	})

	// Redirect to console.
	http.Redirect(w, r, "/console", http.StatusFound)
}
