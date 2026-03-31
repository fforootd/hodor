// Package mockoidc provides an embedded mock OIDC identity provider
// for local development and end-to-end testing of RP federation flows.
//
// It serves both a default issuer and scenario-specific issuers:
//   - GET  /.well-known/openid-configuration
//   - GET  /authorize
//   - POST /authorize
//   - POST /token
//   - GET  /userinfo
//   - GET  /jwks
//
// Scenario-specific issuers are exposed under /scenarios/{name}/...
package mockoidc

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"

	"github.com/zitadel/zitadel/internal/logging"
)

const (
	ScenarioHappyPath                 = "happy-path"
	ScenarioVerifiedEmailExistingUser = "verified-email-existing-user"
	ScenarioLinkOnlyFailure           = "link-only-failure"
	ScenarioUserInfoOnly              = "userinfo-only"
	ScenarioNonceMismatch             = "nonce-mismatch"
	ScenarioTokenFailure              = "token-failure"
	ScenarioAccessDenied              = "access-denied"
)

// Config configures the mock OIDC server.
type Config struct {
	Port     int    // default: 9998
	Issuer   string // auto-derived from port if empty
	Secret   string // HMAC signing secret
	TestUser TestUser
}

// TestUser defines the mock user credentials and claims.
type TestUser struct {
	Sub           string `json:"sub"`
	Email         string `json:"email"`
	Name          string `json:"name"`
	GivenName     string `json:"given_name"`
	FamilyName    string `json:"family_name"`
	Picture       string `json:"picture"`
	EmailVerified bool   `json:"email_verified"`
	Password      string `json:"-"` // not included in claims
}

type scenario struct {
	Name             string
	User             TestUser
	IssueIDToken     bool
	ForceNonce       string
	TokenError       string
	AuthorizeError   string
	AuthorizeErrDesc string
}

type authCode struct {
	Code        string
	Scenario    string
	ClientID    string
	RedirectURI string
	Nonce       string
	Claims      map[string]any
	CreatedAt   time.Time
}

type issuedAccessToken struct {
	Token     string
	Claims    map[string]any
	CreatedAt time.Time
}

// DefaultConfig returns a sensible default configuration.
func DefaultConfig() Config {
	return Config{
		Port:   9998,
		Secret: "mock-oidc-dev-secret-do-not-use-in-production",
		TestUser: TestUser{
			Sub:           "mock-user-001",
			Email:         "mock-rp-user@example.com",
			Name:          "Mock RP User",
			GivenName:     "Mock",
			FamilyName:    "User",
			Picture:       "",
			EmailVerified: true,
			Password:      "password123",
		},
	}
}

// Server is the mock OIDC identity provider.
type Server struct {
	cfg          Config
	issuer       string
	scenarios    map[string]scenario
	codes        map[string]*authCode
	accessTokens map[string]*issuedAccessToken
	mu           sync.Mutex
}

// New creates a new mock OIDC server.
func New(cfg Config) *Server {
	if cfg.Port == 0 {
		cfg.Port = 9998
	}
	if cfg.Issuer == "" {
		cfg.Issuer = fmt.Sprintf("http://127.0.0.1:%d", cfg.Port)
	}
	if cfg.TestUser.Password == "" {
		cfg.TestUser = DefaultConfig().TestUser
	}

	s := &Server{
		cfg:          cfg,
		issuer:       strings.TrimRight(cfg.Issuer, "/"),
		codes:        make(map[string]*authCode),
		accessTokens: make(map[string]*issuedAccessToken),
	}
	s.scenarios = s.defaultScenarios()
	return s
}

// Start starts the mock OIDC server in a goroutine.
func (s *Server) Start() {
	addr := fmt.Sprintf(":%d", s.cfg.Port)
	logging.Printf("[mock-oidc] starting on %s (issuer: %s)", addr, s.issuer)
	logging.Printf("[mock-oidc] default test user: %s / %s", s.cfg.TestUser.Email, s.cfg.TestUser.Password)
	go func() {
		if err := http.ListenAndServe(addr, s.Handler()); err != nil {
			logging.Printf("[mock-oidc] server error: %v", err)
		}
	}()
}

// Handler returns the HTTP handler for the mock provider.
func (s *Server) Handler() http.Handler {
	return http.HandlerFunc(s.handle)
}

// Issuer returns the default issuer URL.
func (s *Server) Issuer() string { return s.issuer }

// ScenarioIssuer returns the issuer URL for a named scenario.
func (s *Server) ScenarioIssuer(name string) string {
	name = strings.TrimSpace(name)
	if name == "" || name == ScenarioHappyPath {
		return s.issuer
	}
	return s.issuer + "/scenarios/" + url.PathEscape(name)
}

