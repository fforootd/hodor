package fga

import (
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"strings"
)

// Middleware provides FGA-based authorization for API requests.
// It replaces the old requireAdmin capability check with
// relationship-based authorization checks.
type Middleware struct {
	svc     *Service
	configs map[string]AuthZConfig // keyed by FGA type
	routes  map[string]string      // route prefix → FGA type
}

// NewMiddleware creates a new FGA middleware with catalog-driven config.
func NewMiddleware(svc *Service) *Middleware {
	configs, routes := BuildAuthZFromCatalog()
	return &Middleware{svc: svc, configs: configs, routes: routes}
}

// Gate is the authorization middleware. For each request it:
//  1. Resolves which FGA type the route maps to
//  2. Determines the required permission from HTTP method + collection/resource
//  3. Runs an FGA Check against the system store
//  4. Returns 403 if denied
//
// Gate expects to run AFTER AuthGate (authn), so X-Identity-Id is set.
func (m *Middleware) Gate(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Skip public routes — AuthGate already handles this, but be safe.
		if httputil.IsPublicRoute(r.Method, r.URL.Path) {
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
		fgaType, resourceID := m.resolveRouteType(r.URL.Path)
		if fgaType == "" {
			// Unknown route — allow (non-FGA routes like healthz).
			next.ServeHTTP(w, r)
			return
		}

		// Look up the AuthZ config for this type.
		cfg, ok := m.configs[fgaType]
		if !ok {
			next.ServeHTTP(w, r)
			return
		}

		// Pick the right permission: collection vs resource.
		var permission string
		if resourceID == "" {
			permission = cfg.CollectionPerms[r.Method]
		} else {
			permission = cfg.ResourcePerms[r.Method]
		}
		if permission == "" {
			// No permission defined for this method — allow.
			next.ServeHTTP(w, r)
			return
		}

		// Build the FGA object to check against.
		object := cfg.resolveObject(resourceID, r)

		// Run the check.
		allowed, err := m.svc.Check(r.Context(), "user:"+userID, permission, object)
		if err != nil {
			logging.Printf("[fga] check error: user=%s perm=%s obj=%s err=%v", userID, permission, object, err)
			// On FGA errors, deny by default (secure fail-closed).
			httputil.WriteError(w, http.StatusForbidden, "authorization check failed")
			return
		}

		if !allowed {
			logging.Printf("[fga] denied: user=%s perm=%s obj=%s", userID, permission, object)
			httputil.WriteError(w, http.StatusForbidden, "insufficient permissions")
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
				httputil.WriteError(w, http.StatusUnauthorized, "authentication required")
				return
			}

			allowed, err := m.svc.Check(r.Context(), "user:"+userID, permission, object)
			if err != nil || !allowed {
				httputil.WriteError(w, http.StatusForbidden, "insufficient permissions")
				return
			}

			next(w, r)
		}
	}
}

// resolveRouteType determines the FGA type and resource ID from the URL path.
func (m *Middleware) resolveRouteType(path string) (fgaType string, resourceID string) {
	// Try longest prefix match.
	bestPrefix := ""
	for prefix, fType := range m.routes {
		if strings.HasPrefix(path, prefix) && len(prefix) > len(bestPrefix) {
			bestPrefix = prefix
			fgaType = fType
		}
	}

	if bestPrefix == "" {
		return "", ""
	}

	// Extract resource ID from path (e.g., /v1/orgs/abc123 → abc123).
	remainder := strings.TrimPrefix(path, bestPrefix)
	remainder = strings.TrimPrefix(remainder, "/")
	if idx := strings.Index(remainder, "/"); idx >= 0 {
		resourceID = remainder[:idx]
	} else {
		resourceID = remainder
	}

	return fgaType, resourceID
}

// resolveObject constructs the FGA object string for the check.
// Uses the scope from AuthZConfig to determine the target.
func (cfg AuthZConfig) resolveObject(resourceID string, r *http.Request) string {
	// Resource-level operations with scope override.
	if resourceID != "" && cfg.ResourceScope == "resource" {
		return cfg.FGAType + ":" + resourceID
	}

	// Everything else follows the default scope.
	switch cfg.Scope {
	case "instance":
		return "instance:default"
	case "org":
		return "org:" + resolveOrgID(r)
	default:
		return "instance:default"
	}
}

// resolveOrgID extracts the org context from the request using httputil.
func resolveOrgID(r *http.Request) string {
	return httputil.ResolveOrgID(r, "_global")
}
