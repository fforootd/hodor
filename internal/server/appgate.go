package server

import (
	"net"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/config"
)

// AppGate returns middleware that controls access to individual apps.
// It can disable apps entirely (returns 404) or restrict by IP allowlist.
// IP checks use the real client IP from the RealIP middleware context.
func AppGate(paths *config.PathConfig, access *config.AppAccessConfig) func(http.Handler) http.Handler {
	// Pre-parse CIDR allowlists at startup.
	consoleNets := parseCIDRs(access.Console.IPAllow)
	adminNets := parseCIDRs(access.Admin.IPAllow)
	apiNets := parseCIDRs(access.API.IPAllow)
	loginNets := parseCIDRs(access.Login.IPAllow)

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			reqPath := r.URL.Path

			// Check console access.
			if paths.Console != "" && strings.HasPrefix(reqPath, paths.Console) {
				if !access.Console.Enabled {
					http.NotFound(w, r)
					return
				}
				if !checkIPAllow(r, consoleNets) {
					http.Error(w, "Forbidden", http.StatusForbidden)
					return
				}
			}

			// Check admin access.
			if paths.Admin != "" && strings.HasPrefix(reqPath, paths.Admin) {
				if !access.Admin.Enabled {
					http.NotFound(w, r)
					return
				}
				if !checkIPAllow(r, adminNets) {
					http.Error(w, "Forbidden", http.StatusForbidden)
					return
				}
			}

			// Check API access.
			apiPrefix := paths.API + "/v1/"
			if strings.HasPrefix(reqPath, apiPrefix) {
				if !access.API.Enabled {
					http.NotFound(w, r)
					return
				}
				if !checkIPAllow(r, apiNets) {
					http.Error(w, "Forbidden", http.StatusForbidden)
					return
				}
			}

			// Check login access.
			if paths.Login != "" && strings.HasPrefix(reqPath, paths.Login) {
				if !access.Login.Enabled {
					http.NotFound(w, r)
					return
				}
				if !checkIPAllow(r, loginNets) {
					http.Error(w, "Forbidden", http.StatusForbidden)
					return
				}
			}

			next.ServeHTTP(w, r)
		})
	}
}

// checkIPAllow returns true if the request's real IP is in the allowlist,
// or if the allowlist is empty (allow all).
func checkIPAllow(r *http.Request, nets []*net.IPNet) bool {
	if len(nets) == 0 {
		return true // no restriction
	}

	clientIP := FromContext(r) // uses RealIP middleware context
	ip := net.ParseIP(clientIP)
	if ip == nil {
		return false
	}

	for _, cidr := range nets {
		if cidr.Contains(ip) {
			return true
		}
	}
	return false
}

// parseCIDRs converts CIDR strings to net.IPNet slices, ignoring parse errors.
func parseCIDRs(cidrs []string) []*net.IPNet {
	var nets []*net.IPNet
	for _, cidr := range cidrs {
		_, ipNet, err := net.ParseCIDR(strings.TrimSpace(cidr))
		if err == nil {
			nets = append(nets, ipNet)
		}
	}
	return nets
}