// ClientID returns the expected client ID.
func (s *Server) ClientID() string { return "mock-client-id" }

// ClientSecret returns the expected client secret.
func (s *Server) ClientSecret() string { return "mock-client-secret" }

func (s *Server) handle(w http.ResponseWriter, r *http.Request) {
	scenario, basePath, routePath, ok := s.resolveScenario(r.URL.Path)
	if !ok {
		http.NotFound(w, r)
		return
	}

	switch {
	case r.Method == http.MethodGet && routePath == "/.well-known/openid-configuration":
		s.discovery(w, scenario, basePath)
	case r.Method == http.MethodGet && routePath == "/authorize":
		s.authorizeForm(w, r, scenario, basePath)
	case r.Method == http.MethodPost && routePath == "/authorize":
		s.authorizeSubmit(w, r, scenario)
	case r.Method == http.MethodPost && routePath == "/token":
		s.token(w, r, scenario, basePath)
	case r.Method == http.MethodGet && routePath == "/userinfo":
		s.userInfo(w, r)
	case r.Method == http.MethodGet && routePath == "/jwks":
		s.jwks(w)
	default:
		http.NotFound(w, r)
	}
}

func (s *Server) resolveScenario(path string) (scenario, string, string, bool) {
	if strings.HasPrefix(path, "/scenarios/") {
		rest := strings.TrimPrefix(path, "/scenarios/")
		parts := strings.SplitN(rest, "/", 2)
		if len(parts) < 2 || strings.TrimSpace(parts[0]) == "" {
			return scenario{}, "", "", false
		}
		name := parts[0]
		scn, ok := s.scenarios[name]
		if !ok {
			return scenario{}, "", "", false
		}
		return scn, "/scenarios/" + name, "/" + parts[1], true
	}

	return s.scenarios[ScenarioHappyPath], "", path, true
}

func (s *Server) defaultScenarios() map[string]scenario {
	defaultPassword := s.cfg.TestUser.Password
	return map[string]scenario{
		ScenarioHappyPath: {
			Name:         ScenarioHappyPath,
			User:         s.cfg.TestUser,
			IssueIDToken: true,
		},
		ScenarioVerifiedEmailExistingUser: {
			Name: ScenarioVerifiedEmailExistingUser,
			User: TestUser{
				Sub:           "mock-user-existing-001",
				Email:         "e2e-user@example.com",
				Name:          "E2E User",
				GivenName:     "E2E",
				FamilyName:    "User",
				Picture:       "",
				EmailVerified: true,
				Password:      defaultPassword,
			},
			IssueIDToken: true,
		},
		ScenarioLinkOnlyFailure: {
			Name: ScenarioLinkOnlyFailure,
			User: TestUser{
				Sub:           "mock-user-unlinked-001",
				Email:         "unlinked-rp-user@example.com",
				Name:          "Unlinked RP User",
				GivenName:     "Unlinked",
				FamilyName:    "User",
				Picture:       "",
				EmailVerified: true,
				Password:      defaultPassword,
			},
			IssueIDToken: true,
		},
		ScenarioUserInfoOnly: {
			Name: ScenarioUserInfoOnly,
			User: TestUser{
				Sub:           "mock-user-userinfo-001",
				Email:         "userinfo-rp-user@example.com",
				Name:          "Userinfo RP User",
				GivenName:     "Userinfo",
				FamilyName:    "User",
				Picture:       "",
				EmailVerified: true,
				Password:      defaultPassword,
			},
			IssueIDToken: false,
		},
		ScenarioNonceMismatch: {
			Name: ScenarioNonceMismatch,
			User: TestUser{
				Sub:           "mock-user-nonce-001",
				Email:         "nonce-rp-user@example.com",
				Name:          "Nonce RP User",
				GivenName:     "Nonce",
				FamilyName:    "User",
				Picture:       "",
				EmailVerified: true,
				Password:      defaultPassword,
			},
			IssueIDToken: true,
			ForceNonce:   "wrong-nonce",
		},
		ScenarioTokenFailure: {
			Name: ScenarioTokenFailure,
			User: TestUser{
				Sub:           "mock-user-token-error-001",
				Email:         "token-failure-rp-user@example.com",
				Name:          "Token Failure RP User",
				GivenName:     "Token",
				FamilyName:    "Failure",
				Picture:       "",
				EmailVerified: true,
				Password:      defaultPassword,
			},
			IssueIDToken: true,
			TokenError:   "simulated_token_failure",
		},
		ScenarioAccessDenied: {
			Name:             ScenarioAccessDenied,
			User:             s.cfg.TestUser,
			IssueIDToken:     true,
			AuthorizeError:   "access_denied",
			AuthorizeErrDesc: "mock user denied access",
		},
	}
}

