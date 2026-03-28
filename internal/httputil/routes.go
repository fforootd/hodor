package httputil

import (
	"strings"
)

// PublicRoutes returns the single source of truth for routes that bypass
// both AuthN (api/middleware.go) and AuthZ (fga/middleware.go).
// Keeping both lists in sync is critical — a mismatch causes silent 401/403.
func PublicRoutes() map[string][]string {
	return map[string][]string{
		"GET": {
			"/healthz",
			"/readyz",
			"/login",
			"/assets/",
			"/console",
			"/console/",
			"/account",
			"/account/",
			"/.well-known/",
			"/openapi.json",
			"/v1/branding",
			"/v1/auth/settings",
			"/v1/providers/templates",
			"/v1/schemas",
			"/v1/schemas/",
			"/",
			"/authorize",
			"/oauth/",
			"/userinfo",
			"/keys",
			"/end_session",
			"/revoke",
			"/devicecode",
		},
		"POST": {
			"/v1/login/",
			"/authorize",
			"/oauth/",
		},
	}
}

// IsPublicRoute checks if method+path matches a public route pattern.
func IsPublicRoute(method, path string) bool {
	// Non-API routes (console, assets, etc.) bypass all gates.
	if !strings.HasPrefix(path, "/v1/") &&
		!strings.HasPrefix(path, "/authorize") &&
		!strings.HasPrefix(path, "/oauth/") &&
		!strings.HasPrefix(path, "/userinfo") &&
		!strings.HasPrefix(path, "/keys") &&
		!strings.HasPrefix(path, "/healthz") &&
		!strings.HasPrefix(path, "/readyz") {
		return true
	}

	routes := PublicRoutes()
	for _, pattern := range routes[method] {
		if MatchesPattern(path, pattern) {
			return true
		}
	}
	return false
}

// MatchesPattern checks if a path matches a route pattern.
// Exact match or prefix match for patterns ending with "/".
func MatchesPattern(path, pattern string) bool {
	if path == pattern {
		return true
	}
	if len(pattern) > 1 && strings.HasSuffix(pattern, "/") && strings.HasPrefix(path, pattern) {
		return true
	}
	return false
}
