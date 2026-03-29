package api

import (
	"database/sql"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
)

// AuthGate is the top-level default-deny middleware.
// Every request not on the public allowlist must carry a valid token.
// On success, it injects X-Identity-Id, X-Session-Id, and X-Token-Type headers.
func AuthGate(cookieCfg *session.CookieConfig, db *sql.DB) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Allow public routes unconditionally.
			if httputil.IsPublicRoute(r.Method, r.URL.Path) {
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
			r.Header.Set("X-Identity-Id", info.UserID)
			r.Header.Set("X-Session-Id", info.SessionID)
			r.Header.Set("X-Token-Type", info.TokenType)
			if info.OrgID != "" {
				r.Header.Set("X-Org-Id", info.OrgID)
			}

			// Inject session_id into context for tracing downstream
			ctx := telemetry.WithSessionID(r.Context(), info.SessionID)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

type responseWriterWrapper struct {
	http.ResponseWriter

	statusCode int
}

func (rw *responseWriterWrapper) WriteHeader(code int) {
	rw.statusCode = code
	rw.ResponseWriter.WriteHeader(code)
}

// RequestLogMiddleware logs authenticated API requests via the structured
// logging system. Request records flow through the cache sink (Tier 2) and
// are batch-drained to the analytics backend. This replaces the old
// EventStreamMiddleware that wrote directly to the events table.
func RequestLogMiddleware() func(http.Handler) http.Handler {
	logger := logging.New(logging.StreamRequest)
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			start := time.Now()

			rw := &responseWriterWrapper{ResponseWriter: w, statusCode: http.StatusOK}
			next.ServeHTTP(rw, r)

			duration := time.Since(start).Milliseconds()

			actorID := r.Header.Get("X-Identity-Id")
			if actorID == "" {
				// Don't log unauthenticated requests.
				return
			}

			logger.InfoContext(r.Context(), "request.api",
				"method", r.Method,
				"path", r.URL.Path,
				"status", rw.statusCode,
				"duration_ms", duration,
				"actor_id", actorID,
				"trace_id", telemetry.TraceIDFromContext(r.Context()),
				"span_id", telemetry.SpanIDFromContext(r.Context()),
				"session_id", telemetry.SessionIDFromContext(r.Context()),
				"flow_id", telemetry.FlowIDFromContext(r.Context()),
				"device_fingerprint", telemetry.FingerprintFromContext(r.Context()),
			)
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
// DEPRECATED: FGA middleware (FGAGate) handles authorization for API routes.
// Kept for backward compatibility with non-API routes (e.g., UI handlers).
func (a *API) requireAdmin(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		callerID, err := a.resolveCallerIdentity(r)
		if err != nil {
			writeError(w, http.StatusUnauthorized, "authentication required")
			return
		}

		// Use FGA to check admin access when available.
		if svc := FGAService; svc != nil {
			allowed, err := svc.Check(r.Context(), "user:"+callerID, "admin", "instance:default")
			if err != nil || !allowed {
				writeError(w, http.StatusForbidden, "admin access required")
				return
			}
		}

		next(w, r)
	}
}

// noopMiddleware is a passthrough middleware that does nothing.
// Used to replace requireAdmin in route registration when FGA handles authz.
func noopMiddleware(next http.HandlerFunc) http.HandlerFunc {
	return next
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
	r.Header.Set("X-Identity-Id", info.UserID)
	r.Header.Set("X-Session-Id", info.SessionID)
	r.Header.Set("X-Token-Type", info.TokenType)

	// Notice: Downstream handlers might use context.
	// Since this is called mid-flight inside requireAdmin, we don't recreate the request context here.
	// AuthGate handles the primary session context injection.

	return info.UserID, nil
}

// extractToken gets the session token from either the Authorization header or cookie.
// This is the API-bound version used by requireAdmin/requireSession.
func (a *API) extractToken(r *http.Request) string {
	return extractTokenFromRequest(r, a.cookies)
}
