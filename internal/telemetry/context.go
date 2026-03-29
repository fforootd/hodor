package telemetry

import "context"

type contextKey string

const (
	requestIDKey      contextKey = "request_id"
	sessionIDKey      contextKey = "session_id"
	flowIDKey         contextKey = "flow_id"
	fingerprintKey    contextKey = "device_fingerprint"
	clientIDKey       contextKey = "client_id"
	tokenIDKey        contextKey = "token_id"
	delegationTypeKey contextKey = "delegation_type"
	sdkNameKey        contextKey = "sdk_name"
	sdkVersionKey     contextKey = "sdk_version"
)

// --- Request ID (replaces trace_id) ---

// WithRequestID adds a request_id to the context.
func WithRequestID(ctx context.Context, requestID string) context.Context {
	return context.WithValue(ctx, requestIDKey, requestID)
}

// RequestIDFromContext gets the request_id from the context.
func RequestIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(requestIDKey).(string); ok {
		return val
	}
	return ""
}

// --- Session ID ---

// WithSessionID adds a session_id to the context.
func WithSessionID(ctx context.Context, sessionID string) context.Context {
	return context.WithValue(ctx, sessionIDKey, sessionID)
}

// SessionIDFromContext gets the session_id from the context.
func SessionIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(sessionIDKey).(string); ok {
		return val
	}
	return ""
}

// --- Flow ID ---

// WithFlowID adds a flow_id to the context.
func WithFlowID(ctx context.Context, flowID string) context.Context {
	return context.WithValue(ctx, flowIDKey, flowID)
}

// FlowIDFromContext gets the flow_id from the context.
func FlowIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(flowIDKey).(string); ok {
		return val
	}
	return ""
}

// --- Fingerprint ---

// WithFingerprint adds a device fingerprint to the context.
func WithFingerprint(ctx context.Context, fingerprint string) context.Context {
	return context.WithValue(ctx, fingerprintKey, fingerprint)
}

// FingerprintFromContext gets the device fingerprint from the context.
func FingerprintFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(fingerprintKey).(string); ok {
		return val
	}
	return ""
}

// --- Client ID (app/agent that made the call) ---

// WithClientID adds a client_id to the context.
func WithClientID(ctx context.Context, clientID string) context.Context {
	return context.WithValue(ctx, clientIDKey, clientID)
}

// ClientIDFromContext gets the client_id from the context.
func ClientIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(clientIDKey).(string); ok {
		return val
	}
	return ""
}

// --- Token ID ---

// WithTokenID adds a token_id to the context.
func WithTokenID(ctx context.Context, tokenID string) context.Context {
	return context.WithValue(ctx, tokenIDKey, tokenID)
}

// TokenIDFromContext gets the token_id from the context.
func TokenIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(tokenIDKey).(string); ok {
		return val
	}
	return ""
}

// --- Delegation Type ---

// WithDelegationType adds a delegation_type to the context.
func WithDelegationType(ctx context.Context, dt string) context.Context {
	return context.WithValue(ctx, delegationTypeKey, dt)
}

// DelegationTypeFromContext gets the delegation_type from the context.
func DelegationTypeFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(delegationTypeKey).(string); ok {
		return val
	}
	return ""
}

// --- SDK Info ---

// WithSDKName adds the SDK name to the context.
func WithSDKName(ctx context.Context, name string) context.Context {
	return context.WithValue(ctx, sdkNameKey, name)
}

// SDKNameFromContext gets the SDK name from the context.
func SDKNameFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(sdkNameKey).(string); ok {
		return val
	}
	return ""
}

// WithSDKVersion adds the SDK version to the context.
func WithSDKVersion(ctx context.Context, version string) context.Context {
	return context.WithValue(ctx, sdkVersionKey, version)
}

// SDKVersionFromContext gets the SDK version from the context.
func SDKVersionFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(sdkVersionKey).(string); ok {
		return val
	}
	return ""
}

// --- Backward compatibility aliases ---
// These allow callers that still use the old names to compile during migration.

// Deprecated: Use WithRequestID instead.
func WithTraceID(ctx context.Context, traceID string) context.Context {
	return WithRequestID(ctx, traceID)
}

// Deprecated: Use RequestIDFromContext instead.
func TraceIDFromContext(ctx context.Context) string {
	return RequestIDFromContext(ctx)
}

// Deprecated: span_id is now demoted to metadata. These are no-ops for compat.
func WithSpanID(ctx context.Context, _ string) context.Context { return ctx }
func SpanIDFromContext(_ context.Context) string               { return "" }
func WithParentSpanID(ctx context.Context, _ string) context.Context {
	return ctx
}
func ParentSpanIDFromContext(_ context.Context) string { return "" }
