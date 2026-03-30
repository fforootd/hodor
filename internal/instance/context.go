// Package instance provides request-scoped instance resolution for
// multi-tenant Zitadel deployments (ADR-021).
//
// Every HTTP request is resolved to an instance_id, which is then used
// to scope all database queries. Resolution priority:
//
//  1. Nested path: /v1/instances/{iid}/...
//  2. X-Zitadel-Instance header
//  3. Domain lookup: Host → instances.domain
//  4. Default: "inst_root"
package instance

import (
	"context"
	"database/sql"
	"net/http"
	"strings"
)

type contextKey struct{}

// DefaultInstance is the fallback when no instance context is found.
const DefaultInstance = "inst_root"

// FromContext returns the instance_id from the request context.
// Falls back to DefaultInstance if not set.
func FromContext(ctx context.Context) string {
	if id, ok := ctx.Value(contextKey{}).(string); ok && id != "" {
		return id
	}
	return DefaultInstance
}

// WithContext returns a new context with the instance_id set.
func WithContext(ctx context.Context, id string) context.Context {
	return context.WithValue(ctx, contextKey{}, id)
}

// Middleware resolves the instance context for each request.
// It runs early in the chain (before auth) so all downstream
// handlers can use FromContext(r.Context()).
func Middleware(db *sql.DB) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			instanceID := resolve(r, db)
			ctx := WithContext(r.Context(), instanceID)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

// resolve determines the instance_id from the request.
func resolve(r *http.Request, db *sql.DB) string {
	// 1. Nested path: /v1/instances/{iid}/...
	// (handled by the proxy handler, not here — the proxy sets context directly)

	// 2. Explicit header
	if h := r.Header.Get("X-Zitadel-Instance"); h != "" {
		return h
	}

	// 3. Domain-based lookup
	host := r.Host
	// Strip port if present.
	if idx := strings.LastIndex(host, ":"); idx > 0 {
		host = host[:idx]
	}
	// Don't do domain lookup for localhost / 127.0.0.1
	if host != "" && host != "localhost" && host != "127.0.0.1" {
		var instanceID string
		err := db.QueryRowContext(r.Context(),
			`SELECT id FROM instances WHERE domain = ? AND state = 'active' LIMIT 1`, host,
		).Scan(&instanceID)
		if err == nil && instanceID != "" {
			return instanceID
		}
	}

	// 4. Default
	return DefaultInstance
}
