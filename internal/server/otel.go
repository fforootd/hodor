package server

import (
	"context"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/crypto"
)

type contextKey string

const traceIDKey contextKey = "trace_id"

// WithTraceID adds a trace_id to the context.
func WithTraceID(ctx context.Context, traceID string) context.Context {
	return context.WithValue(ctx, traceIDKey, traceID)
}

// OTelMiddleware injects trace_id and session_id into the request context.
// If a W3C traceparent header is present, extracts the trace ID from it.
// Otherwise, generates a new random trace ID.
func OTelMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()

		// Extract or generate trace ID.
		traceID := extractTraceID(r)
		if traceID == "" {
			traceID = generateTraceID()
		}
		ctx = WithTraceID(ctx, traceID)

		// Set trace ID in response header for correlation.
		w.Header().Set("X-Trace-Id", traceID)

		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// extractTraceID extracts trace ID from W3C traceparent header.
func extractTraceID(r *http.Request) string {
	tp := r.Header.Get("Traceparent")
	if tp == "" {
		return ""
	}
	parts := strings.SplitN(tp, "-", 4)
	if len(parts) >= 2 && len(parts[1]) == 32 {
		return parts[1]
	}
	return ""
}

// generateTraceID creates a random 32-char hex trace ID (128-bit).
func generateTraceID() string {
	return crypto.MustRandomHex(16)
}
