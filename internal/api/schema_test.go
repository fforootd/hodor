package api_test

import (
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestSchema_CRUD(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()

	// 1. Create Schema
	schemaDef := map[string]any{
		"type": "object",
		"properties": map[string]any{
			"test_field": map[string]any{"type": "string"},
		},
	}

	code, body := srv.PostJSONWithBearer("/v1/schemas", map[string]any{
		"id":     "custom_test_schema_v1",
		"type":   "custom_schema",
		"schema": schemaDef,
	}, adminToken)

	if code != 201 {
		t.Fatalf("expected 201 creating schema, got %d: %v", code, body)
	}

	// 2. Read Schema (Public)
	code, body = srv.GetRaw("/v1/schemas/custom_test_schema_v1")
	if code != 200 {
		t.Fatalf("expected 200 reading schema, got %d", code)
	}
	if body["id"] != "custom_test_schema_v1" {
		t.Errorf("expected custom_test_schema_v1, got %v", body["id"])
	}

	// 3. List Schemas (Public)
	code, body = srv.GetRaw("/v1/schemas")
	if code != 200 {
		t.Fatalf("expected 200 listing schemas, got %d", code)
	}
	items, _ := body["items"].([]any)
	if len(items) == 0 {
		t.Fatalf("expected >0 schemas, got 0")
	}
}

func TestSchema_NonAdminForbiddenWrite(t *testing.T) {
	srv := testutil.NewTestServer(t)
	userID := srv.CreateIdentity("user4@test.com", "User 4")
	userToken := srv.CreateSession(userID)

	code, _ := srv.PostJSONWithBearer("/v1/schemas", map[string]any{
		"id": "hack_schema_v1",
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin schema create, got %d", code)
	}
}

func TestSchema_PromoteAffectsOnlyFutureWrites(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()

	createCode, createdUser := srv.PostJSONWithBearer("/v1/users", map[string]any{
		"identifier":   "before-promotion@example.com",
		"display_name": "Before Promotion",
		"schema_id":    "human_user_v1",
	}, adminToken)
	if createCode != 201 {
		t.Fatalf("create pinned user status = %d body=%#v", createCode, createdUser)
	}
	if createdUser["schema_id"] != "human_user_v1" {
		t.Fatalf("created pinned user schema_id = %v, want human_user_v1", createdUser["schema_id"])
	}
	pinnedUserID := createdUser["id"].(string)

	getCode, currentSchema := srv.GetRaw("/v1/schemas/human_user_v1")
	if getCode != 200 {
		t.Fatalf("get base schema status = %d body=%#v", getCode, currentSchema)
	}
	schemaDoc, _ := currentSchema["schema"].(map[string]any)
	properties, _ := schemaDoc["properties"].(map[string]any)
	if properties == nil {
		t.Fatal("human_user_v1 schema.properties missing")
	}
	properties["favorite_color"] = map[string]any{"type": "string"}

	patchCode, newSchema := srv.PatchJSONWithBearer("/v1/schemas/human_user_v1", map[string]any{
		"schema":  schemaDoc,
		"message": "add favorite color",
	}, adminToken)
	if patchCode != 201 {
		t.Fatalf("create new schema version status = %d body=%#v", patchCode, newSchema)
	}
	if newSchema["id"] != "human_user_v2" {
		t.Fatalf("new schema id = %v, want human_user_v2", newSchema["id"])
	}

	promoteCode, promoted := srv.RequestWithHeaders("POST", "/v1/schemas/human_user_v2/promote", map[string]string{
		"Authorization": "Bearer " + adminToken,
	}, nil)
	if promoteCode != 200 {
		t.Fatalf("promote schema status = %d body=%#v", promoteCode, promoted)
	}
	if promoted["schema_id"] != "human_user_v2" {
		t.Fatalf("promoted schema_id = %v, want human_user_v2", promoted["schema_id"])
	}
	futureWrites, _ := promoted["future_default_writes"].(map[string]any)
	if futureWrites["scope"] != "new_writes_only" {
		t.Fatalf("future_default_writes.scope = %v, want new_writes_only", futureWrites["scope"])
	}
	if futureWrites["applies_to_existing_rows"] != false {
		t.Fatalf("future_default_writes.applies_to_existing_rows = %v, want false", futureWrites["applies_to_existing_rows"])
	}

	userCode, pinnedUser := srv.GetWithBearer("/v1/users/"+pinnedUserID, adminToken)
	if userCode != 200 {
		t.Fatalf("get pinned user status = %d body=%#v", userCode, pinnedUser)
	}
	if pinnedUser["schema_id"] != "human_user_v1" {
		t.Fatalf("pinned user schema_id = %v, want human_user_v1", pinnedUser["schema_id"])
	}

	newUserCode, futureUser := srv.PostJSONWithBearer("/v1/users", map[string]any{
		"identifier":   "after-promotion@example.com",
		"display_name": "After Promotion",
	}, adminToken)
	if newUserCode != 201 {
		t.Fatalf("create future user status = %d body=%#v", newUserCode, futureUser)
	}
	if futureUser["schema_id"] != "human_user_v2" {
		t.Fatalf("future user schema_id = %v, want human_user_v2", futureUser["schema_id"])
	}
}
