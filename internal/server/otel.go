package server

import (
	"context"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/crypto"
)

type contextKey string

const (
	traceIDKey   contextKey = "trace_id"
	spanIDKey    contextKey = "span_id"
	sessionIDKey contextKey = "session_id"
)

// WithTraceID adds a trace_id to the context.
func WithTraceID(ctx context.Context, traceID string) context.Context {
	return context.WithValue(ctx, traceIDKey, traceID)
}

// WithSpanID adds a span_id to the context.
func WithSpanID(ctx context.Context, spanID string) context.Context {
	return context.WithValue(ctx, spanIDKey, spanID)
}

// WithSessionID adds a session_id to the context.
func WithSessionID(ctx context.Context, sessionID string) context.Context {
	return context.WithValue(ctx, sessionIDKey, sessionID)
}

// TraceIDFromContext gets the trace_id from the context.
func TraceIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(traceIDKey).(string); ok {
		return val
	}
	return ""
}

// SpanIDFromContext gets the span_id from the context.
func SpanIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(spanIDKey).(string); ok {
		return val
	}
	return ""
}

// SessionIDFromContext gets the session_id from the context.
func SessionIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(sessionIDKey).(string); ok {
		return val
	}
	return ""
}

// OTelMiddleware injects trace_id, span_id, and session_id into the request context.
// If a W3C traceparent header is present, extracts the trace ID and optional span ID from it.
// Otherwise, generates new identifiers.
func OTelMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()

		// Extract or generate trace ID and span ID.
		traceID, spanID := extractTraceparent(r)
		if traceID == "" {
			traceID = generateTraceID()
		}
		if spanID == "" {
			spanID = generateSpanID()
		}

		ctx = WithTraceID(ctx, traceID)
		ctx = WithSpanID(ctx, spanID)

		// Also check if a Session ID header exists (set by load balancer or elsewhere)
		if sessionID := r.Header.Get("X-Session-Id"); sessionID != "" {
			ctx = WithSessionID(ctx, sessionID)
		}

		// Set trace headers in response for correlation.
		w.Header().Set("X-Trace-Id", traceID)
		w.Header().Set("X-Span-Id", spanID)

		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// extractTraceparent extracts trace ID and span ID from W3C traceparent header.
func extractTraceparent(r *http.Request) (string, string) {
	tp := r.Header.Get("Traceparent")
	if tp == "" {
		return "", ""
	}
	parts := strings.SplitN(tp, "-", 4)
	if len(parts) >= 3 && len(parts[1]) == 32 && len(parts[2]) == 16 {
		return parts[1], parts[2]
	}
	return "", ""
}

// generateTraceID creates a random 32-char hex trace ID (128-bit).
func generateTraceID() string {
	return crypto.MustRandomHex(16)
}

// generateSpanID creates a random 16-char hex span ID (64-bit).
func generateSpanID() string {
	return crypto.MustRandomHex(8)
}
