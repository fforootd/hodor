package server

import (
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/telemetry"
)

// OTelMiddleware injects trace_id, span_id, and session_id into the request context.
// If a W3C traceparent header is present, extracts the trace ID and optional span ID from it.
// Otherwise, generates new identifiers.
func OTelMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()

		// Extract or generate trace ID and span ID.
		traceID, incomingSpanID := extractTraceparent(r)
		if traceID == "" {
			traceID = generateTraceID()
		}

		// The incoming span ID becomes the parent; we always generate a fresh child span.
		parentSpanID := incomingSpanID
		spanID := generateSpanID()

		ctx = telemetry.WithTraceID(ctx, traceID)
		ctx = telemetry.WithSpanID(ctx, spanID)
		ctx = telemetry.WithParentSpanID(ctx, parentSpanID)

		// Also check if a Session ID header exists (set by load balancer or elsewhere)
		if sessionID := r.Header.Get("X-Session-Id"); sessionID != "" {
			ctx = telemetry.WithSessionID(ctx, sessionID)
		}

		// Check for Flow ID header (set by login WC during flow steps).
		if flowID := r.Header.Get("X-Flow-Id"); flowID != "" {
			ctx = telemetry.WithFlowID(ctx, flowID)
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
