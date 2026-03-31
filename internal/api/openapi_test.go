package api_test

import (
	"encoding/json"
	"testing"

	"github.com/zitadel/zitadel/internal/api"
)

func TestOpenAPISpecGeneration(t *testing.T) {
	spec := newOpenAPISpec()

	assertTopLevelSpec(t, spec)
	paths := requireMap(t, spec, "paths")
	assertMinimumPaths(t, paths)
	assertCriticalPaths(t, paths)
	components := requireMap(t, spec, "components")
	assertComponents(t, components)
	assertProtectedOperationSecurity(t, paths)
	assertTags(t, spec)
	assertSchemaTypeFilter(t, paths, "/v1/users")
	assertSchemaTypeFilter(t, paths, "/v1/apps")
}

func TestOpenAPISpecJSON(t *testing.T) {
	a := api.New(nil, nil, nil)
	a.RegisterOpenAPIOnly()

	data, err := a.Spec().SpecJSON()
	if err != nil {
		t.Fatalf("SpecJSON failed: %v", err)
	}

	// Verify it's valid JSON.
	var parsed map[string]any
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("generated spec is not valid JSON: %v", err)
	}

	// Verify minimum size (should be substantial).
	if len(data) < 10000 {
		t.Errorf("spec seems too small: %d bytes", len(data))
	}
}

func TestOpenAPIOperationIDs(t *testing.T) {
	paths := requireMap(t, newOpenAPISpec(), "paths")

	seen := map[string]bool{}
	for path, methods := range paths {
		methodMap, ok := methods.(map[string]any)
		if !ok {
			t.Errorf("path %s: expected method map, got %T", path, methods)
			continue
		}
		for method, opAny := range methodMap {
			op, ok := opAny.(map[string]any)
			if !ok {
				t.Errorf("%s %s: expected operation map, got %T", method, path, opAny)
				continue
			}
			opID, ok := op["operationId"].(string)
			if !ok || opID == "" {
				t.Errorf("%s %s: missing operationId", method, path)
				continue
			}
			if seen[opID] {
				t.Errorf("duplicate operationId: %s", opID)
			}
			seen[opID] = true
		}
	}
}

func newOpenAPISpec() map[string]any {
	a := api.New(nil, nil, nil)
	a.RegisterOpenAPIOnly()
	return a.Spec().Spec()
}

func assertTopLevelSpec(t *testing.T, spec map[string]any) {
	t.Helper()

	if spec["openapi"] != "3.1.0" {
		t.Errorf("expected openapi 3.1.0, got %v", spec["openapi"])
	}

	info := requireMap(t, spec, "info")
	if info["title"] != "Zitadel API" {
		t.Errorf("expected title 'Zitadel API', got %v", info["title"])
	}
}

func assertMinimumPaths(t *testing.T, paths map[string]any) {
	t.Helper()

	minPaths := 35
	if len(paths) < minPaths {
		t.Errorf("expected at least %d paths, got %d", minPaths, len(paths))
	}
}

func assertCriticalPaths(t *testing.T, paths map[string]any) {
	t.Helper()

	criticalPaths := []string{
		"/v1/users",
		"/v1/users/{id}",
		"/v1/apps",
		"/v1/apps/{id}",
		"/v1/schemas",
		"/v1/schemas/{id}",
		"/v1/sessions",
		"/v1/events",
		"/v1/pats",
		"/v1/fga/check",
		"/v1/fga/tuples",
		"/v1/providers",
		"/v1/settings/{type}",
		"/v1/catalog",
		"/v1/search",
		"/v1/counts",
		"/v1/account/profile",
		"/v1/branding",
		"/v1/auth/settings",
		"/v1/login/flows",
		"/v1/orgs",
	}
	for _, p := range criticalPaths {
		if _, exists := paths[p]; !exists {
			t.Errorf("critical path %s missing from spec", p)
		}
	}
}

func assertComponents(t *testing.T, components map[string]any) {
	t.Helper()

	schemas := requireMap(t, components, "schemas")
	minSchemas := 20
	if len(schemas) < minSchemas {
		t.Errorf("expected at least %d schemas, got %d", minSchemas, len(schemas))
	}

	secSchemes := requireMap(t, components, "securitySchemes")
	if _, exists := secSchemes["bearerAuth"]; !exists {
		t.Error("bearerAuth security scheme missing")
	}
	if _, exists := secSchemes["cookieAuth"]; !exists {
		t.Error("cookieAuth security scheme missing")
	}
}

func assertProtectedOperationSecurity(t *testing.T, paths map[string]any) {
	t.Helper()

	accountPath := requireMap(t, paths, "/v1/account/profile")
	getProfile := requireMap(t, accountPath, "get")
	security, ok := getProfile["security"].([]map[string]any)
	if !ok || len(security) != 2 {
		t.Fatalf("expected two security alternatives for GET /v1/account/profile, got %T %#v", getProfile["security"], getProfile["security"])
	}
	if _, ok := security[0]["cookieAuth"]; !ok {
		t.Error("expected cookieAuth to be the default security scheme")
	}
	if _, ok := security[1]["bearerAuth"]; !ok {
		t.Error("expected bearerAuth to remain available")
	}
}

func assertTags(t *testing.T, spec map[string]any) {
	t.Helper()

	tags, ok := spec["tags"].([]map[string]any)
	if !ok {
		t.Fatal("missing tags")
	}
	if len(tags) < 10 {
		t.Errorf("expected at least 10 tags, got %d", len(tags))
	}
}

func assertSchemaTypeFilter(t *testing.T, paths map[string]any, path string) {
	t.Helper()

	pathItem := requireMap(t, paths, path)
	getOp := requireMap(t, pathItem, "get")
	params, ok := getOp["parameters"].([]map[string]any)
	if !ok {
		t.Fatalf("missing GET %s parameters", path)
	}
	var foundSchemaType bool
	for _, param := range params {
		if param["name"] == "schema_type" {
			foundSchemaType = true
			break
		}
	}
	if !foundSchemaType {
		t.Errorf("expected GET %s to document schema_type filter", path)
	}
}

func requireMap(t *testing.T, parent map[string]any, key string) map[string]any {
	t.Helper()

	value, ok := parent[key]
	if !ok {
		t.Fatalf("missing %s", key)
	}
	child, ok := value.(map[string]any)
	if !ok {
		t.Fatalf("%s is %T, want map[string]any", key, value)
	}
	return child
}