func (s *Server) discovery(w http.ResponseWriter, scn scenario, basePath string) {
	issuer := s.issuer + basePath
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]any{
		"issuer":                                issuer,
		"authorization_endpoint":                issuer + "/authorize",
		"token_endpoint":                        issuer + "/token",
		"userinfo_endpoint":                     issuer + "/userinfo",
		"jwks_uri":                              issuer + "/jwks",
		"response_types_supported":              []string{"code"},
		"subject_types_supported":               []string{"public"},
		"id_token_signing_alg_values_supported": []string{"HS256"},
		"scopes_supported":                      []string{"openid", "email", "profile"},
		"code_challenge_methods_supported":      []string{"S256"},
		"mock_scenario":                         scn.Name,
	})
}

func (s *Server) authorizeForm(w http.ResponseWriter, r *http.Request, scn scenario, basePath string) {
	state := r.URL.Query().Get("state")
	redirectURI := r.URL.Query().Get("redirect_uri")
	nonce := r.URL.Query().Get("nonce")
	clientID := r.URL.Query().Get("client_id")

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	fmt.Fprintf(w, `<!DOCTYPE html>
<html><head>
<title>Mock OIDC Sign In</title>
<style>
  body { font-family: ui-sans-serif, system-ui, sans-serif; background: #f4f1ea; color: #18181b;
    display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }
  .card { background: #ffffff; border-radius: 20px; padding: 2rem; width: 380px;
    border: 1px solid #e4ded1; box-shadow: 0 20px 40px rgba(41, 32, 18, 0.08); }
  h1 { margin: 0 0 0.25rem; font-size: 1.4rem; }
  .subtitle { color: #6b6456; font-size: 0.875rem; margin-bottom: 1.5rem; }
  .scenario { display: inline-block; margin-bottom: 1rem; padding: 0.2rem 0.6rem; border-radius: 999px;
    background: #18181b; color: #ffffff; font-size: 0.75rem; }
  label { display: block; font-size: 0.875rem; margin-bottom: 0.35rem; }
  input { width: 100%%; box-sizing: border-box; padding: 0.7rem 0.8rem; border: 1px solid #d6cec0;
    border-radius: 10px; background: #fffdf8; color: inherit; font-size: 0.95rem; margin-bottom: 1rem; }
  button { width: 100%%; padding: 0.8rem 0.9rem; border: none; border-radius: 10px;
    background: #f25543; color: #ffffff; font-size: 0.95rem; font-weight: 700; cursor: pointer; }
  .hint { margin-top: 1rem; font-size: 0.8rem; color: #6b6456; }
</style>
</head><body>
<div class="card">
  <div class="scenario">%s</div>
  <h1>Mock OIDC Provider</h1>
  <p class="subtitle">Sign in to continue to %s</p>
  <form method="POST" action="%s/authorize">
    <input type="hidden" name="state" value="%s">
    <input type="hidden" name="redirect_uri" value="%s">
    <input type="hidden" name="nonce" value="%s">
    <input type="hidden" name="client_id" value="%s">
    <label>Email</label>
    <input name="email" type="email" value="%s" autofocus>
    <label>Password</label>
    <input name="password" type="password" placeholder="%s">
    <button type="submit">Sign in</button>
  </form>
  <p class="hint">Use: %s / %s</p>
</div>
</body></html>`,
		scn.Name,
		clientID,
		basePath,
		state,
		redirectURI,
		nonce,
		clientID,
		scn.User.Email,
		scn.User.Password,
		scn.User.Email,
		scn.User.Password,
	)
}

func (s *Server) authorizeSubmit(w http.ResponseWriter, r *http.Request, scn scenario) {
	_ = r.ParseForm()
	email := r.FormValue("email")
	password := r.FormValue("password")
	state := r.FormValue("state")
	redirectURI := r.FormValue("redirect_uri")
	nonce := r.FormValue("nonce")
	clientID := r.FormValue("client_id")

	if scn.AuthorizeError != "" {
		u, err := url.Parse(redirectURI)
		if err != nil {
			http.Error(w, "invalid redirect_uri", http.StatusBadRequest)
			return
		}
		q := u.Query()
		q.Set("error", scn.AuthorizeError)
		if scn.AuthorizeErrDesc != "" {
			q.Set("error_description", scn.AuthorizeErrDesc)
		}
		q.Set("state", state)
		u.RawQuery = q.Encode()
		http.Redirect(w, r, u.String(), http.StatusFound)
		return
	}

	if email != scn.User.Email || password != scn.User.Password {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		fmt.Fprintf(w, `<html><body><h1>Invalid credentials</h1><p>Expected %s / %s</p><a href="javascript:history.back()">Back</a></body></html>`,
			scn.User.Email, scn.User.Password)
		return
	}

	claims := s.userClaims(scn.User)
	code := fmt.Sprintf("mock-code-%d", time.Now().UnixNano())

	s.mu.Lock()
	s.codes[code] = &authCode{
		Code:        code,
		Scenario:    scn.Name,
		ClientID:    clientID,
		RedirectURI: redirectURI,
		Nonce:       nonce,
		Claims:      claims,
		CreatedAt:   time.Now(),
	}
	s.mu.Unlock()

	u, err := url.Parse(redirectURI)
	if err != nil {
		http.Error(w, "invalid redirect_uri", http.StatusBadRequest)
		return
	}
	q := u.Query()
	q.Set("code", code)
	q.Set("state", state)
	u.RawQuery = q.Encode()

	http.Redirect(w, r, u.String(), http.StatusFound)
}

