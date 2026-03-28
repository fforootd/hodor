package api

import (
	"database/sql"
	"fmt"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/session"
)

// publicRoutes returns the set of path prefixes/patterns that skip authentication.
// Patterns ending with "*" are treated as prefix matches.
func publicRoutes() map[string][]string {
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

// isPublicRoute checks if the request matches any public route pattern.
func isPublicRoute(method, path string) bool {
	routes := publicRoutes()

	// Check method-specific routes.
	for _, pattern := range routes[method] {
		if matchesPattern(path, pattern) {
			return true
		}
	}

	return false
}

func matchesPattern(path, pattern string) bool {
	// Exact match.
	if path == pattern {
		return true
	}
	// Prefix match for patterns ending with "/" (but not the root "/" itself).
	if len(pattern) > 1 && strings.HasSuffix(pattern, "/") && strings.HasPrefix(path, pattern) {
		return true
	}
	return false
}

// AuthGate is the top-level default-deny middleware.
// Every request not on the public allowlist must carry a valid token.
// On success, it injects X-Identity-Id, X-Session-Id, and X-Token-Type headers.
func AuthGate(cookieCfg *session.CookieConfig, db *sql.DB) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Allow public routes unconditionally.
			if isPublicRoute(r.Method, r.URL.Path) {
				next.ServeHTTP(w, r)
				return
			}

			// Extract token from header or cookie.
			rawToken := extractTokenFromRequest(r, cookieCfg)
			if rawToken == "" {
				writeError(w, http.StatusUnauthorized, "authentication required")
				return
			}

			// Resolve token against the database.
			info, err := resolveToken(r.Context(), db, rawToken)
			if err != nil {
				writeError(w, http.StatusUnauthorized, "invalid or expired token")
				return
			}

			// Inject identity info into request headers (internal use only).
			r.Header.Set("X-Identity-Id", info.EntityID)
			r.Header.Set("X-Session-Id", info.SessionID)
			r.Header.Set("X-Token-Type", info.TokenType)

			next.ServeHTTP(w, r)
		})
	}
}

// extractTokenFromRequest gets the token from either the Authorization header or cookie.
// Bearer tokens are used as-is. Cookie tokens are HMAC-verified first.
// This is the standalone version used by AuthGate (not bound to API struct).
func extractTokenFromRequest(r *http.Request, cookies *session.CookieConfig) string {
	// Try Authorization: Bearer <token> first (API clients).
	if auth := r.Header.Get("Authorization"); auth != "" {
		if strings.HasPrefix(auth, "Bearer ") {
			return strings.TrimPrefix(auth, "Bearer ")
		}
	}
	// Fall back to HMAC-signed session cookie (browser).
	if cookies != nil {
		if token, ok := session.ReadSessionCookie(r, cookies); ok {
			return token
		}
	}
	return ""
}

// requireAdmin is middleware that checks for a valid session with the "admin" capability.
// It supports both:
//   - Cookie-based auth (browser UI): HMAC-signed session cookie
//   - Bearer token auth (API clients): Authorization: Bearer <token>
//
// When AuthGate is active, identity info is already injected via X-Identity-Id.
// This middleware additionally checks for admin capability.
func (a *API) requireAdmin(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		identityID, err := a.resolveCallerIdentity(r)
		if err != nil {
			writeError(w, http.StatusUnauthorized, "authentication required")
			return
		}

		// Check for admin capability.
		var adminCap int
		err = a.db.SQL().QueryRowContext(r.Context(),
			`SELECT 1 FROM entity_capabilities WHERE entity_id = ? AND capability = 'admin'`,
			identityID,
		).Scan(&adminCap)
		if err == sql.ErrNoRows {
			writeError(w, http.StatusForbidden, "admin capability required")
			return
		}
		if err != nil {
			writeError(w, http.StatusInternalServerError, "authorization check failed")
			return
		}

		next(w, r)
	}
}

// requireSession is middleware that ensures a valid authenticated user.
// It injects the caller's identity ID into the request header.
// When AuthGate is active, identity info is already injected via X-Identity-Id.
func (a *API) requireSession(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		_, err := a.resolveCallerIdentity(r)
		if err != nil {
			writeError(w, http.StatusUnauthorized, "authentication required")
			return
		}
		next(w, r)
	}
}

// resolveCallerIdentity extracts the caller's identity from AuthGate-injected headers,
// falling back to direct token resolution if headers aren't set (e.g., in tests).
func (a *API) resolveCallerIdentity(r *http.Request) (string, error) {
	// Fast path: AuthGate already resolved the identity.
	if idStr := r.Header.Get("X-Identity-Id"); idStr != "" {
		return idStr, nil
	}

	// Slow path: resolve token directly (backward compatibility / tests).
	rawToken := a.extractToken(r)
	if rawToken == "" {
		return "", fmt.Errorf("no token")
	}

	info, err := resolveToken(r.Context(), a.db.SQL(), rawToken)
	if err != nil {
		return "", err
	}

	// Inject headers for downstream handlers.
	r.Header.Set("X-Identity-Id", info.EntityID)
	r.Header.Set("X-Session-Id", info.SessionID)
	r.Header.Set("X-Token-Type", info.TokenType)

	return info.EntityID, nil
}

// extractToken gets the session token from either the Authorization header or cookie.
// This is the API-bound version used by requireAdmin/requireSession.
func (a *API) extractToken(r *http.Request) string {
	return extractTokenFromRequest(r, a.cookies)
}
