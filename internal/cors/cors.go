// Package cors provides CORS middleware for the Zitadel HTTP API.
//
// Required for cross-origin embedding of the <zitadel-login> web component.
// When a customer embeds the login WC on https://app.acme.com but the API
// lives at https://auth.acme.com, the browser enforces CORS preflight.
//
// ADR-020: Customizable Login Layouts (cross-origin embedding)
package cors

import (
	"net/http"
	"strings"
)

// Config holds CORS configuration.
type Config struct {
	// AllowedOrigins is the list of origins permitted to make cross-origin requests.
	// Use ["*"] to allow all origins (not recommended for production with credentials).
	AllowedOrigins []string

	// AllowCredentials controls the Access-Control-Allow-Credentials header.
	// Must be true for cross-origin cookies (session cookies with SameSite=None).
	AllowCredentials bool

	// MaxAge is the preflight cache duration in seconds.
	MaxAge string
}

// DefaultConfig returns a permissive config suitable for development.
func DefaultConfig() Config {
	return Config{
		AllowedOrigins:   []string{"*"},
		AllowCredentials: false,
		MaxAge:           "3600",
	}
}

// Middleware returns an http.Handler that adds CORS headers and handles preflight.
// It only applies to the login flow API paths (/v1/login/*, /v1/branding, /v1/auth/settings).
func Middleware(cfg Config, next http.Handler) http.Handler {
	originSet := make(map[string]bool, len(cfg.AllowedOrigins))
	allowAll := false
	for _, o := range cfg.AllowedOrigins {
		if o == "*" {
			allowAll = true
		}
		originSet[o] = true
	}

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Only apply CORS to login-related API routes.
		if !isLoginRoute(r.URL.Path) {
			next.ServeHTTP(w, r)
			return
		}

		origin := r.Header.Get("Origin")
		if origin == "" {
			// Not a cross-origin request.
			next.ServeHTTP(w, r)
			return
		}

		// Check if origin is allowed.
		if !allowAll && !originSet[origin] {
			next.ServeHTTP(w, r)
			return
		}

		// Set CORS headers.
		if allowAll && !cfg.AllowCredentials {
			w.Header().Set("Access-Control-Allow-Origin", "*")
		} else {
			w.Header().Set("Access-Control-Allow-Origin", origin)
			w.Header().Set("Vary", "Origin")
		}

		if cfg.AllowCredentials {
			w.Header().Set("Access-Control-Allow-Credentials", "true")
		}

		// Handle preflight.
		if r.Method == http.MethodOptions {
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Flow-ID")
			w.Header().Set("Access-Control-Max-Age", cfg.MaxAge)
			w.WriteHeader(http.StatusNoContent)
			return
		}

		next.ServeHTTP(w, r)
	})
}

// isLoginRoute returns true for paths that need CORS for WC embedding.
func isLoginRoute(path string) bool {
	return strings.HasPrefix(path, "/v1/login/") ||
		path == "/v1/login/flows" ||
		path == "/v1/branding" ||
		path == "/v1/auth/settings" ||
		strings.HasPrefix(path, "/v1/auth/magic-link") ||
		strings.HasPrefix(path, "/v1/auth/sso/") ||
		strings.HasPrefix(path, "/v1/captcha/") ||
		strings.HasPrefix(path, "/v1/otel/") ||
		strings.HasPrefix(path, "/assets/")
}
