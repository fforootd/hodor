// Package login provides SSO handlers for the OIDC/OAuth2 authorization code flow.
package login

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

type providerConnectionConfig struct {
	Issuer           string `json:"issuer"`
	ClientID         string `json:"client_id"`
	ClientSecret     string `json:"client_secret"`
	TenantID         string `json:"tenant_id"`
	AuthorizationURL string `json:"authorization_url"`
	TokenURL         string `json:"token_url"`
	UserInfoURL      string `json:"userinfo_url"`
	Scopes           any    `json:"scopes"`
}

type oidcEndpoints struct {
	AuthorizationEndpoint string `json:"authorization_endpoint"`
	TokenEndpoint         string `json:"token_endpoint"`
	UserInfoEndpoint      string `json:"userinfo_endpoint"`
	JwksURI               string `json:"jwks_uri"`
}

func (h *Handler) RegisterSSORoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/auth/sso/{provider_id}/start", h.handleSSOStart)
	mux.HandleFunc("GET /v1/auth/sso/callback", h.handleSSOCallback)
}

func (h *Handler) handleSSOStart(w http.ResponseWriter, r *http.Request) {
	providerID := r.PathValue("provider_id")
	repo := providers.NewRepository(h.db.SQL())
	prov, err := repo.Get(r.Context(), providerID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}
	if !prov.Enabled {
		httputil.WriteError(w, http.StatusForbidden, "provider is disabled")
		return
	}
	if prov.Protocol != "oidc" && prov.Protocol != "oauth2" {
		httputil.WriteError(w, http.StatusBadRequest, "only OIDC and OAuth2 providers are supported")
		return
	}

	flowID := r.URL.Query().Get("flow_id")
	if flowID != "" {
		cfg, cfgErr := h.loadFlowConfigStrict(r.Context(), flowID, r)
		if cfgErr != nil || !providerAllowedForConfig(*prov, cfg) {
			httputil.WriteError(w, http.StatusForbidden, "provider is not allowed for this flow")
			return
		}
	}

	var cfg providerConnectionConfig
	configJSON, _ := json.Marshal(prov.Connection)
	_ = json.Unmarshal(configJSON, &cfg)

	issuer := cfg.Issuer
	if cfg.TenantID != "" {
		issuer = strings.Replace(issuer, "{tenant_id}", cfg.TenantID, 1)
	}

	endpoints := &oidcEndpoints{
		AuthorizationEndpoint: cfg.AuthorizationURL,
		TokenEndpoint:         cfg.TokenURL,
		UserInfoEndpoint:      cfg.UserInfoURL,
	}
	if prov.Protocol == "oidc" {
		endpoints, err = discoverOIDC(r.Context(), issuer)
		if err != nil {
			httputil.WriteError(w, http.StatusBadGateway, "OIDC discovery failed: "+err.Error())
			return
		}
	}

	state := crypto.MustRandomHex(16)
	nonce := ""
	if prov.Protocol == "oidc" {
		nonce = crypto.MustRandomHex(8)
	}
	pkceVerifier := crypto.MustRandomHex(22)
	pkceChallenge := crypto.HashTokenBase64URL(pkceVerifier)
	stateDataJSON, _ := json.Marshal(map[string]any{"flow_id": flowID})

	_, _ = h.db.SQL().ExecContext(r.Context(),
		`INSERT INTO auth_states (id, type, state, provider_id, pkce_verifier, nonce, redirect_uri, data, expires_at, created_at)
		 VALUES (?, 'sso', ?, ?, ?, ?, ?, ?, datetime('now', '+10 minutes'), datetime('now'))`,
		state, state, providerID, pkceVerifier, nonce, h.baseURL+"/v1/auth/sso/callback", string(stateDataJSON),
	)

	params := url.Values{
		"response_type":         {"code"},
		"client_id":             {cfg.ClientID},
		"redirect_uri":          {h.baseURL + "/v1/auth/sso/callback"},
		"scope":                 {scopeString(cfg.Scopes, prov.Protocol)},
		"state":                 {state},
		"code_challenge":        {pkceChallenge},
		"code_challenge_method": {"S256"},
	}
	if nonce != "" {
		params.Set("nonce", nonce)
	}

	http.Redirect(w, r, endpoints.AuthorizationEndpoint+"?"+params.Encode(), http.StatusFound)
}

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

	var providerID, pkceVerifier, nonce, stateDataJSON string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT provider_id, pkce_verifier, nonce, COALESCE(data,'{}') FROM auth_states WHERE state = ?`, state,
	).Scan(&providerID, &pkceVerifier, &nonce, &stateDataJSON)
	if err != nil {
		logging.Printf("[sso] state lookup failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_expired", http.StatusFound)
		return
	}
	_, _ = h.db.SQL().ExecContext(r.Context(), `DELETE FROM auth_states WHERE state = ?`, state)

	var stateData map[string]any
	_ = json.Unmarshal([]byte(stateDataJSON), &stateData)
	flowID, _ := stateData["flow_id"].(string)
	if flowID != "" {
		r = r.WithContext(telemetry.WithFlowID(r.Context(), flowID))
	}

	repo := providers.NewRepository(h.db.SQL())
	prov, err := repo.Get(r.Context(), providerID)
	if err != nil {
		logging.Printf("[sso] provider lookup failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_config", http.StatusFound)
		return
	}

	var cfg providerConnectionConfig
	configJSON, _ := json.Marshal(prov.Connection)
	_ = json.Unmarshal(configJSON, &cfg)

	issuer := cfg.Issuer
	if cfg.TenantID != "" {
		issuer = strings.Replace(issuer, "{tenant_id}", cfg.TenantID, 1)
	}

	endpoints := &oidcEndpoints{
		AuthorizationEndpoint: cfg.AuthorizationURL,
		TokenEndpoint:         cfg.TokenURL,
		UserInfoEndpoint:      cfg.UserInfoURL,
	}
	if prov.Protocol == "oidc" {
		endpoints, err = discoverOIDC(r.Context(), issuer)
		if err != nil {
			logging.Printf("[sso] discovery failed: %v", err)
			http.Redirect(w, r, "/login?error=sso_discovery", http.StatusFound)
			return
		}
	}

	tokenResp, err := exchangeCode(r.Context(), endpoints.TokenEndpoint, cfg, code, pkceVerifier, h.baseURL+"/v1/auth/sso/callback")
	if err != nil {
		logging.Printf("[sso] token exchange failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_token", http.StatusFound)
		return
	}

	var claims map[string]any
	if tokenResp.IDToken != "" {
		claims, err = parseIDTokenClaims(tokenResp.IDToken)
		if err != nil {
			logging.Printf("[sso] ID token parse failed: %v", err)
			http.Redirect(w, r, "/login?error=sso_token_invalid", http.StatusFound)
			return
		}
		if nonce != "" {
			claimNonce, _ := claims["nonce"].(string)
			if claimNonce != nonce {
				logging.Printf("[sso] nonce mismatch")
				http.Redirect(w, r, "/login?error=sso_nonce", http.StatusFound)
				return
			}
		}
	} else {
		claims, err = fetchUserInfo(r.Context(), endpoints.UserInfoEndpoint, tokenResp.AccessToken)
		if err != nil {
			logging.Printf("[sso] userinfo fetch failed: %v", err)
			http.Redirect(w, r, "/login?error=sso_userinfo", http.StatusFound)
			return
		}
	}

	externalSub, _ := claims["sub"].(string)
	if externalSub == "" {
		externalSub = stringifyClaim(claims["id"])
	}
	externalEmail, _ := claims["email"].(string)
	if externalEmail == "" {
		externalEmail = stringifyClaim(claims["preferred_username"])
	}
	if externalSub == "" {
		logging.Printf("[sso] no subject claim in provider response")
		http.Redirect(w, r, "/login?error=sso_no_sub", http.StatusFound)
		return
	}

	userID, err := h.findOrCreateLinkedIdentity(r.Context(), *prov, externalSub, externalEmail, claims)
	if err != nil {
		logging.Printf("[sso] link/create failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_link_failed", http.StatusFound)
		return
	}

	sessResp, err := h.api.CreateSessionForLogin(r.Context(), userID, r.UserAgent(), r.RemoteAddr, nil, &SessionProvenance{
		AuthMethod:   "sso",
		ProviderID:   prov.ID,
		ProviderKind: prov.Kind,
		LoginFlowID:  flowID,
		AuthContext: map[string]any{
			"provider_protocol": prov.Protocol,
			"provider_kind":     prov.Kind,
			"flow_id":           flowID,
		},
	})
	if err != nil {
		logging.Printf("[sso] session create failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_session", http.StatusFound)
		return
	}

	h.api.EmitAuthEvent(r.Context(), "auth.sso_login", userID, map[string]any{
		"provider_id":   prov.ID,
		"provider_kind": prov.Kind,
		"protocol":      prov.Protocol,
		"login_flow_id": flowID,
		"external_sub":  externalSub,
	})

	session.SetSessionCookie(w, sessResp.Token, h.cookies)
	http.Redirect(w, r, "/console", http.StatusFound)
}

func (h *Handler) findOrCreateLinkedIdentity(ctx context.Context, prov providers.Provider, externalSub, externalEmail string, claims map[string]any) (string, error) {
	var userID string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT user_id FROM linked_identities WHERE provider_id = ? AND external_sub = ?`,
		prov.ID, externalSub,
	).Scan(&userID)
	if err == nil {
		claimsJSON, _ := json.Marshal(claims)
		_, _ = h.db.SQL().ExecContext(ctx,
			`UPDATE linked_identities SET last_used_at = datetime('now'), raw_claims = ?, external_email = ? WHERE provider_id = ? AND external_sub = ?`,
			string(claimsJSON), externalEmail, prov.ID, externalSub,
		)
		return userID, nil
	}

	if linkedUserID, ok := h.findLinkableIdentity(ctx, prov.Linking, externalEmail, externalSub); ok {
		claimsJSON, _ := json.Marshal(claims)
		linkID := id.New()
		if _, linkErr := h.db.SQL().ExecContext(ctx,
			`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
			 VALUES (?, ?, ?, ?, ?, ?, datetime('now'))`,
			linkID, linkedUserID, prov.ID, externalSub, externalEmail, string(claimsJSON),
		); linkErr == nil {
			return linkedUserID, nil
		}
	}

	if prov.Linking.Mode == providers.LinkModeLinkOnly {
		return "", fmt.Errorf("no linked account found and provider is configured for link_only")
	}

	targetSchemaID, _, err := providers.ResolveTargetSchema(ctx, h.db.SQL(), prov.Target)
	if err != nil {
		return "", err
	}
	schemaRec, err := schema.LoadSchemaRecord(ctx, h.db.SQL(), targetSchemaID)
	if err != nil {
		return "", fmt.Errorf("resolve target schema %s: %w", targetSchemaID, err)
	}

	profile, _ := MapClaims(schemaRec.Schema, prov.Mapping.Claims, claims)

	displayName := ""
	if dn, ok := profile["display_name"].(string); ok {
		displayName = dn
	}
	if displayName == "" {
		displayName = externalEmail
	}
	if displayName == "" {
		displayName = externalSub
	}

	identifier := externalEmail
	if prov.Linking.MatchBy == providers.LinkMatchIdentifier && identifier == "" {
		identifier = externalSub
	}
	if identifier == "" {
		identifier = externalSub
	}

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

	linkID := id.New()
	claimsJSON, _ := json.Marshal(claims)
	_, err = tx.ExecContext(ctx,
		`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
		 VALUES (?, ?, ?, ?, ?, ?, datetime('now'))`,
		linkID, newID, prov.ID, externalSub, externalEmail, string(claimsJSON),
	)
	if err != nil {
		return "", fmt.Errorf("create linked account: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return "", fmt.Errorf("commit identity create: %w", err)
	}

	return newID, nil
}

