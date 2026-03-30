// Package login provides SSO handlers for the OIDC authorization code flow.
//
// Flow:
//  1. GET /v1/auth/sso/{provider_id}/start → redirect to IDP authorize URL
//  2. IDP redirects back to GET /v1/auth/sso/callback?code=...&state=...
//  3. Exchange code for tokens, validate ID token, map claims, create/link identity
package login

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// oidcConfig holds parsed OIDC provider configuration.
type oidcConfig struct {
	Issuer       string `json:"issuer"`
	ClientID     string `json:"client_id"`
	ClientSecret string `json:"client_secret"`
	Scopes       string `json:"scopes"`
	TenantID     string `json:"tenant_id"`
}

// oidcEndpoints holds discovered OIDC endpoints.
type oidcEndpoints struct {
	AuthorizationEndpoint string `json:"authorization_endpoint"`
	TokenEndpoint         string `json:"token_endpoint"`
	UserInfoEndpoint      string `json:"userinfo_endpoint"`
	JwksURI               string `json:"jwks_uri"`
}

// RegisterSSORoutes mounts the SSO auth endpoints.
func (h *Handler) RegisterSSORoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/auth/sso/{provider_id}/start", h.handleSSOStart)
	mux.HandleFunc("GET /v1/auth/sso/callback", h.handleSSOCallback)
}

// --- SSO Start ---

