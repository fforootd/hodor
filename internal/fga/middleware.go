package fga

import (
	"fmt"
	"log"
	"net/http"
	"strings"
)

// Middleware provides FGA-based authorization for API requests.
// It replaces the old requireAdmin capability check with
// relationship-based authorization checks.
type Middleware struct {
	svc *Service
}

// NewMiddleware creates a new FGA middleware.
func NewMiddleware(svc *Service) *Middleware {
	return &Middleware{svc: svc}
}

// Gate is the authorization middleware. For each request it:
//  1. Resolves which FGA type the route maps to
//  2. Determines the required permission from HTTP method
//  3. Runs an FGA Check against the system store
//  4. Returns 403 if denied
//
// Gate expects to run AFTER AuthGate (authn), so X-Identity-Id is set.
func (m *Middleware) Gate(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Skip public routes — AuthGate already handles this, but be safe.
		if isPublicFGARoute(r.Method, r.URL.Path) {
			next.ServeHTTP(w, r)
			return
		}

		// Who is asking?
		userID := r.Header.Get("X-Identity-Id")
		if userID == "" {
			// Not authenticated — let AuthGate handle the 401.
			next.ServeHTTP(w, r)
			return
		}

		// What FGA type is this route about?
		fgaType, resourceID := resolveRouteType(r.URL.Path)
		if fgaType == "" {
			// Unknown route — allow (non-FGA routes like healthz).
			next.ServeHTTP(w, r)
			return
		}

		// What permission is needed?
		perms, ok := PermissionMap[fgaType]
		if !ok {
			next.ServeHTTP(w, r)
			return
		}
		permission, ok := perms[r.Method]
		if !ok {
			next.ServeHTTP(w, r)
			return
		}

		// Build the FGA object to check against.
		object := buildCheckObject(fgaType, resourceID, r)

		// When list/create operations fall back to org-level check,
		// remap permissions to the org's domain (e.g., can_read → can_read_entity).
		if strings.HasPrefix(object, "org:") && fgaType != "org" {
			permission = remapToOrgPermission(permission)
		}

		// Run the check.
		allowed, err := m.svc.Check(r.Context(), "user:"+userID, permission, object)
		if err != nil {
			log.Printf("[fga] check error: user=%s perm=%s obj=%s err=%v", userID, permission, object, err)
			// On FGA errors, deny by default (secure fail-closed).
			writeJSONError(w, http.StatusForbidden, "authorization check failed")
			return
		}

		if !allowed {
			log.Printf("[fga] denied: user=%s perm=%s obj=%s", userID, permission, object)
			writeJSONError(w, http.StatusForbidden, "insufficient permissions")
			return
		}

		// Inject FGA context for downstream use.
		r.Header.Set("X-Fga-Type", fgaType)
		r.Header.Set("X-Fga-Permission", permission)

		next.ServeHTTP(w, r)
	})
}

// Require returns a middleware that checks a specific permission on a specific object type.
// Use this for explicit per-route authorization when the generic Gate isn't enough.
//
// Example:
//
//	mux.Handle("POST /v1/schemas", m.Require("schema", "can_manage_schemas", "instance:default")(handler))
func (m *Middleware) Require(fgaType, permission, object string) func(http.HandlerFunc) http.HandlerFunc {
	return func(next http.HandlerFunc) http.HandlerFunc {
		return func(w http.ResponseWriter, r *http.Request) {
			userID := r.Header.Get("X-Identity-Id")
			if userID == "" {
				writeJSONError(w, http.StatusUnauthorized, "authentication required")
				return
			}

			allowed, err := m.svc.Check(r.Context(), "user:"+userID, permission, object)
			if err != nil || !allowed {
				writeJSONError(w, http.StatusForbidden, "insufficient permissions")
				return
			}

			next(w, r)
		}
	}
}

