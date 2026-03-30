package fga

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/zitadel/zitadel/internal/httputil"
)

func TestResolveRouteType(t *testing.T) {
	m := &Middleware{
		routes: map[string]string{
			"/v1/users":     "entity",
			"/v1/apps":      "app",
			"/v1/orgs":      "org",
			"/v1/schemas":   "schema",
			"/v1/sessions":  "session",
			"/v1/settings":  "settings",
			"/v1/providers": "provider",
			"/v1/fga":       "fga",
		},
	}

	tests := []struct {
		path     string
		wantType string
		wantID   string
	}{
		{"/v1/users", "entity", ""},
		{"/v1/users/abc123", "entity", "abc123"},
		{"/v1/apps", "app", ""},
		{"/v1/apps/myapp", "app", "myapp"},
		{"/v1/schemas", "schema", ""},
		{"/v1/schemas/human_user_v1", "schema", "human_user_v1"},
		{"/v1/sessions", "session", ""},
		{"/v1/settings", "settings", ""},
		{"/v1/settings/password_policy", "settings", "password_policy"},
		{"/v1/orgs", "org", ""},
		{"/v1/orgs/org1", "org", "org1"},
		{"/v1/fga", "fga", ""},
		{"/v1/fga/check", "fga", "check"},
		{"/healthz", "", ""},
		{"/console", "", ""},
		{"/v1/nonexistent", "", ""},
	}

	for _, tc := range tests {
		t.Run(tc.path, func(t *testing.T) {
			gotType, gotID := m.resolveRouteType(tc.path)
			if gotType != tc.wantType {
				t.Errorf("resolveRouteType(%q) type = %q, want %q", tc.path, gotType, tc.wantType)
			}
			if gotID != tc.wantID {
				t.Errorf("resolveRouteType(%q) id = %q, want %q", tc.path, gotID, tc.wantID)
			}
		})
	}
}

func TestResolveObject(t *testing.T) {
	tests := []struct {
		name       string
		cfg        AuthZConfig
		resourceID string
		method     string
		orgID      string
		want       string
	}{
		// Instance-scoped: always instance:self
		{"schema_list", AuthZConfig{Scope: "instance", FGAType: "schema"}, "", "GET", "", "instance:self"},
		{"schema_resource", AuthZConfig{Scope: "instance", FGAType: "schema"}, "abc", "GET", "", "instance:self"},
		{"provider_create", AuthZConfig{Scope: "instance", FGAType: "provider"}, "", "POST", "", "instance:self"},
		{"session_list", AuthZConfig{Scope: "instance", FGAType: "session"}, "", "GET", "", "instance:self"},
		{"entity_list", AuthZConfig{Scope: "instance", FGAType: "entity"}, "", "GET", "", "instance:self"},
		{"entity_resource", AuthZConfig{Scope: "instance", FGAType: "entity"}, "abc", "GET", "", "instance:self"},

		// Org-scoped: check against org
		{"app_list", AuthZConfig{Scope: "org", FGAType: "app"}, "", "GET", "org1", "org:org1"},
		{"app_create", AuthZConfig{Scope: "org", FGAType: "app"}, "", "POST", "org1", "org:org1"},
		{"app_resource", AuthZConfig{Scope: "org", FGAType: "app"}, "myapp", "GET", "org1", "org:org1"},
		{"app_no_org", AuthZConfig{Scope: "org", FGAType: "app"}, "", "GET", "", "org:_global"},

		// Org with resource scope override: resource-level for specific org
		{"org_list", AuthZConfig{Scope: "instance", FGAType: "org"}, "", "GET", "", "instance:self"},
		{"org_create", AuthZConfig{Scope: "instance", FGAType: "org"}, "", "POST", "", "instance:self"},
		{"org_resource", AuthZConfig{Scope: "instance", FGAType: "org", ResourceScope: "resource"}, "org1", "GET", "", "org:org1"},
		{"org_resource_patch", AuthZConfig{Scope: "instance", FGAType: "org", ResourceScope: "resource"}, "org1", "PATCH", "", "org:org1"},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			r := httptest.NewRequest(tc.method, "/v1/test", nil)
			if tc.orgID != "" {
				r.Header.Set("X-Org-Id", tc.orgID)
			}
			got := tc.cfg.resolveObject(tc.resourceID, r)
			if got != tc.want {
				t.Errorf("resolveObject(%q, %q) = %q, want %q", tc.cfg.FGAType, tc.resourceID, got, tc.want)
			}
		})
	}
}

