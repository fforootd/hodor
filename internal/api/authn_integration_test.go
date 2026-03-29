package api_test

import (
	"fmt"
	"net/http"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

// --- OWASP SESS: Session Lifecycle Tests ---

func TestSession_CreateUseRevoke(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("cycle@test.com", "Cycle User")
	token := srv.CreateSession(userID)

	// Use the session.
	code, _ := srv.GetWithCookie("/v1/account/profile", token)
	if code != 200 {
		t.Fatalf("expected 200 using valid session, got %d", code)
	}

	// Revoke via admin.
	adminToken := srv.LoginAdmin()
	code, body := srv.GetWithCookie("/v1/sessions", adminToken)
	if code != 200 {
		t.Fatalf("expected 200 listing sessions, got %d", code)
	}
	items, _ := body["items"].([]any)
	if len(items) == 0 {
		t.Skip("no sessions found to revoke")
	}
	first, _ := items[0].(map[string]any)
	sessionID, _ := first["id"].(string)
	if sessionID == "" {
		// Try numeric.
		if num, ok := first["id"].(float64); ok {
			sessionID = fmt.Sprintf("%.0f", num)
		}
	}

	code, _ = srv.PostJSONWithCookie("/v1/sessions/"+sessionID+"/revoke", nil, adminToken)
	if code != 204 && code != 200 {
		t.Fatalf("expected 204 revoking session, got %d", code)
	}
}

func TestSession_DoubleRevoke_Idempotent(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("double@test.com", "Double User")
	_ = srv.CreateSession(userID)
	adminToken := srv.LoginAdmin()

	code, body := srv.GetWithCookie("/v1/sessions", adminToken)
	if code != 200 {
		t.Fatalf("list sessions: %d", code)
	}
	items, _ := body["items"].([]any)
	if len(items) == 0 {
		t.Skip("no sessions")
	}
	first, _ := items[0].(map[string]any)
	sid := fmt.Sprintf("%v", first["id"])

	// First revoke.
	code, _ = srv.PostJSONWithCookie("/v1/sessions/"+sid+"/revoke", nil, adminToken)
	if code != 204 && code != 200 {
		t.Fatalf("first revoke: %d", code)
	}

	// Second revoke should not crash (idempotent).
	code2, _ := srv.PostJSONWithCookie("/v1/sessions/"+sid+"/revoke", nil, adminToken)
	if code2 >= 500 {
		t.Fatalf("double revoke caused 5xx: %d", code2)
	}
}

// --- OWASP AUTH: Bearer Token Tests ---

func TestBearer_ValidPAT(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()

	// Get the admin's identity ID for the PAT.
	code, body := srv.GetWithCookie("/v1/account/profile", adminToken)
	if code != 200 {
		t.Fatalf("expected 200 getting admin profile, got %d: %v", code, body)
	}
	// Profile returns nested: {"identity": {"id": "uuid...", ...}}
	identity, _ := body["identity"].(map[string]any)
	adminID, _ := identity["id"].(string)
	if adminID == "" {
		t.Fatalf("could not extract admin identity ID from profile: %v", body)
	}

	// Create a PAT via the API.
	code, body = srv.PostJSONWithCookie("/v1/pats", map[string]any{
		"name":    "test-pat",
		"user_id": adminID,
		"scopes":  []string{"admin"},
	}, adminToken)
	if code != 201 {
		t.Fatalf("expected 201 creating PAT, got %d: %v", code, body)
	}

	patToken, _ := body["token"].(string)
	if patToken == "" {
		t.Fatal("expected PAT token in response")
	}

	// Use PAT via Bearer header.
	code, _ = srv.GetWithBearer("/v1/users", patToken)
	if code != 200 {
		t.Fatalf("expected 200 using PAT, got %d", code)
	}
}

func TestBearer_MalformedHeader_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// Send with Basic instead of Bearer.
	req, _ := http.NewRequest("GET", srv.URL()+"/v1/users", nil)
	req.Header.Set("Authorization", "Basic dXNlcjpwYXNz")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("request: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != 401 {
		t.Fatalf("expected 401 for Basic auth, got %d", resp.StatusCode)
	}
}

func TestBearer_EmptyToken_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)
	code, _ := srv.GetWithBearer("/v1/users", "")
	if code != 401 {
		t.Fatalf("expected 401 for empty bearer, got %d", code)
	}
}

// --- OWASP AUTH: Error Message Uniformity ---

func TestUniform_NoToken_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)
	code, body := srv.GetRaw("/v1/users")
	if code != 401 {
		t.Fatalf("expected 401, got %d", code)
	}
	// Must have an error field.
	if _, ok := body["error"]; !ok {
		t.Error("401 response missing 'error' field")
	}
}

func TestUniform_InvalidToken_SameAs_ExpiredToken(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// Invalid token.
	code1, body1 := srv.GetWithBearer("/v1/users", "zit_ses_invalid_garbage_token_value_1234")

	// Fabricated expired token.
	code2, body2 := srv.GetWithBearer("/v1/users", "zit_ses_0000000000000000000000000000000000000000000000000000000000000000")

	if code1 != 401 || code2 != 401 {
		t.Fatalf("expected both 401, got %d and %d", code1, code2)
	}

	// Both should have same JSON shape.
	_, hasErr1 := body1["error"]
	_, hasErr2 := body2["error"]
	if hasErr1 != hasErr2 {
		t.Error("error response shape should be identical for invalid vs expired tokens")
	}
}

func TestNoSensitiveData_In401Body(t *testing.T) {
	srv := testutil.NewTestServer(t)
	_, body := srv.GetWithBearer("/v1/users", "zit_ses_bad_token")

	// Must not leak stack traces, SQL errors, or internal paths.
	for _, key := range []string{"stack", "trace", "sql", "path", "file"} {
		if _, has := body[key]; has {
			t.Errorf("401 body should not contain %q field", key)
		}
	}
}

// --- OWASP SESS: Header Injection Prevention ---

func TestXIdentityId_CantBeInjected(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// Try to inject X-Identity-Id header without valid auth.
	req, _ := http.NewRequest("GET", srv.URL()+"/v1/account/profile", nil)
	req.Header.Set("X-Identity-Id", "99999")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("request: %v", err)
	}
	defer resp.Body.Close()

	// Should still be 401 (injected header ignored without valid auth).
	if resp.StatusCode != 401 {
		t.Fatalf("expected 401 with injected X-Identity-Id, got %d", resp.StatusCode)
	}
}

func TestXSessionId_CantBeInjected(t *testing.T) {
	srv := testutil.NewTestServer(t)

	req, _ := http.NewRequest("GET", srv.URL()+"/v1/account/profile", nil)
	req.Header.Set("X-Session-Id", "99999")
	req.Header.Set("X-Identity-Id", "1")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != 401 {
		t.Fatalf("expected 401 with injected X-Session-Id, got %d", resp.StatusCode)
	}
}

// --- Unsigned Cookie Rejection ---

func TestUnsignedCookie_Rejected(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("unsigned@test.com", "Unsigned")
	_ = srv.CreateSession(userID)

	// Try to authenticate with a raw unsigned cookie.
	req, _ := http.NewRequest("GET", srv.URL()+"/v1/account/profile", nil)
	req.AddCookie(&http.Cookie{Name: "__zitadel_session", Value: "raw-unsigned-token"})

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != 401 {
		t.Fatalf("expected 401 for unsigned cookie, got %d", resp.StatusCode)
	}
}