func (s *Server) token(w http.ResponseWriter, r *http.Request, scn scenario, basePath string) {
	_ = r.ParseForm()
	code := r.FormValue("code")

	s.mu.Lock()
	ac, ok := s.codes[code]
	if ok {
		delete(s.codes, code)
	}
	s.mu.Unlock()

	if !ok {
		s.writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_grant"})
		return
	}

	scenarioFromCode, ok := s.scenarios[ac.Scenario]
	if !ok {
		scenarioFromCode = scn
	}
	if scenarioFromCode.TokenError != "" {
		s.writeJSON(w, http.StatusBadRequest, map[string]string{
			"error":             "invalid_request",
			"error_description": scenarioFromCode.TokenError,
		})
		return
	}

	issuer := s.issuer
	if ac.Scenario != ScenarioHappyPath {
		issuer = s.ScenarioIssuer(ac.Scenario)
	}
	claims := cloneMap(ac.Claims)
	now := time.Now().Unix()
	claims["iss"] = issuer
	claims["aud"] = ac.ClientID
	claims["exp"] = now + 3600
	claims["iat"] = now
	if scenarioFromCode.ForceNonce != "" {
		claims["nonce"] = scenarioFromCode.ForceNonce
	} else {
		claims["nonce"] = ac.Nonce
	}

	accessToken := fmt.Sprintf("mock-access-token-%d", time.Now().UnixNano())
	s.mu.Lock()
	s.accessTokens[accessToken] = &issuedAccessToken{
		Token:     accessToken,
		Claims:    cloneMap(ac.Claims),
		CreatedAt: time.Now(),
	}
	s.mu.Unlock()

	response := map[string]any{
		"access_token": accessToken,
		"token_type":   "Bearer",
		"expires_in":   3600,
	}
	if scenarioFromCode.IssueIDToken {
		response["id_token"] = s.signJWT(claims)
	}
	s.writeJSON(w, http.StatusOK, response)
}

func (s *Server) userInfo(w http.ResponseWriter, r *http.Request) {
	authz := strings.TrimSpace(r.Header.Get("Authorization"))
	if !strings.HasPrefix(authz, "Bearer ") {
		s.writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "missing_bearer_token"})
		return
	}
	accessToken := strings.TrimPrefix(authz, "Bearer ")

	s.mu.Lock()
	token, ok := s.accessTokens[accessToken]
	s.mu.Unlock()
	if !ok {
		s.writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "invalid_token"})
		return
	}

	s.writeJSON(w, http.StatusOK, token.Claims)
}

func (s *Server) jwks(w http.ResponseWriter) {
	s.writeJSON(w, http.StatusOK, map[string]any{"keys": []any{}})
}

func (s *Server) userClaims(user TestUser) map[string]any {
	return map[string]any{
		"sub":            user.Sub,
		"email":          user.Email,
		"email_verified": user.EmailVerified,
		"name":           user.Name,
		"given_name":     user.GivenName,
		"family_name":    user.FamilyName,
		"picture":        user.Picture,
	}
}

func (s *Server) signJWT(claims map[string]any) string {
	header := base64.RawURLEncoding.EncodeToString([]byte(`{"alg":"HS256","typ":"JWT"}`))
	payloadJSON, _ := json.Marshal(claims)
	payload := base64.RawURLEncoding.EncodeToString(payloadJSON)

	signingInput := header + "." + payload
	mac := hmac.New(sha256.New, []byte(s.cfg.Secret))
	mac.Write([]byte(signingInput))
	sig := base64.RawURLEncoding.EncodeToString(mac.Sum(nil))

	return signingInput + "." + sig
}

func (s *Server) writeJSON(w http.ResponseWriter, status int, body any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(body)
}

func cloneMap(value map[string]any) map[string]any {
	if value == nil {
		return nil
	}
	out := make(map[string]any, len(value))
	for key, item := range value {
		out[key] = item
	}
	return out
}