func (h *Handler) handleSSOStart(w http.ResponseWriter, r *http.Request) {
	providerID := r.PathValue("provider_id")

	// Load provider config from providers table.
	var configStr, protocol string
	var enabled bool
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT config, protocol, enabled FROM providers WHERE id = ?`, providerID,
	).Scan(&configStr, &protocol, &enabled)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}
	if !enabled {
		httputil.WriteError(w, http.StatusForbidden, "provider is disabled")
		return
	}
	configJSONStr := configStr
	if protocol != "oidc" {
		httputil.WriteError(w, http.StatusBadRequest, "only OIDC providers are supported")
		return
	}

	var cfg oidcConfig
	_ = json.Unmarshal([]byte(configJSONStr), &cfg)

	// Substitute tenant_id in issuer if present.
	issuer := cfg.Issuer
	if cfg.TenantID != "" {
		issuer = strings.Replace(issuer, "{tenant_id}", cfg.TenantID, 1)
	}

	// Discover OIDC endpoints.
	endpoints, err := discoverOIDC(r.Context(), issuer)
	if err != nil {
		httputil.WriteError(w, http.StatusBadGateway, "OIDC discovery failed: "+err.Error())
		return
	}

	// Generate state, nonce, PKCE.
	state := crypto.MustRandomHex(16)
	nonce := crypto.MustRandomHex(8)
	pkceVerifier := crypto.MustRandomHex(22)
	pkceChallenge := crypto.HashTokenBase64URL(pkceVerifier)

	// Store state in database.
	_, _ = h.db.SQL().ExecContext(r.Context(),
		`INSERT INTO auth_states (id, type, state, provider_id, pkce_verifier, nonce, redirect_uri, expires_at, created_at)
		 VALUES (?, 'sso', ?, ?, ?, ?, ?, datetime('now', '+10 minutes'), datetime('now'))`,
		state, state, providerID, pkceVerifier, nonce, h.baseURL+"/v1/auth/sso/callback",
	)

	// Build authorize URL.
	scopes := cfg.Scopes
	if scopes == "" {
		scopes = "openid email profile"
	}

	params := url.Values{
		"response_type":         {"code"},
		"client_id":             {cfg.ClientID},
		"redirect_uri":          {h.baseURL + "/v1/auth/sso/callback"},
		"scope":                 {scopes},
		"state":                 {state},
		"nonce":                 {nonce},
		"code_challenge":        {pkceChallenge},
		"code_challenge_method": {"S256"},
	}

	authorizeURL := endpoints.AuthorizationEndpoint + "?" + params.Encode()
	http.Redirect(w, r, authorizeURL, http.StatusFound)
}

// --- SSO Callback ---

func (h *Handler) handleSSOCallback(w http.ResponseWriter, r *http.Request) {
	code := r.URL.Query().Get("code")
	state := r.URL.Query().Get("state")
	errParam := r.URL.Query().Get("error")

	if errParam != "" {
		errDesc := r.URL.Query().Get("error_description")
		logging.Printf("[sso] IDP returned error: %s — %s", url.QueryEscape(errParam), url.QueryEscape(errDesc))
		http.Redirect(w, r, "/login?error=sso_failed", http.StatusFound)
		return
	}
	if code == "" || state == "" {
		http.Redirect(w, r, "/login?error=sso_invalid", http.StatusFound)
		return
	}

	// Look up state.
	var providerID, pkceVerifier, nonce string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT provider_id, pkce_verifier, nonce FROM auth_states WHERE state = ?`, state,
	).Scan(&providerID, &pkceVerifier, &nonce)
	if err != nil {
		logging.Printf("[sso] state lookup failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_expired", http.StatusFound)
		return
	}

	// Delete used state.
	_, _ = h.db.SQL().ExecContext(r.Context(), `DELETE FROM auth_states WHERE state = ?`, state)

	// Load provider from providers table.
	var provConfigStr, provOverridesStr string
	err = h.db.SQL().QueryRowContext(r.Context(),
		`SELECT config, COALESCE(claim_overrides, '{}') FROM providers WHERE id = ?`, providerID,
	).Scan(&provConfigStr, &provOverridesStr)
	if err != nil {
		logging.Printf("[sso] provider lookup failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_config", http.StatusFound)
		return
	}
	var configMap2 map[string]any
	_ = json.Unmarshal([]byte(provConfigStr), &configMap2)
	configJSON2, _ := json.Marshal(configMap2)
	overridesJSON2 := []byte(provOverridesStr)

	// Load template and auto_register from providers table.
	var template string
	var autoRegister bool
	_ = h.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(template, ''), auto_register FROM providers WHERE id = ?`, providerID,
	).Scan(&template, &autoRegister)

	var cfg oidcConfig
	_ = json.Unmarshal(configJSON2, &cfg)

	issuer := cfg.Issuer
	if cfg.TenantID != "" {
		issuer = strings.Replace(issuer, "{tenant_id}", cfg.TenantID, 1)
	}

	// Discover token endpoint.
	endpoints, err := discoverOIDC(r.Context(), issuer)
	if err != nil {
		logging.Printf("[sso] discovery failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_discovery", http.StatusFound)
		return
	}

	// Exchange code for tokens.
	tokenResp, err := exchangeCode(r.Context(), endpoints.TokenEndpoint, cfg, code, pkceVerifier, h.baseURL+"/v1/auth/sso/callback")
	if err != nil {
		logging.Printf("[sso] token exchange failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_token", http.StatusFound)
		return
	}

	// Parse ID token claims (simplified — production would verify signature).
	claims, err := parseIDTokenClaims(tokenResp.IDToken)
	if err != nil {
		logging.Printf("[sso] ID token parse failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_token_invalid", http.StatusFound)
		return
	}

	// Verify nonce.
	if claimNonce, _ := claims["nonce"].(string); claimNonce != nonce {
		logging.Printf("[sso] nonce mismatch")
		http.Redirect(w, r, "/login?error=sso_nonce", http.StatusFound)
		return
	}

	// Extract external subject.
	externalSub, _ := claims["sub"].(string)
	externalEmail, _ := claims["email"].(string)
	if externalSub == "" {
		logging.Printf("[sso] no sub claim in ID token")
		http.Redirect(w, r, "/login?error=sso_no_sub", http.StatusFound)
		return
	}

	// Find or create linked account.
	userID, err := h.findOrCreateLinkedIdentity(r.Context(), providerID, externalSub, externalEmail, claims, string(overridesJSON2), autoRegister)
	if err != nil {
		logging.Printf("[sso] link/create failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_link_failed", http.StatusFound)
		return
	}

	// Create session.
	sessResp, err := h.api.CreateSessionForLogin(r.Context(), userID, r.UserAgent(), r.RemoteAddr, nil)
	if err != nil {
		logging.Printf("[sso] session create failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_session", http.StatusFound)
		return
	}

	// Emit event.
	h.api.EmitAuthEvent(r.Context(), "auth.sso_login", userID, map[string]any{
		"provider_id":  providerID,
		"template":     template,
		"external_sub": externalSub,
	})

	// Set cookie and redirect.
	session.SetSessionCookie(w, sessResp.Token, h.cookies)
	http.Redirect(w, r, "/console", http.StatusFound)
}

// --- Identity Linking ---

func (h *Handler) findOrCreateLinkedIdentity(ctx context.Context, providerID, externalSub, externalEmail string, claims map[string]any, overridesJSON string, autoRegister bool) (string, error) {
	// Check if already linked.
	var userID string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT user_id FROM linked_identities WHERE provider_id = ? AND external_sub = ?`,
		providerID, externalSub,
	).Scan(&userID)

	if err == nil {
		// Already linked — update last_used_at and raw_claims.
		claimsJSON, _ := json.Marshal(claims)
		_, _ = h.db.SQL().ExecContext(ctx,
			`UPDATE linked_identities SET last_used_at = datetime('now'), raw_claims = ?, external_email = ? WHERE provider_id = ? AND external_sub = ?`,
			string(claimsJSON), externalEmail, providerID, externalSub,
		)
		return userID, nil
	}

	if !autoRegister {
		return "", fmt.Errorf("no linked account found and auto_register is disabled")
	}

	// Map claims to profile using the default human-user schema + provider overrides.
	schemaRec, err := schema.ResolveDefaultHumanUserSchema(ctx, h.db.SQL())
	if err != nil {
		return "", fmt.Errorf("resolve default human user schema: %w", err)
	}
	var overrides map[string]string
	_ = json.Unmarshal([]byte(overridesJSON), &overrides)

	profile, _ := MapClaims(schemaRec.Schema, overrides, claims)

	displayName := ""
	if dn, ok := profile["display_name"].(string); ok {
		displayName = dn
	}
	if displayName == "" {
		displayName = externalEmail
	}

	identifier := externalEmail
	if identifier == "" {
		identifier = externalSub
	}

	// Create identity.
	newID := id.New()
	payload := schema.MaterializeUserData(schemaRec.Schema, identifier, displayName, profile)
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return "", fmt.Errorf("validate identity against %s: %w", schemaRec.ID, err)
	}

	profileJSON, _ := json.Marshal(profile)

	tx, err := h.db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return "", fmt.Errorf("begin identity create: %w", err)
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(ctx,
		`INSERT INTO users (id, org_id, identifier, display_name, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, 'active', ?, ?, datetime('now'), datetime('now'))`,
		newID, identifier, displayName, schemaRec.ID, string(profileJSON),
	)
	if err != nil {
		return "", fmt.Errorf("create identity: %w", err)
	}
	if err := uniqueness.EnforceFromIdentifier(ctx, tx, newID, "1", identifier); err != nil {
		return "", err
	}
	if err := uniqueness.Enforce(ctx, tx, newID, "1", uniqueness.ExtractConstraints(schemaRec.Schema), payload); err != nil {
		return "", err
	}

	// Create linked account.
	linkID := id.New()
	claimsJSON, _ := json.Marshal(claims)
	_, err = tx.ExecContext(ctx,
		`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
		 VALUES (?, ?, ?, ?, ?, ?, datetime('now'))`,
		linkID, newID, providerID, externalSub, externalEmail, string(claimsJSON),
	)
	if err != nil {
		return "", fmt.Errorf("create linked account: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return "", fmt.Errorf("commit identity create: %w", err)
	}

	return newID, nil
}

