package api_test

import (
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestPostgres_SchemaBackedCRUDSmoke(t *testing.T) {
	postgresURL := os.Getenv("ZITADEL_TEST_POSTGRES_URL")
	if postgresURL == "" {
		t.Skip("set ZITADEL_TEST_POSTGRES_URL to run Postgres CRUD smoke tests")
	}

	srv := testutil.NewTestServerWithDatabaseURL(t, postgresURL)
	adminToken := srv.LoginAdmin()
	suffix := fmt.Sprintf("%d", time.Now().UnixNano())

	userCode, user := srv.PostJSONWithBearer("/v1/users", map[string]any{
		"identifier":   "pg-user-" + suffix + "@example.com",
		"display_name": "PG User " + suffix,
		"schema_id":    "human_user_v1",
	}, adminToken)
	if userCode != 201 {
		t.Fatalf("create user status = %d body=%#v", userCode, user)
	}
	if user["schema_id"] != "human_user_v1" {
		t.Fatalf("user schema_id = %v, want human_user_v1", user["schema_id"])
	}

	appCode, app := srv.PostJSONWithBearer("/v1/apps", map[string]any{
		"name":           "PG App " + suffix,
		"client_id":      "pg-app-" + suffix,
		"redirect_uris":  []string{"https://example.com/callback"},
		"grant_types":    []string{"authorization_code"},
		"response_types": []string{"code"},
		"schema_id":      "app_v1",
	}, adminToken)
	if appCode != 201 {
		t.Fatalf("create app status = %d body=%#v", appCode, app)
	}
	if app["schema_id"] != "app_v1" {
		t.Fatalf("app schema_id = %v, want app_v1", app["schema_id"])
	}

	providerCode, provider := srv.PostJSONWithBearer("/v1/providers", map[string]any{
		"display_name": "PG Provider " + suffix,
		"protocol":     "oidc",
		"connection": map[string]any{
			"issuer":    "https://issuer.example.com",
			"client_id": "pg-provider-" + suffix,
		},
		"target": map[string]any{
			"schema_type": "human_user",
		},
	}, adminToken)
	if providerCode != 201 {
		t.Fatalf("create provider status = %d body=%#v", providerCode, provider)
	}
	if provider["schema_id"] != "provider_v1" {
		t.Fatalf("provider schema_id = %v, want provider_v1", provider["schema_id"])
	}

	flowCode, flow := srv.PostJSONWithBearer("/v1/login-flows", map[string]any{
		"name":      "PG Flow " + suffix,
		"strategy":  "identifier_first",
		"state":     "draft",
		"schema_id": "login_flow_v1",
		"auth_methods": map[string]any{
			"password": map[string]any{"enabled": true},
		},
	}, adminToken)
	if flowCode != 201 {
		t.Fatalf("create login flow status = %d body=%#v", flowCode, flow)
	}
	if flow["schema_id"] != "login_flow_v1" {
		t.Fatalf("login flow schema_id = %v, want login_flow_v1", flow["schema_id"])
	}
}
