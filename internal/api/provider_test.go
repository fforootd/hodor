package api_test

import (
	"fmt"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestProvider_CRUD(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()

	// 1. Create a provider
	code, body := srv.PostJSONWithBearer("/v1/providers", map[string]any{
		"display_name": "Test Provider",
		"kind":         "custom",
		"protocol":     "oidc",
		"connection": map[string]any{
			"issuer":    "https://test.issuer.com",
			"client_id": "client-123",
		},
		"target": map[string]any{
			"schema_type": "human_user",
		},
	}, adminToken)

	if code != 201 {
		t.Fatalf("expected 201 creating provider, got %d: %v", code, body)
	}

	providerID := fmt.Sprintf("%v", body["id"])

	// 2. List providers
	code, body = srv.GetWithBearer("/v1/providers", adminToken)
	if code != 200 {
		t.Fatalf("expected 200 listing providers, got %d", code)
	}
	items, _ := body["providers"].([]any)
	if len(items) == 0 {
		t.Fatalf("expected providers in list, got 0")
	}

	// 3. Update provider
	code, body = srv.PatchJSONWithBearer("/v1/providers/"+providerID, map[string]any{
		"display_name": "Updated Provider",
	}, adminToken)
	if code != 200 {
		t.Fatalf("expected 200 updating provider, got %d", code)
	}
	if body["display_name"] != "Updated Provider" {
		t.Errorf("expected updated provider name, got %v", body["display_name"])
	}

	// 4. Delete provider
	code, _ = srv.DeleteWithBearer("/v1/providers/"+providerID, adminToken)
	if code != 200 {
		t.Fatalf("expected 200 deleting provider, got %d", code)
	}

	// 5. Verify deleted
	code, _ = srv.GetWithBearer("/v1/providers/"+providerID, adminToken)
	if code != 404 {
		t.Fatalf("expected 404 after delete, got %d", code)
	}
}

func TestProvider_NonAdminForbidden(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("user3@test.com", "User 3")
	userToken := srv.CreateSession(userID)

	code, _ := srv.PostJSONWithBearer("/v1/providers", map[string]any{
		"display_name": "Hacker Provider",
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin creating provider, got %d", code)
	}
}
