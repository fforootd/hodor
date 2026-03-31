package api_test

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestLoginFlow_ManagementRoutes(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()

	createCode, created := srv.PostJSONWithBearer("/v1/login-flows", map[string]any{
		"name":     "Beta Flow",
		"strategy": "identifier_first",
		"state":    "draft",
		"priority": 42,
		"config": map[string]any{
			"branding": map[string]any{
				"heading": "Beta",
			},
		},
		"auth_methods": map[string]any{
			"password": map[string]any{"enabled": true},
		},
	}, adminToken)
	if createCode != http.StatusCreated {
		t.Fatalf("create login flow status = %d body=%#v", createCode, created)
	}

	flowID := fmt.Sprintf("%v", created["id"])
	if flowID == "" {
		t.Fatal("created login flow id is empty")
	}
	flowOrgID := fmt.Sprintf("%v", created["org_id"])
	if flowOrgID == "" {
		t.Fatal("created login flow org_id is empty")
	}

	listCode, listBody := srv.GetWithBearer("/v1/login-flows?state=draft", adminToken)
	if listCode != http.StatusOK {
		t.Fatalf("list login flows status = %d", listCode)
	}
	items, _ := listBody["items"].([]any)
	if len(items) == 0 {
		t.Fatal("expected draft login flows")
	}

	updateCode, updated := srv.PatchJSONWithBearer("/v1/login-flows/"+flowID, map[string]any{
		"display_name": "Beta Flow Updated",
		"state":        "testing",
		"priority":     99,
	}, adminToken)
	if updateCode != http.StatusOK {
		t.Fatalf("update login flow status = %d body=%#v", updateCode, updated)
	}
	if updated["name"] != "Beta Flow Updated" {
		t.Fatalf("updated name = %v", updated["name"])
	}

	promoteCode, promoted := srv.RequestWithHeaders("POST", "/v1/login-flows/"+flowID+"/promote", map[string]string{
		"Authorization": "Bearer " + adminToken,
	}, nil)
	if promoteCode != http.StatusOK {
		t.Fatalf("promote login flow status = %d body=%#v", promoteCode, promoted)
	}
	if promoted["state"] != "active" {
		t.Fatalf("promoted state = %v, want active", promoted["state"])
	}

	client := &http.Client{}
	req, _ := http.NewRequest("GET", srv.URL()+"/v1/login-flows/"+flowID+"/export", nil)
	req.Header.Set("Authorization", "Bearer "+adminToken)
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("GET export: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("export status = %d", resp.StatusCode)
	}
	if !strings.Contains(resp.Header.Get("Content-Disposition"), flowID) {
		t.Fatalf("Content-Disposition = %q", resp.Header.Get("Content-Disposition"))
	}
	var exported map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&exported); err != nil {
		t.Fatalf("decode export: %v", err)
	}
	payload, _ := exported["payload"].(map[string]any)
	if payload["name"] != "Beta Flow Updated" {
		t.Fatalf("export payload name = %v", payload["name"])
	}

	resolveCode, resolved := srv.PostJSONWithBearer("/v1/login-flows/resolve", map[string]any{
		"org_id":    flowOrgID,
		"schema_id": "human_user_v1",
	}, adminToken)
	if resolveCode != http.StatusOK {
		t.Fatalf("resolve login flow status = %d body=%#v", resolveCode, resolved)
	}
	if fmt.Sprintf("%v", resolved["id"]) != flowID {
		t.Fatalf("resolved id = %v, want %s", resolved["id"], flowID)
	}

	archiveCode, archived := srv.RequestWithHeaders("POST", "/v1/login-flows/"+flowID+"/archive", map[string]string{
		"Authorization": "Bearer " + adminToken,
	}, nil)
	if archiveCode != http.StatusOK {
		t.Fatalf("archive login flow status = %d body=%#v", archiveCode, archived)
	}
	if archived["state"] != "archived" {
		t.Fatalf("archived state = %v", archived["state"])
	}

	deleteCode, _ := srv.DeleteWithBearer("/v1/login-flows/"+flowID, adminToken)
	if deleteCode != http.StatusNoContent {
		t.Fatalf("delete login flow status = %d", deleteCode)
	}
}