func (h *Handler) findLinkableIdentity(ctx context.Context, linking providers.Linking, externalEmail, externalSub string) (string, bool) {
	switch linking.MatchBy {
	case providers.LinkMatchVerifiedEmail:
		if externalEmail == "" {
			return "", false
		}
		var userID string
		err := h.db.SQL().QueryRowContext(ctx, `SELECT id FROM users WHERE identifier = ?`, externalEmail).Scan(&userID)
		return userID, err == nil
	case providers.LinkMatchIdentifier:
		candidate := externalEmail
		if candidate == "" {
			candidate = externalSub
		}
		if candidate == "" {
			return "", false
		}
		var userID string
		err := h.db.SQL().QueryRowContext(ctx, `SELECT id FROM users WHERE identifier = ?`, candidate).Scan(&userID)
		return userID, err == nil
	default:
		return "", false
	}
}

func providerAllowedForConfig(prov providers.Provider, cfg *SchemaAuthConfig) bool {
	if cfg == nil {
		return true
	}
	if cfg.SSOProviderMode == "allowlist" && len(cfg.SSOProviderIDs) > 0 {
		allowed := false
		for _, providerID := range cfg.SSOProviderIDs {
			if providerID == prov.ID {
				allowed = true
				break
			}
		}
		if !allowed {
			return false
		}
	}
	targetSchemaType := cfg.RegistrationSchemaType
	if targetSchemaType == "" {
		targetSchemaType = "human_user"
	}
	return prov.Target.SchemaType == "" || prov.Target.SchemaType == targetSchemaType || prov.Target.SchemaID != ""
}

