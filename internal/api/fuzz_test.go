package api_test

import (
	"bytes"
	"net/http"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

// FuzzAPIJSON sends fuzzed JSON bodies to key API endpoints.
// The goal is to verify the server never panics on malformed input.
func FuzzAPIJSON(f *testing.F) {
	// Seed corpus with common attack patterns.
	f.Add(`{"identifier":"test"}`)
	f.Add(`{}`)
	f.Add(`null`)
	f.Add(`[]`)
	f.Add(`{"a":"` + string(make([]byte, 10000)) + `"}`)
	f.Add(`{"nested":{"deep":{"level":true}}}`)
	f.Add(`{"identifier":12345}`)
	f.Add(`{"identifier":null,"display_name":false}`)
	f.Add("")
	f.Add(`{"\x00":"\xff"}`)

	f.Fuzz(func(t *testing.T, body string) {
		srv := testutil.NewTestServer(t)
		token := srv.LoginAdmin()

		endpoints := []struct {
			method string
			path   string
		}{
			{"POST", "/v1/users"},
			{"POST", "/v1/schemas"},
			{"POST", "/v1/import"},
			{"POST", "/v1/sessions"},
		}

		for _, ep := range endpoints {
			req, _ := http.NewRequest(ep.method, srv.URL()+ep.path, bytes.NewReader([]byte(body)))
			req.Header.Set("Content-Type", "application/json")
			req.Header.Set("Authorization", "Bearer "+token)

			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				continue // Network-level errors are fine.
			}
			resp.Body.Close()

			// Must never return 5xx — that indicates a panic or unhandled error.
			if resp.StatusCode >= 500 {
				t.Errorf("%s %s returned %d for body: %q", ep.method, ep.path, resp.StatusCode, truncate(body, 100))
			}
		}
	})
}

// FuzzSessionToken tests the session validation middleware with fuzzed Bearer values.
func FuzzSessionToken(f *testing.F) {
	f.Add("")
	f.Add("valid-looking-hex-token-0123456789abcdef")
	f.Add("a")
	f.Add(string(make([]byte, 10000)))
	f.Add("DROP TABLE sessions;")
	f.Add("' OR 1=1 --")
	f.Add("../../../etc/passwd")
	f.Add("zit_ses_" + string(make([]byte, 64)))
	f.Add("zit_pat_" + string(make([]byte, 64)))

	f.Fuzz(func(t *testing.T, token string) {
		srv := testutil.NewTestServer(t)

		req, _ := http.NewRequest("GET", srv.URL()+"/v1/sessions", nil)
		req.Header.Set("Authorization", "Bearer "+token)

		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			return // Network errors are fine.
		}
		resp.Body.Close()

		// Must never panic. 401/403 are expected for bad tokens.
		if resp.StatusCode >= 500 {
			t.Errorf("server returned %d for token: %q", resp.StatusCode, truncate(token, 100))
		}
	})
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}

// FuzzBearerTokenResolution fuzz tests the Authorization: Bearer header parsing.
func FuzzBearerTokenResolution(f *testing.F) {
	f.Add("zit_ses_valid")
	f.Add("zit_pat_valid")
	f.Add("zit_opq_valid")
	f.Add("Bearer token") // Double-Bearer
	f.Add("' OR 1=1 --")
	f.Add("../../../etc/passwd")
	f.Add(string(make([]byte, 5000)))

	f.Fuzz(func(t *testing.T, token string) {
		srv := testutil.NewTestServer(t)

		req, err := http.NewRequest("GET", srv.URL()+"/v1/account/profile", nil)
		if err != nil {
			return // malformed URL is fine
		}
		req.Header["Authorization"] = []string{"Bearer " + token}

		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			return // transport-level errors (e.g. invalid header bytes) are fine
		}
		resp.Body.Close()

		if resp.StatusCode >= 500 {
			t.Errorf("5xx for bearer %q: %d", truncate(token, 50), resp.StatusCode)
		}
	})
}

// FuzzCookieTokenResolution fuzz tests cookie-based auth through the AuthGate.
func FuzzCookieTokenResolution(f *testing.F) {
	f.Add("raw-unsigned")
	f.Add("base64.garbage")
	f.Add("")
	f.Add(string(make([]byte, 5000)))

	f.Fuzz(func(t *testing.T, cookie string) {
		srv := testutil.NewTestServer(t)

		req, _ := http.NewRequest("GET", srv.URL()+"/v1/account/profile", nil)
		req.AddCookie(&http.Cookie{Name: "__zitadel_session", Value: cookie})

		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			return
		}
		resp.Body.Close()

		if resp.StatusCode >= 500 {
			t.Errorf("5xx for cookie %q: %d", truncate(cookie, 50), resp.StatusCode)
		}
	})
}

// FuzzXIdentityIdHeader ensures the X-Identity-Id header cannot be injected.
func FuzzXIdentityIdHeader(f *testing.F) {
	f.Add("1")
	f.Add("99999")
	f.Add("0")
	f.Add("-1")
	f.Add("abc")
	f.Add("' OR 1=1 --")

	f.Fuzz(func(t *testing.T, headerVal string) {
		srv := testutil.NewTestServer(t)

		req, _ := http.NewRequest("GET", srv.URL()+"/v1/account/profile", nil)
		req.Header.Set("X-Identity-Id", headerVal)

		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			return
		}
		resp.Body.Close()

		// Without valid auth, the injected header should be ignored → 401.
		if resp.StatusCode >= 500 {
			t.Errorf("5xx for X-Identity-Id %q: %d", headerVal, resp.StatusCode)
		}
		if resp.StatusCode == 200 {
			t.Errorf("injected X-Identity-Id %q returned 200 — auth bypass!", headerVal)
		}
	})
}
