package api_test

import (
	"encoding/json"
	"testing"

	"github.com/zitadel/zitadel/internal/api"
)

func TestOpenAPISpecGeneration(t *testing.T) {
	a := api.New(nil, nil, nil)
	a.RegisterOpenAPIOnly()

	spec := a.Spec().Spec()

	// Verify top-level structure.
	if spec["openapi"] != "3.1.0" {
		t.Errorf("expected openapi 3.1.0, got %v", spec["openapi"])
	}

	info, ok := spec["info"].(map[string]any)
	if !ok {
		t.Fatal("missing info object")
	}
	if info["title"] != "Zitadel API" {
		t.Errorf("expected title 'Zitadel API', got %v", info["title"])
	}

	paths, ok := spec["paths"].(map[string]any)
	if !ok {
		t.Fatal("missing paths object")
	}

	// Verify minimum endpoint coverage.
	minPaths := 35
	if len(paths) < minPaths {
		t.Errorf("expected at least %d paths, got %d", minPaths, len(paths))
	}

	// Verify critical endpoints are present.
	criticalPaths := []string{
		"/v1/users",
		"/v1/users/{id}",
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

	// Verify components/schemas exist.
	components, ok := spec["components"].(map[string]any)
	if !ok {
		t.Fatal("missing components")
	}
	schemas, ok := components["schemas"].(map[string]any)
	if !ok {
		t.Fatal("missing components/schemas")
	}
	minSchemas := 20
	if len(schemas) < minSchemas {
		t.Errorf("expected at least %d schemas, got %d", minSchemas, len(schemas))
	}

	// Verify security scheme.
	secSchemes, ok := components["securitySchemes"].(map[string]any)
	if !ok {
		t.Fatal("missing securitySchemes")
	}
	if _, exists := secSchemes["bearerAuth"]; !exists {
		t.Error("bearerAuth security scheme missing")
	}

	// Verify tags are present.
	tags, ok := spec["tags"].([]map[string]any)
	if !ok {
		t.Fatal("missing tags")
	}
	if len(tags) < 10 {
		t.Errorf("expected at least 10 tags, got %d", len(tags))
	}
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
	a := api.New(nil, nil, nil)
	a.RegisterOpenAPIOnly()

	spec := a.Spec().Spec()
	paths := spec["paths"].(map[string]any)

	seen := map[string]bool{}
	for path, methods := range paths {
		methodMap := methods.(map[string]any)
		for method, opAny := range methodMap {
			op := opAny.(map[string]any)
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
