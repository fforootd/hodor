package telemetry

import "context"

type contextKey string

const (
	traceIDKey      contextKey = "trace_id"
	spanIDKey       contextKey = "span_id"
	parentSpanIDKey contextKey = "parent_span_id"
	sessionIDKey    contextKey = "session_id"
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

// WithParentSpanID adds a parent_span_id to the context.
func WithParentSpanID(ctx context.Context, parentSpanID string) context.Context {
	return context.WithValue(ctx, parentSpanIDKey, parentSpanID)
}

// ParentSpanIDFromContext gets the parent_span_id from the context.
func ParentSpanIDFromContext(ctx context.Context) string {
	if val, ok := ctx.Value(parentSpanIDKey).(string); ok {
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
