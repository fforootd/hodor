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

func TestProvider_StoresResourceAndTargetSchemasSeparately(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()

	code, body := srv.PostJSONWithBearer("/v1/providers", map[string]any{
		"display_name": "Service Provisioner",
		"protocol":     "oidc",
		"connection": map[string]any{
			"issuer":    "https://issuer.example.com",
			"client_id": "svc-client",
		},
		"target": map[string]any{
			"schema_type": "service_user",
		},
	}, adminToken)
	if code != 201 {
		t.Fatalf("expected 201 creating provider, got %d: %v", code, body)
	}

	providerID := fmt.Sprintf("%v", body["id"])
	if body["schema_id"] != "provider_v1" {
		t.Fatalf("resource schema_id = %v, want provider_v1", body["schema_id"])
	}
	target, _ := body["target"].(map[string]any)
	if target["schema_id"] != "service_user_v1" {
		t.Fatalf("target.schema_id = %v, want service_user_v1", target["schema_id"])
	}
	if target["schema_type"] != "service_user" {
		t.Fatalf("target.schema_type = %v, want service_user", target["schema_type"])
	}

	var resourceSchemaID, targetSchemaID, targetSchemaType string
	if err := srv.DB.SQL().QueryRow(
		`SELECT schema_id, target_schema_id, target_schema_type FROM providers WHERE id = ?`,
		providerID,
	).Scan(&resourceSchemaID, &targetSchemaID, &targetSchemaType); err != nil {
		t.Fatalf("load provider row: %v", err)
	}
	if resourceSchemaID != "provider_v1" {
		t.Fatalf("providers.schema_id = %q, want provider_v1", resourceSchemaID)
	}
	if targetSchemaID != "service_user_v1" {
		t.Fatalf("providers.target_schema_id = %q, want service_user_v1", targetSchemaID)
	}
	if targetSchemaType != "service_user" {
		t.Fatalf("providers.target_schema_type = %q, want service_user", targetSchemaType)
	}
}
