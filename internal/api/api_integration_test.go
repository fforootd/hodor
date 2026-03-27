package api_test

import (
	"fmt"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

// ===================== AuthN Tests =====================

func TestHealthz(t *testing.T) {
	srv := testutil.NewTestServer(t)
	code, _ := srv.GetRaw("/healthz")
	if code != 200 {
		t.Fatalf("expected 200, got %d", code)
	}
}

func TestUnauthenticated_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// Admin-only endpoints without auth.
	code, body := srv.PostJSONWithCookie("/v1/schemas", map[string]any{"type": "test"}, "")
	if code != 401 {
		t.Fatalf("expected 401, got %d: %v", code, body)
	}

	// Account endpoints without auth.
	code, _ = srv.GetWithCookie("/v1/account/profile", "")
	if code != 401 {
		t.Fatalf("expected 401 for profile, got %d", code)
	}
}

func TestInvalidToken_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)

	code, _ := srv.GetWithCookie("/v1/account/profile", "invalid-token-123")
	if code != 401 {
		t.Fatalf("expected 401, got %d", code)
	}
}

func TestValidAdminToken_Returns200(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	code, body := srv.GetWithCookie("/v1/account/profile", token)
	if code != 200 {
		t.Fatalf("expected 200, got %d: %v", code, body)
	}

	// Verify we got the admin identity.
	identity, _ := body["identity"].(map[string]any)
	if identity["identifier"] != "admin" {
		t.Errorf("expected admin, got %v", identity["identifier"])
	}
}

func TestExpiredSession_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	// Manually expire the session.
	_, _ = srv.DB.SQL().Exec(`UPDATE sessions SET expires_at = datetime('now', '-1 hour')`)

	code, _ := srv.GetWithCookie("/v1/account/profile", token)
	if code != 401 {
		t.Fatalf("expected 401 for expired session, got %d", code)
	}
}

func TestRevokedSession_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	// Revoke the session.
	_, _ = srv.DB.SQL().Exec(`UPDATE sessions SET revoked_at = datetime('now')`)

	code, _ := srv.GetWithCookie("/v1/account/profile", token)
	if code != 401 {
		t.Fatalf("expected 401 for revoked session, got %d", code)
	}
}

// ===================== AuthZ Tests =====================

