package httputil

import "net/http"

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