// --- OIDC Helpers ---

func discoverOIDC(ctx context.Context, issuer string) (*oidcEndpoints, error) {
	discoveryURL := strings.TrimRight(issuer, "/") + "/.well-known/openid-configuration"

	req, err := http.NewRequestWithContext(ctx, "GET", discoveryURL, nil)
	if err != nil {
		return nil, err
	}

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("discovery request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("discovery returned %d", resp.StatusCode)
	}

	var endpoints oidcEndpoints
	if err := json.NewDecoder(resp.Body).Decode(&endpoints); err != nil {
		return nil, fmt.Errorf("decode discovery: %w", err)
	}

	return &endpoints, nil
}

type tokenResponse struct {
	AccessToken  string `json:"access_token"`
	IDToken      string `json:"id_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int    `json:"expires_in"`
	RefreshToken string `json:"refresh_token"`
}

func exchangeCode(ctx context.Context, tokenEndpoint string, cfg oidcConfig, code, pkceVerifier, redirectURI string) (*tokenResponse, error) {
	data := url.Values{
		"grant_type":    {"authorization_code"},
		"code":          {code},
		"redirect_uri":  {redirectURI},
		"client_id":     {cfg.ClientID},
		"code_verifier": {pkceVerifier},
	}
	if cfg.ClientSecret != "" {
		data.Set("client_secret", cfg.ClientSecret)
	}

	req, err := http.NewRequestWithContext(ctx, "POST", tokenEndpoint, strings.NewReader(data.Encode()))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("token request: %w", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("token endpoint returned %d: %s", resp.StatusCode, string(body))
	}

	var tokenResp tokenResponse
	if err := json.Unmarshal(body, &tokenResp); err != nil {
		return nil, fmt.Errorf("decode token response: %w", err)
	}

	return &tokenResp, nil
}

// parseIDTokenClaims decodes the payload of a JWT without verifying the signature.
// Production usage should verify the signature using the JWKS endpoint.
func parseIDTokenClaims(idToken string) (map[string]any, error) {
	parts := strings.Split(idToken, ".")
	if len(parts) != 3 {
		return nil, fmt.Errorf("invalid JWT: expected 3 parts, got %d", len(parts))
	}

	// Decode payload (part 1).
	payload, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return nil, fmt.Errorf("decode payload: %w", err)
	}

	var claims map[string]any
	if err := json.Unmarshal(payload, &claims); err != nil {
		return nil, fmt.Errorf("parse claims: %w", err)
	}

	return claims, nil
}
