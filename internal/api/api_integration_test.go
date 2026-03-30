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
	code, body := srv.PostJSONWithBearer("/v1/schemas", map[string]any{"type": "test"}, "")
	if code != 401 {
		t.Fatalf("expected 401, got %d: %v", code, body)
	}

	// Account endpoints without auth.
	code, _ = srv.GetWithBearer("/v1/account/profile", "")
	if code != 401 {
		t.Fatalf("expected 401 for profile, got %d", code)
	}
}

func TestInvalidToken_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)

	code, _ := srv.GetWithBearer("/v1/account/profile", "invalid-token-123")
	if code != 401 {
		t.Fatalf("expected 401, got %d", code)
	}
}

func TestValidAdminToken_Returns200(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	code, body := srv.GetWithBearer("/v1/account/profile", token)
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

	code, _ := srv.GetWithBearer("/v1/account/profile", token)
	if code != 401 {
		t.Fatalf("expected 401 for expired session, got %d", code)
	}
}

func TestRevokedSession_Returns401(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	// Revoke the session.
	_, _ = srv.DB.SQL().Exec(`UPDATE sessions SET revoked_at = datetime('now')`)

	code, _ := srv.GetWithBearer("/v1/account/profile", token)
	if code != 401 {
		t.Fatalf("expected 401 for revoked session, got %d", code)
	}
}

// ===================== AuthZ Tests =====================

