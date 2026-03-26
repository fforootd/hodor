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
	
	code, body := srv.PostJSONWithCookie("/v1/schemas", map[string]any{
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
	identityID := srv.CreateIdentity("user4@test.com", "User 4")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.PostJSONWithCookie("/v1/schemas", map[string]any{
		"id": "hack_schema_v1",
	}, userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin schema create, got %d", code)
	}
}
