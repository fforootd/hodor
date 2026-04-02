package server

import (
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
)

// InstanceGate is middleware that extracts the instance ID from the request
// and stores it in context. In multi-tenant mode (cloud), the instance ID
// comes from the X-Instance-Id header injected by the cloud router/proxy.
// In single-tenant mode (self-hosted default), it always uses DefaultInstanceID.
//
// This middleware does NOT perform domain-to-instance resolution — that
// responsibility belongs to the cloud wrapper (reverse proxy) outside Zitadel.
func InstanceGate(multiTenant bool) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			instanceID := httputil.DefaultInstanceID

			if multiTenant {
				if hdr := r.Header.Get("X-Instance-Id"); hdr != "" {
					instanceID = hdr
				}
			}

			ctx := httputil.WithInstanceID(r.Context(), instanceID)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}