// resolveRouteType determines the FGA type and resource ID from the URL path.
func resolveRouteType(path string) (fgaType string, resourceID string) {
	// Try longest prefix match.
	bestPrefix := ""
	for prefix, fType := range RouteToFGAType {
		if strings.HasPrefix(path, prefix) && len(prefix) > len(bestPrefix) {
			bestPrefix = prefix
			fgaType = fType
		}
	}

	if bestPrefix == "" {
		return "", ""
	}

	// Extract resource ID from path (e.g., /v1/entities/abc123 → abc123).
	remainder := strings.TrimPrefix(path, bestPrefix)
	remainder = strings.TrimPrefix(remainder, "/")
	if idx := strings.Index(remainder, "/"); idx >= 0 {
		resourceID = remainder[:idx]
	} else {
		resourceID = remainder
	}

	return fgaType, resourceID
}

// buildCheckObject constructs the FGA object string for the check.
// For list/create operations (no resource ID), check against the org.
// For read/update/delete operations, check against the specific resource.
func buildCheckObject(fgaType, resourceID string, r *http.Request) string {
	switch {
	// Schema and provider operations → check against instance
	case fgaType == "schema" || fgaType == "provider":
		return "instance:default"

	// Session and entity operations → always org-level
	// Sessions are ephemeral (no per-session FGA tuples).
	// Entities fall back to org until creation handlers wire OnEntityCreated.
	case fgaType == "session" || fgaType == "entity":
		orgID := resolveOrgID(r)
		return "org:" + orgID

	// Org-level checks (creating resources, listing collections)
	case r.Method == "POST" || (resourceID == "" && r.Method == "GET"):
		orgID := resolveOrgID(r)
		if fgaType == "org" && r.Method == "POST" {
			// Creating an org → check against instance
			return "instance:default"
		}
		return "org:" + orgID

	// Setting operations → settings:{type}_{scope}_{scopeID}
	case fgaType == "settings":
		if resourceID != "" {
			return "settings:" + resourceID
		}
		orgID := resolveOrgID(r)
		return "org:" + orgID

	// Resource-level check
	case resourceID != "":
		return fgaType + ":" + resourceID

	// Fallback to org-level
	default:
		orgID := resolveOrgID(r)
		return "org:" + orgID
	}
}

// resolveOrgID extracts the org context from the request.
// Priority: X-Org-Id header → org_id query param → "default".
func resolveOrgID(r *http.Request) string {
	if orgID := r.Header.Get("X-Org-Id"); orgID != "" {
		return orgID
	}
	if orgID := r.URL.Query().Get("org_id"); orgID != "" {
		return orgID
	}
	return "_global"
}

// isPublicFGARoute returns true for paths that skip FGA checks.
func isPublicFGARoute(method, path string) bool {
	// All non-API routes skip FGA
	if !strings.HasPrefix(path, "/v1/") {
		return true
	}

	// Public GET routes
	if method == "GET" {
		publicGET := []string{
			"/v1/schemas",
			"/v1/branding",
			"/v1/auth/settings",
			"/v1/providers/templates",
		}
		for _, p := range publicGET {
			if path == p || strings.HasPrefix(path, p+"/") {
				return true
			}
		}
	}

	// Public POST routes
	if method == "POST" {
		publicPOST := []string{
			"/v1/login/",
		}
		for _, p := range publicPOST {
			if strings.HasPrefix(path, p) {
				return true
			}
		}
	}

	return false
}

// remapToOrgPermission translates resource-level permissions to org-scoped
// equivalents for collection/list endpoints that check against the org.
func remapToOrgPermission(perm string) string {
	switch perm {
	case "can_read":
		return "can_read_entity"
	case "can_update":
		return "can_update_entity"
	case "can_delete":
		return "can_delete_entity"
	case "can_revoke":
		return "can_read_entity" // session revoke at org level = read access
	default:
		return perm // already an org-level permission (can_manage_*, can_create_entity, etc.)
	}
}

func writeJSONError(w http.ResponseWriter, status int, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_, _ = fmt.Fprintf(w, `{"error":%q,"code":%d}`, msg, status)
}
