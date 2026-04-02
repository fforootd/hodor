package httputil

import (
	"context"
	"net/http"
)

// DefaultInstanceID is the instance_id used for self-hosted single-tenant
// deployments. When no X-Instance-Id header is present (the default), all
// data is scoped to this value. Self-hosters never see or configure this.
const DefaultInstanceID = "default"

type ctxKey int

const instanceIDKey ctxKey = iota

// WithInstanceID stores the instance ID in the context.
func WithInstanceID(ctx context.Context, id string) context.Context {
	return context.WithValue(ctx, instanceIDKey, id)
}

// InstanceIDFromContext returns the instance ID from context.
// Never returns empty — defaults to DefaultInstanceID if not set.
func InstanceIDFromContext(ctx context.Context) string {
	if id, ok := ctx.Value(instanceIDKey).(string); ok && id != "" {
		return id
	}
	return DefaultInstanceID
}

// ResolveOrgID extracts the org context from a request.
// Priority: X-Org-Id header → "org" query param → fallback.
// Pass "" as fallback to get empty string when no org context is present.
func ResolveOrgID(r *http.Request, fallback string) string {
	if orgID := r.Header.Get("X-Org-Id"); orgID != "" {
		return orgID
	}
	if orgID := r.URL.Query().Get("org"); orgID != "" {
		return orgID
	}
	return fallback
}
