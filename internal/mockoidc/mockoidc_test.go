package mockoidc

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil/httptestutil"
)

func newTestServer(t *testing.T) (*Server, *httptest.Server) {
	t.Helper()

	srv := New(DefaultConfig())
	ts := httptestutil.NewServer(t, srv.Handler())
	srv.issuer = ts.URL
	return srv, ts
}

func TestScenarioDiscoveryUsesScenarioIssuer(t *testing.T) {
	srv, ts := newTestServer(t)
	defer ts.Close()

	resp, err := http.Get(srv.ScenarioIssuer(ScenarioUserInfoOnly) + "/.well-known/openid-configuration")
	if err != nil {
		t.Fatalf("discovery request failed: %v", err)
	}
	defer resp.Body.Close()

	var body map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&body); err != nil {
		t.Fatalf("decode discovery: %v", err)
	}

	if got, want := body["issuer"], srv.ScenarioIssuer(ScenarioUserInfoOnly); got != want {
		t.Fatalf("issuer = %v, want %v", got, want)
	}
	if got, want := body["authorization_endpoint"], srv.ScenarioIssuer(ScenarioUserInfoOnly)+"/authorize"; got != want {
		t.Fatalf("authorization_endpoint = %v, want %v", got, want)
	}
}

func TestUserInfoOnlyScenarioOmitsIDTokenAndServesUserInfo(t *testing.T) {
	srv, ts := newTestServer(t)
	defer ts.Close()

	client := &http.Client{
		CheckRedirect: func(_ *http.Request, _ []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}

	form := url.Values{
		"email":        {"userinfo-rp-user@example.com"},
		"password":     {srv.cfg.TestUser.Password},
		"state":        {"test-state"},
		"redirect_uri": {"http://127.0.0.1/callback"},
		"nonce":        {"expected-nonce"},
		"client_id":    {"mock-client-id"},
	}
	resp, err := client.PostForm(srv.ScenarioIssuer(ScenarioUserInfoOnly)+"/authorize", form)
	if err != nil {
		t.Fatalf("authorize submit failed: %v", err)
	}
	defer resp.Body.Close()

	location := resp.Header.Get("Location")
	if location == "" {
		t.Fatal("expected authorize response to include redirect location")
	}
	callbackURL, err := url.Parse(location)
	if err != nil {
		t.Fatalf("parse callback URL: %v", err)
	}
	code := callbackURL.Query().Get("code")
	if code == "" {
		t.Fatal("expected authorization code in redirect")
	}

	tokenResp, err := client.PostForm(srv.ScenarioIssuer(ScenarioUserInfoOnly)+"/token", url.Values{
		"grant_type":    {"authorization_code"},
		"code":          {code},
		"redirect_uri":  {"http://127.0.0.1/callback"},
		"client_id":     {"mock-client-id"},
		"code_verifier": {"verifier"},
	})
	if err != nil {
		t.Fatalf("token request failed: %v", err)
	}
	defer tokenResp.Body.Close()

	var tokenBody map[string]any
	if err := json.NewDecoder(tokenResp.Body).Decode(&tokenBody); err != nil {
		t.Fatalf("decode token response: %v", err)
	}
	if _, ok := tokenBody["id_token"]; ok {
		t.Fatalf("expected userinfo-only scenario to omit id_token, got %#v", tokenBody["id_token"])
	}

	req, err := http.NewRequest(http.MethodGet, srv.ScenarioIssuer(ScenarioUserInfoOnly)+"/userinfo", nil)
	if err != nil {
		t.Fatalf("build userinfo request: %v", err)
	}
	req.Header.Set("Authorization", "Bearer "+tokenBody["access_token"].(string))

	userInfoResp, err := client.Do(req)
	if err != nil {
		t.Fatalf("userinfo request failed: %v", err)
	}
	defer userInfoResp.Body.Close()

	var claims map[string]any
	if err := json.NewDecoder(userInfoResp.Body).Decode(&claims); err != nil {
		t.Fatalf("decode userinfo: %v", err)
	}
	if got, want := claims["email"], "userinfo-rp-user@example.com"; got != want {
		t.Fatalf("userinfo email = %v, want %v", got, want)
	}
	if got, want := claims["email_verified"], true; got != want {
		t.Fatalf("userinfo email_verified = %v, want %v", got, want)
	}
}

func TestAccessDeniedScenarioRedirectsWithProviderError(t *testing.T) {
	srv, ts := newTestServer(t)
	defer ts.Close()

	client := &http.Client{
		CheckRedirect: func(_ *http.Request, _ []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}

	resp, err := client.PostForm(srv.ScenarioIssuer(ScenarioAccessDenied)+"/authorize", url.Values{
		"email":        {srv.cfg.TestUser.Email},
		"password":     {srv.cfg.TestUser.Password},
		"state":        {"denied-state"},
		"redirect_uri": {"http://127.0.0.1/callback"},
		"client_id":    {"mock-client-id"},
	})
	if err != nil {
		t.Fatalf("authorize submit failed: %v", err)
	}
	defer resp.Body.Close()

	location := resp.Header.Get("Location")
	if !strings.Contains(location, "error=access_denied") {
		t.Fatalf("expected access_denied redirect, got %q", location)
	}
	if !strings.Contains(location, "state=denied-state") {
		t.Fatalf("expected state to round-trip, got %q", location)
	}
}
