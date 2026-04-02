package login

import (
	"encoding/json"
	"net/http"
	"net/url"
	"strings"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
)

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

	instanceID := httputil.InstanceIDFromContext(r.Context())
	_, _ = h.db.SQL().ExecContext(r.Context(),
		`INSERT INTO auth_states (id, instance_id, type, state, provider_id, pkce_verifier, nonce, redirect_uri, data, expires_at, created_at)
		 VALUES (?, ?, 'sso', ?, ?, ?, ?, ?, ?, datetime('now', '+10 minutes'), datetime('now'))`,
		state, instanceID, state, providerID, pkceVerifier, nonce, h.baseURL+"/v1/auth/sso/callback", string(stateDataJSON),
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

	instanceID := httputil.InstanceIDFromContext(r.Context())
	var providerID, pkceVerifier, nonce, stateDataJSON string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT provider_id, pkce_verifier, nonce, COALESCE(data,'{}') FROM auth_states WHERE state = ? AND instance_id = ?`, state, instanceID,
	).Scan(&providerID, &pkceVerifier, &nonce, &stateDataJSON)
	if err != nil {
		logging.Printf("[sso] state lookup failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_expired", http.StatusFound)
		return
	}
	_, _ = h.db.SQL().ExecContext(r.Context(), `DELETE FROM auth_states WHERE state = ? AND instance_id = ?`, state, instanceID)

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
	redirectURI, err := h.ssoSuccessRedirect(r.Context(), r, flowID, userID)
	if err != nil {
		logging.Printf("[sso] success redirect resolution failed: %v", err)
		http.Redirect(w, r, "/login?error=sso_complete", http.StatusFound)
		return
	}
	http.Redirect(w, r, redirectURI, http.StatusFound)
}
