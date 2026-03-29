package fga

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/zitadel/zitadel/internal/httputil"
)

func TestResolveRouteType(t *testing.T) {
	tests := []struct {
		path     string
		wantType string
		wantID   string
	}{
		{"/v1/users", "entity", ""},
		{"/v1/users/abc123", "entity", "abc123"},
		{"/v1/users", "entity", ""},
		{"/v1/users/xyz", "entity", "xyz"},
		{"/v1/apps", "app", ""},
		{"/v1/apps/myapp", "app", "myapp"},
		{"/v1/groups", "group", ""},
		{"/v1/groups/g1", "group", "g1"},
		{"/v1/schemas", "schema", ""},
		{"/v1/schemas/human_user_v1", "schema", "human_user_v1"},
		{"/v1/sessions", "session", ""},
		{"/v1/settings", "settings", ""},
		{"/v1/settings/password_policy", "settings", "password_policy"},
		{"/v1/fga", "org", ""},
		{"/v1/fga/check", "org", "check"},
		{"/healthz", "", ""},
		{"/console", "", ""},
		{"/v1/nonexistent", "", ""},
	}

	for _, tc := range tests {
		t.Run(tc.path, func(t *testing.T) {
			gotType, gotID := resolveRouteType(tc.path)
			if gotType != tc.wantType {
				t.Errorf("resolveRouteType(%q) type = %q, want %q", tc.path, gotType, tc.wantType)
			}
			if gotID != tc.wantID {
				t.Errorf("resolveRouteType(%q) id = %q, want %q", tc.path, gotID, tc.wantID)
			}
		})
	}
}

func TestBuildCheckObject(t *testing.T) {
	tests := []struct {
		fgaType    string
		resourceID string
		method     string
		orgID      string
		want       string
	}{
		{"schema", "", "POST", "", "instance:default"},
		{"schema", "human_user_v1", "GET", "", "instance:default"},
		{"provider", "", "POST", "", "instance:default"},
		{"provider", "prov1", "PATCH", "", "instance:default"},
		{"entity", "", "POST", "myorg", "org:myorg"},
		{"entity", "", "GET", "myorg", "org:myorg"},
		{"entity", "abc123", "GET", "", "org:_global"},    // entity → org-level (no per-entity FGA tuples yet)
		{"entity", "abc123", "PATCH", "", "org:_global"},  // entity → org-level
		{"entity", "abc123", "DELETE", "", "org:_global"}, // entity → org-level
		{"entity", "abc123", "GET", "myorg", "org:myorg"}, // entity → org from header
		{"session", "", "GET", "myorg", "org:myorg"},      // session → org-level
		{"session", "s1", "DELETE", "myorg", "org:myorg"}, // session → org-level
		{"org", "", "POST", "", "instance:default"},       // creating org
		{"settings", "pwd", "PATCH", "", "settings:pwd"},
		{"settings", "", "GET", "myorg", "org:myorg"},
	}

	for _, tc := range tests {
		t.Run(tc.fgaType+"_"+tc.method+"_"+tc.resourceID, func(t *testing.T) {
			r := httptest.NewRequest(tc.method, "/v1/test", nil)
			if tc.orgID != "" {
				r.Header.Set("X-Org-Id", tc.orgID)
			}
			got := buildCheckObject(tc.fgaType, tc.resourceID, r)
			if got != tc.want {
				t.Errorf("buildCheckObject(%q, %q, %s) = %q, want %q", tc.fgaType, tc.resourceID, tc.method, got, tc.want)
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

	// Bootstrap: admin owns instance, owns org.
	if err := svc.OnBootstrap(ctx, "admin", "org1"); err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	mw := NewMiddleware(svc)
	handler := mw.Gate(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	// Admin POSTing to /v1/users (checks can_create_entity against org).
	req := httptest.NewRequest("POST", "/v1/users", nil)
	req.Header.Set("X-Identity-Id", "admin")
	req.Header.Set("X-Org-Id", "org1")
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
