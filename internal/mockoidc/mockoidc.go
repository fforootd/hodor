// Package mockoidc provides an embedded mock OIDC identity provider
// for local development and testing of the SSO flow.
//
// It serves:
//   - GET  /.well-known/openid-configuration → OIDC discovery
//   - GET  /authorize → login form
//   - POST /authorize → validate credentials, redirect with code
//   - POST /token    → exchange code for ID token
//
// The mock IDP uses a hardcoded test user and signs JWTs with HMAC-SHA256.
package mockoidc

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"net/url"
	"sync"
	"time"
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
	Sub        string `json:"sub"`
	Email      string `json:"email"`
	Name       string `json:"name"`
	GivenName  string `json:"given_name"`
	FamilyName string `json:"family_name"`
	Picture    string `json:"picture"`
	Password   string `json:"-"` // not included in claims
}

// DefaultConfig returns a sensible default configuration.
func DefaultConfig() Config {
	return Config{
		Port:   9998,
		Secret: "mock-oidc-dev-secret-do-not-use-in-production",
		TestUser: TestUser{
			Sub:        "mock-user-001",
			Email:      "testuser@example.com",
			Name:       "Test User",
			GivenName:  "Test",
			FamilyName: "User",
			Picture:    "",
			Password:   "password",
		},
	}
}

// Server is the mock OIDC identity provider.
type Server struct {
	cfg    Config
	issuer string
	codes  map[string]*authCode // state → code info
	mu     sync.Mutex
}

type authCode struct {
	code        string
	redirectURI string
	nonce       string
	createdAt   time.Time
}

// New creates a new mock OIDC server.
func New(cfg Config) *Server {
	if cfg.Port == 0 {
		cfg.Port = 9998
	}
	if cfg.Issuer == "" {
		cfg.Issuer = fmt.Sprintf("http://localhost:%d", cfg.Port)
	}
	return &Server{
		cfg:    cfg,
		issuer: cfg.Issuer,
		codes:  make(map[string]*authCode),
	}
}

// Start starts the mock OIDC server in a goroutine.
func (s *Server) Start() {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /.well-known/openid-configuration", s.discovery)
	mux.HandleFunc("GET /authorize", s.authorizeForm)
	mux.HandleFunc("POST /authorize", s.authorizeSubmit)
	mux.HandleFunc("POST /token", s.token)

	addr := fmt.Sprintf(":%d", s.cfg.Port)
	logging.Printf("[mock-oidc] starting on %s (issuer: %s)", addr, s.issuer)
	logging.Printf("[mock-oidc] test user: %s / %s", s.cfg.TestUser.Email, s.cfg.TestUser.Password)
	go func() {
		if err := http.ListenAndServe(addr, mux); err != nil {
			logging.Printf("[mock-oidc] server error: %v", err)
		}
	}()
}

// Issuer returns the issuer URL.
func (s *Server) Issuer() string { return s.issuer }

// ClientID returns the expected client ID.
func (s *Server) ClientID() string { return "mock-client-id" }

// ClientSecret returns the expected client secret.
func (s *Server) ClientSecret() string { return "mock-client-secret" }

// --- Discovery ---

func (s *Server) discovery(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]any{
		"issuer":                                s.issuer,
		"authorization_endpoint":                s.issuer + "/authorize",
		"token_endpoint":                        s.issuer + "/token",
		"userinfo_endpoint":                     s.issuer + "/userinfo",
		"jwks_uri":                              s.issuer + "/jwks",
		"response_types_supported":              []string{"code"},
		"subject_types_supported":               []string{"public"},
		"id_token_signing_alg_values_supported": []string{"HS256"},
		"scopes_supported":                      []string{"openid", "email", "profile"},
		"code_challenge_methods_supported":      []string{"S256"},
	})
}

// --- Authorize ---

