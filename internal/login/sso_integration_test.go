package login_test

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/testutil"
)

func TestSSOStart_RedirectsToProviderAuthorizationURL(t *testing.T) {
	ts := testutil.NewTestServer(t)
	adminToken := ts.LoginAdmin()

	status, body := ts.PostJSONWithBearer("/v1/providers", map[string]any{
		"display_name": "GitHub",
		"kind":         "github",
		"protocol":     "oauth2",
		"connection": map[string]any{
			"authorization_url": "https://provider.example.com/oauth/authorize",
			"token_url":         "https://provider.example.com/oauth/token",
			"userinfo_url":      "https://provider.example.com/api/user",
			"client_id":         "client-123",
			"scopes":            []string{"read:user", "user:email"},
		},
	}, adminToken)
	if status != http.StatusCreated {
		t.Fatalf("create provider status = %d body=%#v", status, body)
	}
	providerID := fmt.Sprintf("%v", body["id"])

	client := &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	resp, err := client.Get(ts.URL() + "/v1/auth/sso/" + providerID + "/start")
	if err != nil {
		t.Fatalf("GET /start: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusFound {
		t.Fatalf("status = %d, want 302", resp.StatusCode)
	}
	location := resp.Header.Get("Location")
	parsed, err := url.Parse(location)
	if err != nil {
		t.Fatalf("parse Location: %v", err)
	}
	if parsed.Scheme != "https" || parsed.Host != "provider.example.com" {
		t.Fatalf("redirect host = %s://%s, want provider.example.com", parsed.Scheme, parsed.Host)
	}
	if got := parsed.Query().Get("client_id"); got != "client-123" {
		t.Fatalf("client_id = %q, want client-123", got)
	}
	if got := parsed.Query().Get("scope"); got != "read:user user:email" {
		t.Fatalf("scope = %q", got)
	}
	if got := parsed.Query().Get("redirect_uri"); got != "http://localhost:8080/v1/auth/sso/callback" {
		t.Fatalf("redirect_uri = %q", got)
	}
}

func TestSSOCallback_CreatesLinkedIdentityAndSession(t *testing.T) {
	ts := testutil.NewTestServer(t)
	adminToken := ts.LoginAdmin()

	providerServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/token":
			w.Header().Set("Content-Type", "application/json")
			fmt.Fprint(w, `{"access_token":"access-123","token_type":"Bearer","expires_in":3600}`)
		case "/userinfo":
			w.Header().Set("Content-Type", "application/json")
			fmt.Fprint(w, `{"sub":"external-123","email":"sso-user@example.com","email_verified":true}`)
		default:
			http.NotFound(w, r)
		}
	}))
	defer providerServer.Close()

	status, body := ts.PostJSONWithBearer("/v1/providers", map[string]any{
		"display_name": "GitHub",
		"kind":         "github",
		"protocol":     "oauth2",
		"connection": map[string]any{
			"authorization_url": providerServer.URL + "/authorize",
			"token_url":         providerServer.URL + "/token",
			"userinfo_url":      providerServer.URL + "/userinfo",
			"client_id":         "client-123",
			"client_secret":     "secret-123",
		},
	}, adminToken)
	if status != http.StatusCreated {
		t.Fatalf("create provider status = %d body=%#v", status, body)
	}
	providerID := fmt.Sprintf("%v", body["id"])

	state := id.New()
	_, err := ts.DB.SQL().Exec(
		`INSERT INTO auth_states (id, type, state, provider_id, pkce_verifier, nonce, redirect_uri, data, expires_at, created_at)
		 VALUES (?, 'sso', ?, ?, ?, '', ?, '{}', ?, ?)`,
		state,
		state,
		providerID,
		"verifier-123",
		ts.URL()+"/v1/auth/sso/callback",
		time.Now().UTC().Add(10*time.Minute).Format("2006-01-02 15:04:05"),
		time.Now().UTC().Format("2006-01-02 15:04:05"),
	)
	if err != nil {
		t.Fatalf("insert auth state: %v", err)
	}

	client := &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	resp, err := client.Get(ts.URL() + "/v1/auth/sso/callback?code=code-123&state=" + url.QueryEscape(state))
	if err != nil {
		t.Fatalf("GET /callback: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusFound {
		t.Fatalf("status = %d, want 302", resp.StatusCode)
	}
	if location := resp.Header.Get("Location"); location != "/login?exit=sso_success" {
		t.Fatalf("Location = %q, want login success redirect", location)
	}

	setCookie := strings.Join(resp.Header.Values("Set-Cookie"), ";")
	if !strings.Contains(setCookie, "zitadel_session=") {
		t.Fatalf("Set-Cookie = %q, want session cookie", setCookie)
	}

	var linkedUserID string
	if err := ts.DB.SQL().QueryRow(`SELECT user_id FROM linked_identities WHERE provider_id = ? AND external_sub = ?`, providerID, "external-123").Scan(&linkedUserID); err != nil {
		t.Fatalf("query linked identity: %v", err)
	}
	if linkedUserID == "" {
		t.Fatal("linked user id is empty")
	}

	var sessionCount int
	if err := ts.DB.SQL().QueryRow(`SELECT COUNT(*) FROM sessions WHERE user_id = ?`, linkedUserID).Scan(&sessionCount); err != nil {
		t.Fatalf("query sessions: %v", err)
	}
	if sessionCount == 0 {
		t.Fatal("expected session for SSO-created user")
	}
}

func TestSSOCallback_RejectsProviderError(t *testing.T) {
	ts := testutil.NewTestServer(t)
	client := &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}

	resp, err := client.Get(ts.URL() + "/v1/auth/sso/callback?error=access_denied&error_description=nope")
	if err != nil {
		t.Fatalf("GET /callback error flow: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusFound {
		t.Fatalf("status = %d, want 302", resp.StatusCode)
	}
	if got := resp.Header.Get("Location"); got != "/login?error=sso_failed" {
		t.Fatalf("Location = %q, want /login?error=sso_failed", got)
	}
}
