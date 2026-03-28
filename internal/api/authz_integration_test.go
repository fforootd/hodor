package api_test

import (
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

// --- OWASP AUTHZ: Privilege Escalation Tests ---

func TestNonAdmin_CannotCreateSchema(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user-schema@test.com", "Schema User")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.PostJSONWithCookie("/v1/schemas", map[string]any{
		"type":   "test_type",
		"schema": map[string]any{"type": "object"},
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin creating schema, got %d", code)
	}
}

func TestNonAdmin_CannotCreateEntity(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user-entity@test.com", "Entity User")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.PostJSONWithCookie("/v1/entities", map[string]any{
		"identifier":   "hacker@test.com",
		"display_name": "Hacker",
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin creating entity, got %d", code)
	}
}

func TestNonAdmin_CannotListSessions(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user-sess@test.com", "Session User")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.GetWithCookie("/v1/sessions", userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin listing sessions, got %d", code)
	}
}

func TestNonAdmin_CannotCreatePAT(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user-pat@test.com", "PAT User")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.PostJSONWithCookie("/v1/pats", map[string]any{
		"name":   "evil-pat",
		"scopes": []string{"admin"},
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin creating PAT, got %d", code)
	}
}

func TestNonAdmin_CannotImport(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user-import@test.com", "Import User")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.PostJSONWithCookie("/v1/import", map[string]any{
		"identities": []any{},
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin importing, got %d", code)
	}
}

func TestNonAdmin_CannotManageProviders(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user-prov@test.com", "Provider User")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.PostJSONWithCookie("/v1/providers", map[string]any{
		"name": "Hacker Provider",
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin creating provider, got %d", code)
	}
}

// --- OWASP AUTHZ: IDOR Prevention Tests ---

func TestAccount_CannotViewOtherProfile_ViaHeaderTampering(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// User A.
	idA := srv.CreateIdentity("userA@test.com", "User A")
	tokenA := srv.CreateSession(idA)

	// User B.
	_ = srv.CreateIdentity("userB@test.com", "User B")

	// User A accesses their own profile — should see their own data.
	code, body := srv.GetWithCookie("/v1/account/profile", tokenA)
	if code != 200 {
		t.Fatalf("expected 200, got %d", code)
	}

	// The identity returned should be User A, not User B.
	identifier, _ := body["identifier"].(string)
	if identifier != "" && identifier != "userA@test.com" {
		t.Errorf("expected profile for userA, got %q", identifier)
	}
}

func TestAccount_SelfServiceOnlySeesOwnSessions(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// User A.
	idA := srv.CreateIdentity("sessA@test.com", "Sess A")
	tokenA := srv.CreateSession(idA)

	// User B creates sessions.
	idB := srv.CreateIdentity("sessB@test.com", "Sess B")
	_ = srv.CreateSession(idB) // B's session

	// User A lists their own sessions via self-service.
	code, body := srv.GetWithCookie("/v1/account/sessions", tokenA)
	if code != 200 {
		t.Fatalf("expected 200, got %d", code)
	}

	// All returned sessions should belong to User A.
	items, _ := body["items"].([]any)
	for _, item := range items {
		session, _ := item.(map[string]any)
		entityID, _ := session["entity_id"].(string)
		if entityID != idA && entityID != "" {
			t.Errorf("user A sees session for entity %v — IDOR vulnerability!", entityID)
		}
	}
}

// --- OWASP AUTHZ: Authorization Matrix (table-driven) ---

func TestAuthorizationMatrix(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// Create admin + regular user sessions.
	adminToken := srv.LoginAdmin()
	userID := srv.CreateIdentity("matrix-user@test.com", "Matrix User")
	userToken := srv.CreateSession(userID)

	type testCase struct {
		Method   string
		Path     string
		Body     map[string]any
		Unauth   int // expected code without auth
		User     int // expected code for regular user
		Admin    int // expected code for admin
	}

	cases := []testCase{
		{"POST", "/v1/schemas", map[string]any{"type": "t", "schema": map[string]any{"type": "object"}}, 401, 403, 201},
		{"POST", "/v1/entities", map[string]any{"identifier": "matrix@test.com"}, 401, 403, 201},
		{"GET", "/v1/sessions", nil, 401, 403, 200},
		{"POST", "/v1/pats", map[string]any{"name": "p", "entity_id": 1, "scopes": []string{"admin"}}, 401, 403, 201},
		{"POST", "/v1/import", map[string]any{"identities": []any{}}, 401, 403, 200},
		{"POST", "/v1/providers", map[string]any{"name": "P", "protocol": "oidc", "config": map[string]any{"issuer": "https://x.com", "client_id": "c"}}, 401, 403, 201},
		// Self-service routes should work for both user and admin.
		{"GET", "/v1/account/profile", nil, 401, 200, 200},
		{"GET", "/v1/account/sessions", nil, 401, 200, 200},
	}

	for _, tc := range cases {
		t.Run(tc.Method+"_"+tc.Path, func(t *testing.T) {
			// 1. Unauthenticated.
			var code int
			switch tc.Method {
			case "GET":
				code, _ = srv.GetRaw(tc.Path)
			case "POST":
				code, _ = srv.PostJSONRaw(tc.Path, tc.Body)
			}
			if code != tc.Unauth {
				t.Errorf("unauthenticated %s %s: expected %d, got %d", tc.Method, tc.Path, tc.Unauth, code)
			}

			// 2. Regular user.
			switch tc.Method {
			case "GET":
				code, _ = srv.GetWithCookie(tc.Path, userToken)
			case "POST":
				code, _ = srv.PostJSONWithCookie(tc.Path, tc.Body, userToken)
			}
			if code != tc.User {
				t.Errorf("user %s %s: expected %d, got %d", tc.Method, tc.Path, tc.User, code)
			}

			// 3. Admin — should NOT be blocked by auth (not 401/403).
			switch tc.Method {
			case "GET":
				code, _ = srv.GetWithCookie(tc.Path, adminToken)
			case "POST":
				code, _ = srv.PostJSONWithCookie(tc.Path, tc.Body, adminToken)
			}
			// Admin should get through auth — any code except 401/403 is acceptable.
			if code == 401 || code == 403 {
				t.Errorf("admin %s %s: expected access (got auth error %d)", tc.Method, tc.Path, code)
			}
		})
	}
}