func TestNonAdmin_CannotAccessAdminEndpoints(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user@test.com", "Test User")
	userToken := srv.CreateSession(identityID)

	// Non-admin should not be able to create schemas.
	code, _ := srv.PostJSONWithCookie("/v1/schemas", map[string]any{
		"type":   "test_schema",
		"schema": "{}",
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 for non-admin schema create, got %d", code)
	}
}

func TestNonAdmin_CanAccessOwnProfile(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user@test.com", "Test User")
	userToken := srv.CreateSession(identityID)

	code, body := srv.GetWithCookie("/v1/account/profile", userToken)
	if code != 200 {
		t.Fatalf("expected 200, got %d", code)
	}

	identity, _ := body["identity"].(map[string]any)
	if identity["identifier"] != "user@test.com" {
		t.Errorf("expected user@test.com, got %v", identity["identifier"])
	}
}

func TestAdmin_CanCreateIdentity(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	code, body := srv.PostJSONWithCookie("/v1/entities", map[string]any{
		"identifier":   "new@test.com",
		"display_name": "New User",
		"schema_id":    "human_user_v1",
		"state":        "active",
	}, token)

	if code != 200 && code != 201 {
		t.Fatalf("expected 200/201, got %d: %v", code, body)
	}
}

// ===================== Bulk Import Tests =====================

func TestBulkImport_CreatesIdentities(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	code, body := srv.PostJSONWithCookie("/v1/import", map[string]any{
		"entities": []map[string]any{
			{"identifier": "bulk1@test.com", "display_name": "Bulk One", "password": "pass123"},
			{"identifier": "bulk2@test.com", "display_name": "Bulk Two", "password": "pass456"},
		},
		"on_conflict": "skip",
	}, token)

	if code != 200 {
		t.Fatalf("expected 200, got %d: %v", code, body)
	}

	created, _ := body["created"].(float64)
	if created != 2 {
		t.Errorf("expected 2 created, got %v", body["created"])
	}
}

func TestBulkImport_SkipsDuplicates(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	// Import once.
	srv.PostJSONWithCookie("/v1/import", map[string]any{
		"entities": []map[string]any{
			{"identifier": "dup@test.com", "display_name": "Dup User"},
		},
	}, token)

	// Import again with same identifier.
	code, body := srv.PostJSONWithCookie("/v1/import", map[string]any{
		"entities": []map[string]any{
			{"identifier": "dup@test.com", "display_name": "Dup User Updated"},
		},
		"on_conflict": "skip",
	}, token)

	if code != 200 {
		t.Fatalf("expected 200, got %d", code)
	}

	skipped, _ := body["skipped"].(float64)
	if skipped != 1 {
		t.Errorf("expected 1 skipped, got %v", body["skipped"])
	}
}

func TestBulkImport_WithProviders(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	code, body := srv.PostJSONWithCookie("/v1/import", map[string]any{
		"providers": []map[string]any{
			{"name": "Test OIDC", "protocol": "oidc", "config": map[string]any{"issuer": "https://test.example.com"}},
		},
		"entities": []map[string]any{
			{"identifier": "linked@test.com", "display_name": "Linked User"},
		},
		"linked_accounts": []map[string]any{
			{"entity_identifier": "linked@test.com", "provider_name": "Test OIDC", "external_sub": "ext-123"},
		},
		"on_conflict": "skip",
	}, token)

	if code != 200 {
		t.Fatalf("expected 200, got %d: %v", code, body)
	}

	created, _ := body["created"].(float64)
	if created != 3 {
		t.Errorf("expected 3 created (1 provider + 1 identity + 1 link), got %v", body["created"])
	}
}

func TestIdentitiesBulk_Creates(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	code, body := srv.PostJSONWithCookie("/v1/entities/bulk", map[string]any{
		"entities": []map[string]any{
			{"identifier": "batch1@test.com", "display_name": "Batch 1"},
			{"identifier": "batch2@test.com", "display_name": "Batch 2"},
			{"identifier": "batch3@test.com", "display_name": "Batch 3"},
		},
	}, token)

	if code != 200 {
		t.Fatalf("expected 200, got %d: %v", code, body)
	}

	created, _ := body["created"].(float64)
	if created != 3 {
		t.Errorf("expected 3, got %v", body["created"])
	}

	total, _ := body["total"].(float64)
	if total != 3 {
		t.Errorf("expected total 3, got %v", body["total"])
	}
}

func TestBulkImport_Unauthorized(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// No token — should be 401.
	code, _ := srv.PostJSONWithCookie("/v1/import", map[string]any{
		"entities": []map[string]any{
			{"identifier": "unauth@test.com"},
		},
	}, "")

	if code != 401 {
		t.Fatalf("expected 401, got %d", code)
	}
}

func TestBulkImport_NonAdminForbidden(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("regularuser@test.com", "Regular")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.PostJSONWithCookie("/v1/import", map[string]any{
		"entities": []map[string]any{
			{"identifier": "hack@test.com"},
		},
	}, userToken)

	if code != 403 {
		t.Fatalf("expected 403 for non-admin import, got %d", code)
	}
}

// ===================== CRUD Tests =====================

func TestIdentity_CRUD(t *testing.T) {
	srv := testutil.NewTestServer(t)

	// List should have at least admin.
	code, body := srv.GetRaw("/v1/entities")
	if code != 200 {
		t.Fatalf("list: expected 200, got %d", code)
	}
	items, _ := body["items"].([]any)
	if len(items) == 0 {
		t.Fatal("expected at least 1 identity")
	}

	// Get admin by ID.
	firstItem, _ := items[0].(map[string]any)
	adminID := fmt.Sprintf("%v", firstItem["id"])

	code, body = srv.GetRaw("/v1/entities/" + adminID)
	if code != 200 {
		t.Fatalf("get: expected 200, got %d", code)
	}
	if body["identifier"] != "admin" {
		t.Errorf("expected admin, got %v", body["identifier"])
	}
}
