package api_test

import (
	"fmt"
	"net/http"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestGroupProject_ModuleRoutes(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()

	groupCode, groupCreated := srv.PostJSONWithBearer("/v1/groups", map[string]any{
		"schema_id": "group_v1",
		"name":      "Platform Team",
		"metadata": map[string]any{
			"cost_center": "eng",
		},
	}, adminToken)
	if groupCode != http.StatusCreated {
		t.Fatalf("create group status = %d body=%#v", groupCode, groupCreated)
	}
	groupID := fmt.Sprintf("%v", groupCreated["id"])
	if groupCreated["schema_id"] != "group_v1" {
		t.Fatalf("created group schema_id = %v, want group_v1", groupCreated["schema_id"])
	}

	listGroupCode, listGroups := srv.GetWithBearer("/v1/groups", adminToken)
	if listGroupCode != http.StatusOK {
		t.Fatalf("list groups status = %d", listGroupCode)
	}
	groupItems, _ := listGroups["items"].([]any)
	if len(groupItems) == 0 {
		t.Fatal("expected groups in list")
	}
	foundGroup := false
	for _, item := range groupItems {
		entry, _ := item.(map[string]any)
		if fmt.Sprintf("%v", entry["id"]) == groupID {
			foundGroup = true
			if entry["schema_id"] != "group_v1" {
				t.Fatalf("listed group schema_id = %v, want group_v1", entry["schema_id"])
			}
		}
	}
	if !foundGroup {
		t.Fatalf("group %s missing from list response", groupID)
	}

	getGroupCode, fetchedGroup := srv.GetWithBearer("/v1/groups/"+groupID, adminToken)
	if getGroupCode != http.StatusOK {
		t.Fatalf("get group status = %d body=%#v", getGroupCode, fetchedGroup)
	}
	if fetchedGroup["schema_id"] != "group_v1" {
		t.Fatalf("fetched group schema_id = %v, want group_v1", fetchedGroup["schema_id"])
	}

	updateGroupCode, updatedGroup := srv.PatchJSONWithBearer("/v1/groups/"+groupID, map[string]any{
		"name":      "Platform Team Updated",
		"state":     "inactive",
		"schema_id": "group_v1",
	}, adminToken)
	if updateGroupCode != http.StatusOK {
		t.Fatalf("update group status = %d body=%#v", updateGroupCode, updatedGroup)
	}
	if updatedGroup["name"] != "Platform Team Updated" {
		t.Fatalf("updated group name = %v", updatedGroup["name"])
	}
	if updatedGroup["schema_id"] != "group_v1" {
		t.Fatalf("updated group schema_id = %v, want group_v1", updatedGroup["schema_id"])
	}

	projectCode, projectCreated := srv.PostJSONWithBearer("/v1/projects", map[string]any{
		"schema_id": "project_v1",
		"name":      "Identity Rewrite",
		"metadata": map[string]any{
			"track": "r-and-d",
		},
	}, adminToken)
	if projectCode != http.StatusCreated {
		t.Fatalf("create project status = %d body=%#v", projectCode, projectCreated)
	}
	projectID := fmt.Sprintf("%v", projectCreated["id"])
	if projectCreated["schema_id"] != "project_v1" {
		t.Fatalf("created project schema_id = %v, want project_v1", projectCreated["schema_id"])
	}

	listProjectCode, listProjects := srv.GetWithBearer("/v1/projects", adminToken)
	if listProjectCode != http.StatusOK {
		t.Fatalf("list projects status = %d", listProjectCode)
	}
	projectItems, _ := listProjects["items"].([]any)
	if len(projectItems) == 0 {
		t.Fatal("expected projects in list")
	}
	foundProject := false
	for _, item := range projectItems {
		entry, _ := item.(map[string]any)
		if fmt.Sprintf("%v", entry["id"]) == projectID {
			foundProject = true
			if entry["schema_id"] != "project_v1" {
				t.Fatalf("listed project schema_id = %v, want project_v1", entry["schema_id"])
			}
		}
	}
	if !foundProject {
		t.Fatalf("project %s missing from list response", projectID)
	}

	getProjectCode, fetchedProject := srv.GetWithBearer("/v1/projects/"+projectID, adminToken)
	if getProjectCode != http.StatusOK {
		t.Fatalf("get project status = %d body=%#v", getProjectCode, fetchedProject)
	}
	if fetchedProject["schema_id"] != "project_v1" {
		t.Fatalf("fetched project schema_id = %v, want project_v1", fetchedProject["schema_id"])
	}

	updateProjectCode, updatedProject := srv.PatchJSONWithBearer("/v1/projects/"+projectID, map[string]any{
		"name":      "Identity Rewrite Updated",
		"state":     "inactive",
		"schema_id": "project_v1",
	}, adminToken)
	if updateProjectCode != http.StatusOK {
		t.Fatalf("update project status = %d body=%#v", updateProjectCode, updatedProject)
	}
	if updatedProject["name"] != "Identity Rewrite Updated" {
		t.Fatalf("updated project name = %v", updatedProject["name"])
	}
	if updatedProject["schema_id"] != "project_v1" {
		t.Fatalf("updated project schema_id = %v, want project_v1", updatedProject["schema_id"])
	}

	var groupSchemaID, projectSchemaID string
	if err := srv.DB.SQL().QueryRow(`SELECT schema_id FROM groups WHERE id = ?`, groupID).Scan(&groupSchemaID); err != nil {
		t.Fatalf("load group row: %v", err)
	}
	if err := srv.DB.SQL().QueryRow(`SELECT schema_id FROM projects WHERE id = ?`, projectID).Scan(&projectSchemaID); err != nil {
		t.Fatalf("load project row: %v", err)
	}
	if groupSchemaID != "group_v1" {
		t.Fatalf("stored group schema_id = %q, want group_v1", groupSchemaID)
	}
	if projectSchemaID != "project_v1" {
		t.Fatalf("stored project schema_id = %q, want project_v1", projectSchemaID)
	}

	modulesCode, modules := srv.GetWithBearer("/v1/modules", adminToken)
	if modulesCode != http.StatusOK {
		t.Fatalf("list modules status = %d", modulesCode)
	}
	moduleItems, _ := modules["items"].([]any)
	if len(moduleItems) == 0 {
		t.Fatal("expected modules in list")
	}

	disableCode, disabled := srv.RequestWithHeaders("POST", "/v1/modules/rbac/disable", map[string]string{
		"Authorization": "Bearer " + adminToken,
	}, nil)
	if disableCode != http.StatusOK {
		t.Fatalf("disable module status = %d body=%#v", disableCode, disabled)
	}
	enableCode, enabled := srv.RequestWithHeaders("POST", "/v1/modules/rbac/enable", map[string]string{
		"Authorization": "Bearer " + adminToken,
	}, nil)
	if enableCode != http.StatusOK {
		t.Fatalf("enable module status = %d body=%#v", enableCode, enabled)
	}

	deleteGroupCode, _ := srv.DeleteWithBearer("/v1/groups/"+groupID, adminToken)
	if deleteGroupCode != http.StatusNoContent {
		t.Fatalf("delete group status = %d", deleteGroupCode)
	}
	deleteProjectCode, _ := srv.DeleteWithBearer("/v1/projects/"+projectID, adminToken)
	if deleteProjectCode != http.StatusNoContent {
		t.Fatalf("delete project status = %d", deleteProjectCode)
	}
}