// TestIsPublicRoute verifies public route detection via the shared httputil package.
func TestIsPublicRoute(t *testing.T) {
	tests := []struct {
		method string
		path   string
		want   bool
	}{
		{"GET", "/healthz", true},
		{"GET", "/console", true},
		{"GET", "/v1/schemas", true},
		{"GET", "/v1/schemas/human_user_v1", true},
		{"GET", "/v1/branding", true},
		{"POST", "/v1/login/password", true},
		{"GET", "/v1/users", false},
		{"POST", "/v1/users", false},
		{"GET", "/v1/fga/model", false},
		{"POST", "/v1/fga/check", false},
	}

	for _, tc := range tests {
		t.Run(tc.method+"_"+tc.path, func(t *testing.T) {
			got := httputil.IsPublicRoute(tc.method, tc.path)
			if got != tc.want {
				t.Errorf("IsPublicRoute(%q, %q) = %v, want %v", tc.method, tc.path, got, tc.want)
			}
		})
	}
}

func TestGate_UnauthenticatedPassthrough(t *testing.T) {
	// FGA middleware should pass through unauthenticated requests
	// (AuthGate handles 401 before FGA runs).
	svc := newTestService(t)
	mw := NewMiddleware(svc)

	handler := mw.Gate(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("GET", "/v1/users", nil)
	// No X-Identity-Id header → unauthenticated
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected passthrough for unauthenticated, got %d", rec.Code)
	}
}

func TestGate_DeniesUnauthorized(t *testing.T) {
	svc := newTestService(t)
	mw := NewMiddleware(svc)

	handler := mw.Gate(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("POST", "/v1/users", nil)
	req.Header.Set("X-Identity-Id", "user-with-no-grants")
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusForbidden {
		t.Errorf("expected 403 for unauthorized user, got %d", rec.Code)
	}
}

func TestGate_AllowsAuthorized(t *testing.T) {
	svc := newTestService(t)
	ctx := httptest.NewRequest("GET", "/", nil).Context()

	// Bootstrap: admin owns instance.
	if err := svc.OnBootstrap(ctx, "admin"); err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	mw := NewMiddleware(svc)
	handler := mw.Gate(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	// Admin GETting /v1/users (checks can_manage_entities against instance:default).
	req := httptest.NewRequest("GET", "/v1/users", nil)
	req.Header.Set("X-Identity-Id", "admin")
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected 200 for authorized admin, got %d", rec.Code)
	}
}

func TestGate_PublicRouteBypass(t *testing.T) {
	svc := newTestService(t)
	mw := NewMiddleware(svc)

	handler := mw.Gate(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	// Public schema GET route should bypass FGA.
	req := httptest.NewRequest("GET", "/v1/schemas", nil)
	req.Header.Set("X-Identity-Id", "anybody")
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected 200 for public route, got %d", rec.Code)
	}
}

func TestBuildAuthZFromCatalog(t *testing.T) {
	configs, routes := BuildAuthZFromCatalog()

	// Verify route mapping for key paths.
	if routes["/v1/users"] != "entity" {
		t.Errorf("/v1/users → %q, want entity", routes["/v1/users"])
	}
	if routes["/v1/orgs"] != "org" {
		t.Errorf("/v1/orgs → %q, want org", routes["/v1/orgs"])
	}
	if routes["/v1/schemas"] != "schema" {
		t.Errorf("/v1/schemas → %q, want schema", routes["/v1/schemas"])
	}
	if routes["/v1/fga"] != "fga" {
		t.Errorf("/v1/fga → %q, want fga", routes["/v1/fga"])
	}

	// Verify configs exist for key types.
	for _, fgaType := range []string{"entity", "org", "schema", "provider", "app", "settings", "fga"} {
		if _, ok := configs[fgaType]; !ok {
			t.Errorf("missing AuthZConfig for type %q", fgaType)
		}
	}

	// Verify org override: collection = instance-level, resource = resource-level.
	orgCfg := configs["org"]
	if orgCfg.Scope != "instance" {
		t.Errorf("org scope = %q, want instance", orgCfg.Scope)
	}
	if orgCfg.ResourceScope != "resource" {
		t.Errorf("org ResourceScope = %q, want resource", orgCfg.ResourceScope)
	}
	if orgCfg.CollectionPerms["GET"] != "can_manage_orgs" {
		t.Errorf("org collection GET = %q, want can_manage_orgs", orgCfg.CollectionPerms["GET"])
	}
	if orgCfg.ResourcePerms["GET"] != "owner" {
		t.Errorf("org resource GET = %q, want owner", orgCfg.ResourcePerms["GET"])
	}

	// Verify entity override: instance-scoped.
	entityCfg := configs["entity"]
	if entityCfg.Scope != "instance" {
		t.Errorf("entity scope = %q, want instance", entityCfg.Scope)
	}
	if entityCfg.CollectionPerms["GET"] != "can_manage_entities" {
		t.Errorf("entity collection GET = %q, want can_manage_entities", entityCfg.CollectionPerms["GET"])
	}

	// Verify provider has correct permission (not can_manage_schemas).
	provCfg := configs["provider"]
	if provCfg.CollectionPerms["POST"] != "can_manage_providers" {
		t.Errorf("provider collection POST = %q, want can_manage_providers", provCfg.CollectionPerms["POST"])
	}
}