func stringifyClaim(value any) string {
	switch typed := value.(type) {
	case string:
		return typed
	case float64:
		return fmt.Sprintf("%.0f", typed)
	default:
		return ""
	}
}

func scopeString(value any, protocol string) string {
	switch typed := value.(type) {
	case string:
		if strings.TrimSpace(typed) != "" {
			return typed
		}
	case []any:
		parts := make([]string, 0, len(typed))
		for _, item := range typed {
			if str, ok := item.(string); ok && strings.TrimSpace(str) != "" {
				parts = append(parts, str)
			}
		}
		if len(parts) > 0 {
			return strings.Join(parts, " ")
		}
	}
	if protocol == "oauth2" {
		return "user:email read:user"
	}
	return "openid email profile"
}

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

func exchangeCode(ctx context.Context, tokenEndpoint string, cfg providerConnectionConfig, code, pkceVerifier, redirectURI string) (*tokenResponse, error) {
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

func fetchUserInfo(ctx context.Context, userInfoURL, accessToken string) (map[string]any, error) {
	if strings.TrimSpace(userInfoURL) == "" {
		return nil, fmt.Errorf("userinfo endpoint is required")
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, userInfoURL, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Authorization", "Bearer "+accessToken)
	req.Header.Set("Accept", "application/json")

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("userinfo returned %d: %s", resp.StatusCode, string(body))
	}

	var claims map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&claims); err != nil {
		return nil, err
	}
	return claims, nil
}

func parseIDTokenClaims(idToken string) (map[string]any, error) {
	parts := strings.Split(idToken, ".")
	if len(parts) != 3 {
		return nil, fmt.Errorf("invalid JWT: expected 3 parts, got %d", len(parts))
	}

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
