package httputil

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

// --- WriteJSON ---

func TestWriteJSON_StatusAndContentType(t *testing.T) {
	rec := httptest.NewRecorder()
	WriteJSON(rec, http.StatusCreated, map[string]string{"id": "123"})

	if rec.Code != http.StatusCreated {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusCreated)
	}
	if ct := rec.Header().Get("Content-Type"); ct != "application/json" {
		t.Errorf("Content-Type = %q, want application/json", ct)
	}
}

func TestWriteJSON_Body(t *testing.T) {
	rec := httptest.NewRecorder()
	WriteJSON(rec, http.StatusOK, map[string]any{"count": 42, "ok": true})

	var result map[string]any
	if err := json.NewDecoder(rec.Body).Decode(&result); err != nil {
		t.Fatalf("decode body: %v", err)
	}
	if result["count"] != float64(42) {
		t.Errorf("count = %v, want 42", result["count"])
	}
	if result["ok"] != true {
		t.Errorf("ok = %v, want true", result["ok"])
	}
}

func TestWriteJSON_NilBody(t *testing.T) {
	rec := httptest.NewRecorder()
	WriteJSON(rec, http.StatusNoContent, nil)
	if rec.Code != http.StatusNoContent {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusNoContent)
	}
}

func TestWriteJSON_Slice(t *testing.T) {
	rec := httptest.NewRecorder()
	WriteJSON(rec, http.StatusOK, []string{"a", "b"})

	var result []string
	if err := json.NewDecoder(rec.Body).Decode(&result); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if len(result) != 2 || result[0] != "a" {
		t.Errorf("result = %v, want [a b]", result)
	}
}

// --- WriteError ---

func TestWriteError_Format(t *testing.T) {
	rec := httptest.NewRecorder()
	WriteError(rec, http.StatusForbidden, "access denied")

	if rec.Code != http.StatusForbidden {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusForbidden)
	}

	var result map[string]any
	if err := json.NewDecoder(rec.Body).Decode(&result); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if result["error"] != "access denied" {
		t.Errorf("error = %v, want 'access denied'", result["error"])
	}
	if result["code"] != float64(403) {
		t.Errorf("code = %v, want 403", result["code"])
	}
}

func TestWriteError_EscapesSpecialChars(t *testing.T) {
	rec := httptest.NewRecorder()
	WriteError(rec, http.StatusBadRequest, `value "foo" is <invalid>`)

	var result map[string]any
	if err := json.NewDecoder(rec.Body).Decode(&result); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if result["error"] != `value "foo" is <invalid>` {
		t.Errorf("error = %v", result["error"])
	}
}

// --- IsPublicRoute ---

func TestIsPublicRoute(t *testing.T) {
	tests := []struct {
		method string
		path   string
		want   bool
	}{
		// Non-API routes are always public.
		{"GET", "/console", true},
		{"GET", "/console/overview", true},
		{"GET", "/login", true},
		{"GET", "/assets/style.css", true},
		{"GET", "/account", true},
		{"GET", "/account/profile", true},

		// API public GETs.
		{"GET", "/v1/schemas", true},
		{"GET", "/v1/schemas/human_user_v1", true},
		{"GET", "/v1/branding", true},
		{"GET", "/v1/auth/settings", true},
		{"GET", "/v1/providers/templates", true},
		{"GET", "/healthz", true},
		{"GET", "/readyz", true},

		// API public POSTs.
		{"POST", "/v1/login/password", true},
		{"POST", "/v1/login/flows", true},

		// Protected routes.
		{"GET", "/v1/entities", false},
		{"POST", "/v1/entities", false},
		{"GET", "/v1/sessions", false},
		{"DELETE", "/v1/sessions/abc", false},
		{"GET", "/v1/fga/model", false},
		{"POST", "/v1/fga/check", false},
		{"GET", "/v1/apps", false},
		{"POST", "/v1/orgs", false},
		{"PATCH", "/v1/schemas/abc", false},
		{"DELETE", "/v1/schemas/abc", false},
	}

	for _, tc := range tests {
		t.Run(tc.method+"_"+tc.path, func(t *testing.T) {
			got := IsPublicRoute(tc.method, tc.path)
			if got != tc.want {
				t.Errorf("IsPublicRoute(%q, %q) = %v, want %v", tc.method, tc.path, got, tc.want)
			}
		})
	}
}

