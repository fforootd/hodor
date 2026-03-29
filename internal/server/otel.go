package server

import (
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/telemetry"
)

// RequestContextMiddleware enriches every request context with correlation IDs.
// If a W3C traceparent header is present, extracts the trace ID and stores it as request_id.
// Otherwise, generates a new 128-bit hex request_id.
// Also reads SDK info headers (informational, not validated for authz).
func RequestContextMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()

		// Extract or generate request ID (W3C traceparent compatible).
		requestID, _ := extractTraceparent(r)
		if requestID == "" {
			requestID = generateRequestID()
		}
		ctx = telemetry.WithRequestID(ctx, requestID)

		// Session ID from header (set by load balancer or AuthGate).
		if sessionID := r.Header.Get("X-Session-Id"); sessionID != "" {
			ctx = telemetry.WithSessionID(ctx, sessionID)
		}

		// Flow ID header (set by login WC during flow steps).
		if flowID := r.Header.Get("X-Flow-Id"); flowID != "" {
			ctx = telemetry.WithFlowID(ctx, flowID)
		}

		// Fingerprint header (set by clients for device correlation).
		if fingerprint := r.Header.Get("X-Fingerprint"); fingerprint != "" {
			ctx = telemetry.WithFingerprint(ctx, fingerprint)
		}

		// SDK info headers (informational, not trusted for authz).
		if sdkName := r.Header.Get("X-SDK-Name"); sdkName != "" {
			ctx = telemetry.WithSDKName(ctx, sdkName)
			if sdkVersion := r.Header.Get("X-SDK-Version"); sdkVersion != "" {
				ctx = telemetry.WithSDKVersion(ctx, sdkVersion)
			}
		}

		// Set request ID in response for correlation.
		w.Header().Set("X-Request-Id", requestID)

		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// OTelMiddleware is a backward-compatible alias for RequestContextMiddleware.
// Deprecated: Use RequestContextMiddleware directly.
func OTelMiddleware(next http.Handler) http.Handler {
	return RequestContextMiddleware(next)
}

// extractTraceparent extracts trace ID and span ID from W3C traceparent header.
// The trace ID becomes the request_id. The span ID is discarded (demoted to metadata).
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

// generateRequestID creates a random 32-char hex ID (128-bit, W3C trace ID compatible).
func generateRequestID() string {
	return crypto.MustRandomHex(16)
}