func (s *Server) authorizeForm(w http.ResponseWriter, r *http.Request) {
	state := r.URL.Query().Get("state")
	redirectURI := r.URL.Query().Get("redirect_uri")
	nonce := r.URL.Query().Get("nonce")
	clientID := r.URL.Query().Get("client_id")

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	fmt.Fprintf(w, `<!DOCTYPE html>
<html><head>
<title>Mock OIDC — Sign In</title>
<style>
  body { font-family: Inter, system-ui, sans-serif; background: #1a1a2e; color: #e0e0e0;
    display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }
  .card { background: #2d2d44; border-radius: 16px; padding: 2rem; width: 360px;
    box-shadow: 0 8px 32px rgba(0,0,0,.3); }
  h2 { margin: 0 0 0.25rem; color: #fff; font-size: 1.25rem; }
  .subtitle { color: #9ca3af; font-size: 0.8125rem; margin-bottom: 1.5rem; }
  label { display: block; font-size: 0.8125rem; color: #9ca3af; margin-bottom: 0.25rem; }
  input { width: 100%%; box-sizing: border-box; padding: 0.625rem; border: 1px solid #404060;
    border-radius: 8px; background: #1a1a2e; color: #fff; font-size: 0.875rem;
    margin-bottom: 1rem; font-family: inherit; }
  input:focus { outline: none; border-color: #6366f1; }
  button { width: 100%%; padding: 0.75rem; border: none; border-radius: 8px;
    background: #6366f1; color: #fff; font-size: 0.875rem; font-weight: 600;
    cursor: pointer; font-family: inherit; }
  button:hover { background: #5558e6; }
  .badge { display: inline-block; background: #6366f1; padding: 0.125rem 0.5rem;
    border-radius: 4px; font-size: 0.625rem; color: #fff; margin-left: 0.5rem; }
  .hint { text-align: center; margin-top: 1rem; font-size: 0.75rem; color: #6b7280; }
</style>
</head><body>
<div class="card">
  <h2>Mock OIDC Provider <span class="badge">DEV</span></h2>
  <p class="subtitle">Sign in to continue to %s</p>
  <form method="POST" action="/authorize">
    <input type="hidden" name="state" value="%s">
    <input type="hidden" name="redirect_uri" value="%s">
    <input type="hidden" name="nonce" value="%s">
    <label>Email</label>
    <input name="email" type="email" value="%s" autofocus>
    <label>Password</label>
    <input name="password" type="password" placeholder="password">
    <button type="submit">Sign in</button>
  </form>
  <p class="hint">Use: %s / %s</p>
</div>
</body></html>`,
		clientID, state, redirectURI, nonce,
		s.cfg.TestUser.Email,
		s.cfg.TestUser.Email, s.cfg.TestUser.Password,
	)
}

func (s *Server) authorizeSubmit(w http.ResponseWriter, r *http.Request) {
	_ = r.ParseForm()
	email := r.FormValue("email")
	password := r.FormValue("password")
	state := r.FormValue("state")
	redirectURI := r.FormValue("redirect_uri")
	nonce := r.FormValue("nonce")

	if email != s.cfg.TestUser.Email || password != s.cfg.TestUser.Password {
		w.Header().Set("Content-Type", "text/html")
		fmt.Fprintf(w, `<html><body><h1>Invalid credentials</h1><p>Expected %s / %s</p><a href="javascript:history.back()">Back</a></body></html>`,
			s.cfg.TestUser.Email, s.cfg.TestUser.Password)
		return
	}

	// Generate authorization code.
	code := fmt.Sprintf("mock-code-%d", time.Now().UnixNano())

	s.mu.Lock()
	s.codes[code] = &authCode{
		code:        code,
		redirectURI: redirectURI,
		nonce:       nonce,
		createdAt:   time.Now(),
	}
	s.mu.Unlock()

	// Redirect back to Zitadel with code + state.
	u, _ := url.Parse(redirectURI)
	q := u.Query()
	q.Set("code", code)
	q.Set("state", state)
	u.RawQuery = q.Encode()

	http.Redirect(w, r, u.String(), http.StatusFound)
}

// --- Token ---

func (s *Server) token(w http.ResponseWriter, r *http.Request) {
	_ = r.ParseForm()
	code := r.FormValue("code")

	s.mu.Lock()
	ac, ok := s.codes[code]
	if ok {
		delete(s.codes, code)
	}
	s.mu.Unlock()

	if !ok {
		w.WriteHeader(400)
		_ = json.NewEncoder(w).Encode(map[string]string{"error": "invalid_grant"})
		return
	}

	// Build ID token (JWT with HMAC-SHA256).
	now := time.Now().Unix()
	claims := map[string]any{
		"iss":         s.issuer,
		"sub":         s.cfg.TestUser.Sub,
		"aud":         s.ClientID(),
		"exp":         now + 3600,
		"iat":         now,
		"nonce":       ac.nonce,
		"email":       s.cfg.TestUser.Email,
		"name":        s.cfg.TestUser.Name,
		"given_name":  s.cfg.TestUser.GivenName,
		"family_name": s.cfg.TestUser.FamilyName,
		"picture":     s.cfg.TestUser.Picture,
	}

	idToken := s.signJWT(claims)

	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]any{
		"access_token": "mock-access-token",
		"token_type":   "Bearer",
		"expires_in":   3600,
		"id_token":     idToken,
	})
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