// --- MatchesPattern ---

func TestMatchesPattern(t *testing.T) {
	tests := []struct {
		path, pattern string
		want          bool
	}{
		{"/v1/schemas", "/v1/schemas", true},                   // exact
		{"/v1/schemas/abc", "/v1/schemas/", true},              // prefix with /
		{"/v1/schemas/abc", "/v1/schemas", false},              // no trailing / = exact only
		{"/v1/login/password", "/v1/login/", true},             // prefix
		{"/v1/login", "/v1/login/", false},                     // path shorter than prefix
		{"/console", "/console", true},                         // exact
		{"/console/foo", "/console/", true},                    // prefix
		{"/", "/", true},                                       // root exact
		{"/anything", "/", false},                              // root doesn't prefix-match (len 1)
		{"/v1/entities", "/v1/schemas", false},                 // no match
	}

	for _, tc := range tests {
		t.Run(tc.path+"_"+tc.pattern, func(t *testing.T) {
			got := MatchesPattern(tc.path, tc.pattern)
			if got != tc.want {
				t.Errorf("MatchesPattern(%q, %q) = %v, want %v", tc.path, tc.pattern, got, tc.want)
			}
		})
	}
}

// --- ResolveOrgID ---

func TestResolveOrgID_Header(t *testing.T) {
	r := httptest.NewRequest("GET", "/v1/entities", nil)
	r.Header.Set("X-Org-Id", "org-from-header")

	got := ResolveOrgID(r, "_global")
	if got != "org-from-header" {
		t.Errorf("got %q, want org-from-header", got)
	}
}

func TestResolveOrgID_QueryParam(t *testing.T) {
	r := httptest.NewRequest("GET", "/v1/entities?org=org-from-query", nil)

	got := ResolveOrgID(r, "_global")
	if got != "org-from-query" {
		t.Errorf("got %q, want org-from-query", got)
	}
}

func TestResolveOrgID_HeaderTakesPriority(t *testing.T) {
	r := httptest.NewRequest("GET", "/v1/entities?org=query-org", nil)
	r.Header.Set("X-Org-Id", "header-org")

	got := ResolveOrgID(r, "_global")
	if got != "header-org" {
		t.Errorf("got %q, want header-org (header takes priority)", got)
	}
}

func TestResolveOrgID_Fallback(t *testing.T) {
	r := httptest.NewRequest("GET", "/v1/entities", nil)

	got := ResolveOrgID(r, "_global")
	if got != "_global" {
		t.Errorf("got %q, want _global (fallback)", got)
	}
}

func TestResolveOrgID_EmptyFallback(t *testing.T) {
	r := httptest.NewRequest("GET", "/v1/entities", nil)

	got := ResolveOrgID(r, "")
	if got != "" {
		t.Errorf("got %q, want empty string", got)
	}
}

// --- PublicRoutes consistency ---

func TestPublicRoutes_HasExpectedKeys(t *testing.T) {
	routes := PublicRoutes()
	if _, ok := routes["GET"]; !ok {
		t.Error("missing GET routes")
	}
	if _, ok := routes["POST"]; !ok {
		t.Error("missing POST routes")
	}
}

func TestPublicRoutes_SchemasArePublic(t *testing.T) {
	routes := PublicRoutes()
	found := false
	for _, r := range routes["GET"] {
		if r == "/v1/schemas" {
			found = true
			break
		}
	}
	if !found {
		t.Error("/v1/schemas should be in public GET routes")
	}
}

func TestPublicRoutes_FGANotPublic(t *testing.T) {
	routes := PublicRoutes()
	for method, paths := range routes {
		for _, p := range paths {
			if p == "/v1/fga/model" || p == "/v1/fga/check" || p == "/v1/fga/tuples" {
				t.Errorf("FGA route %s %s should NOT be public", method, p)
			}
		}
	}
}