func TestNonAdmin_CannotAccessAdminEndpoints(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("user@test.com", "Test User")
	userToken := srv.CreateSession(userID)

	// Non-admin should not be able to create schemas.
	code, _ := srv.PostJSONWithBearer("/v1/schemas", map[string]any{
		"type":   "test_schema",
		"schema": "{}",
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 for non-admin schema create, got %d", code)
	}
}

func TestNonAdmin_CanAccessOwnProfile(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("user@test.com", "Test User")
	userToken := srv.CreateSession(userID)

	code, body := srv.GetWithBearer("/v1/account/profile", userToken)
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

	code, body := srv.PostJSONWithBearer("/v1/users", map[string]any{
		"identifier":   "new@test.com",
		"display_name": "New User",
		"schema_id":    "human_user_v1",
		"state":        "active",
	}, token)

	if code != 200 && code != 201 {
		t.Fatalf("expected 200/201, got %d: %v", code, body)
	}
}

func TestAdmin_UserDetailIncludesSchemaContextAndCanonicalData(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	createCode, created := srv.RequestWithHeaders("POST", "/v1/users", map[string]string{
		"Authorization": "Bearer " + token,
		"X-Org-Id":      srv.OrgID,
	}, map[string]any{
		"schema_id": "human_user_v1",
		"data": map[string]any{
			"email":        "schema-user@test.com",
			"display_name": "Schema User",
			"given_name":   "Schema",
			"family_name":  "User",
		},
	})
	if createCode != 200 && createCode != 201 {
		t.Fatalf("create user: expected 200/201, got %d: %v", createCode, created)
	}

	userID, _ := created["id"].(string)
	getCode, body := srv.GetWithBearer("/v1/users/"+userID, token)
	if getCode != 200 {
		t.Fatalf("get user: expected 200, got %d: %v", getCode, body)
	}

	if body["schema_id"] != "human_user_v1" {
		t.Fatalf("schema_id = %v, want human_user_v1", body["schema_id"])
	}
	if body["schema_type"] != "human_user" {
		t.Fatalf("schema_type = %v, want human_user", body["schema_type"])
	}

	data, _ := body["data"].(map[string]any)
	if data["email"] != "schema-user@test.com" {
		t.Fatalf("email = %v, want schema-user@test.com", data["email"])
	}
	if data["display_name"] != "Schema User" {
		t.Fatalf("display_name = %v, want Schema User", data["display_name"])
	}
}

func TestAdmin_AppCanonicalDataRoundTrip(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	createCode, created := srv.RequestWithHeaders("POST", "/v1/apps", map[string]string{
		"Authorization": "Bearer " + token,
		"X-Org-Id":      srv.OrgID,
	}, map[string]any{
		"schema_id": "app_v1",
		"data": map[string]any{
			"client_name":               "Schema App",
			"description":               "Back-office app",
			"app_type":                  "web",
			"redirect_uris":             []string{"https://example.com/callback"},
			"post_logout_redirect_uris": []string{"https://example.com/logout"},
			"grant_types":               []string{"authorization_code"},
			"response_types":            []string{"code"},
			"logo_uri":                  "https://example.com/logo.png",
			"metadata":                  map[string]any{"tier": "pro"},
		},
	})
	if createCode != 200 && createCode != 201 {
		t.Fatalf("create app: expected 200/201, got %d: %v", createCode, created)
	}

	appID, _ := created["id"].(string)
	getCode, body := srv.GetWithBearer("/v1/apps/"+appID, token)
	if getCode != 200 {
		t.Fatalf("get app: expected 200, got %d: %v", getCode, body)
	}

	if body["schema_type"] != "app" {
		t.Fatalf("schema_type = %v, want app", body["schema_type"])
	}
	if body["description"] != "Back-office app" {
		t.Fatalf("description = %v, want Back-office app", body["description"])
	}
	if body["logo_uri"] != "https://example.com/logo.png" {
		t.Fatalf("logo_uri = %v, want logo uri", body["logo_uri"])
	}

	postLogout, _ := body["post_logout_redirect_uris"].([]any)
	if len(postLogout) != 1 || postLogout[0] != "https://example.com/logout" {
		t.Fatalf("post_logout_redirect_uris = %v", body["post_logout_redirect_uris"])
	}

	data, _ := body["data"].(map[string]any)
	if data["client_name"] != "Schema App" {
		t.Fatalf("client_name = %v, want Schema App", data["client_name"])
	}
}

func TestAdmin_OrgGroupProjectCanonicalDataRoundTrip(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	orgCode, orgCreated := srv.PostJSONWithBearer("/v1/orgs", map[string]any{
		"schema_id": "org_v1",
		"data": map[string]any{
			"display_name": "Schema Org",
			"metadata":     map[string]any{"region": "us"},
		},
	}, token)
	if orgCode != 200 && orgCode != 201 {
		t.Fatalf("create org: expected 200/201, got %d: %v", orgCode, orgCreated)
	}
	orgID, _ := orgCreated["id"].(string)

	groupCode, groupCreated := srv.RequestWithHeaders("POST", "/v1/groups", map[string]string{
		"Authorization": "Bearer " + token,
		"X-Org-Id":      srv.OrgID,
	}, map[string]any{
		"data": map[string]any{
			"name":        "Schema Group",
			"description": "Writers",
			"metadata":    map[string]any{"scope": "docs"},
		},
	})
	if groupCode != 200 && groupCode != 201 {
		t.Fatalf("create group: expected 200/201, got %d: %v", groupCode, groupCreated)
	}
	groupID, _ := groupCreated["id"].(string)

	projectCode, projectCreated := srv.RequestWithHeaders("POST", "/v1/projects", map[string]string{
		"Authorization": "Bearer " + token,
		"X-Org-Id":      srv.OrgID,
	}, map[string]any{
		"data": map[string]any{
			"name":        "Schema Project",
			"description": "Prototype",
			"metadata":    map[string]any{"phase": "beta"},
		},
	})
	if projectCode != 200 && projectCode != 201 {
		t.Fatalf("create project: expected 200/201, got %d: %v", projectCode, projectCreated)
	}
	projectID, _ := projectCreated["id"].(string)

	if code, body := srv.GetWithBearer("/v1/orgs/"+orgID, token); code != 200 {
		t.Fatalf("get org: expected 200, got %d: %v", code, body)
	} else if data, _ := body["data"].(map[string]any); data["display_name"] != "Schema Org" {
		t.Fatalf("org display_name = %v, want Schema Org", data["display_name"])
	}

	if code, body := srv.GetWithBearer("/v1/groups/"+groupID, token); code != 200 {
		t.Fatalf("get group: expected 200, got %d: %v", code, body)
	} else if data, _ := body["data"].(map[string]any); data["description"] != "Writers" {
		t.Fatalf("group description = %v, want Writers", data["description"])
	}

	if code, body := srv.GetWithBearer("/v1/projects/"+projectID, token); code != 200 {
		t.Fatalf("get project: expected 200, got %d: %v", code, body)
	} else if data, _ := body["data"].(map[string]any); data["description"] != "Prototype" {
		t.Fatalf("project description = %v, want Prototype", data["description"])
	}
}

// ===================== Bulk Import Tests =====================

func TestBulkImport_CreatesIdentities(t *testing.T) {
	srv := testutil.NewTestServer(t)
	token := srv.LoginAdmin()

	code, body := srv.PostJSONWithBearer("/v1/import", map[string]any{
		"users": []map[string]any{
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
	srv.PostJSONWithBearer("/v1/import", map[string]any{
		"users": []map[string]any{
			{"identifier": "dup@test.com", "display_name": "Dup User"},
		},
	}, token)

	// Import again with same identifier.
	code, body := srv.PostJSONWithBearer("/v1/import", map[string]any{
		"users": []map[string]any{
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

	code, body := srv.PostJSONWithBearer("/v1/import", map[string]any{
		"providers": []map[string]any{
			{"name": "Test OIDC", "protocol": "oidc", "config": map[string]any{"issuer": "https://test.example.com"}},
		},
		"users": []map[string]any{
			{"identifier": "linked@test.com", "display_name": "Linked User"},
		},
		"linked_identities": []map[string]any{
			{"user_identifier": "linked@test.com", "provider_name": "Test OIDC", "external_sub": "ext-123"},
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

	code, body := srv.PostJSONWithBearer("/v1/admin/bulk", map[string]any{
		"users": []map[string]any{
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
	code, _ := srv.PostJSONWithBearer("/v1/import", map[string]any{
		"users": []map[string]any{
			{"identifier": "unauth@test.com"},
		},
	}, "")

	if code != 401 {
		t.Fatalf("expected 401, got %d", code)
	}
}

func TestBulkImport_NonAdminForbidden(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("regularuser@test.com", "Regular")
	userToken := srv.CreateSession(userID)

	code, _ := srv.PostJSONWithBearer("/v1/import", map[string]any{
		"users": []map[string]any{
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
	token := srv.LoginAdmin()

	// List should have at least admin.
	code, body := srv.GetWithBearer("/v1/users", token)
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

	code, body = srv.GetWithBearer("/v1/users/"+adminID, token)
	if code != 200 {
		t.Fatalf("get: expected 200, got %d", code)
	}
	if body["identifier"] != "admin" {
		t.Errorf("expected admin, got %v", body["identifier"])
	}
}
