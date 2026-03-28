package server

import (
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/config"
)

// SecurityHeaders returns middleware that sets production-grade security response headers.
// It reads from SecurityHeadersConfig and applies HSTS, CSP, X-Frame-Options, etc.
func SecurityHeaders(cfg config.SecurityHeadersConfig, isSecure bool) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			h := w.Header()

			// HSTS — only on HTTPS connections.
			if cfg.HSTSEnabled && isSecure {
				val := fmt.Sprintf("max-age=%d", cfg.HSTSMaxAge)
				if cfg.HSTSSubdomains {
					val += "; includeSubDomains"
				}
				if cfg.HSTSPreload {
					val += "; preload"
				}
				h.Set("Strict-Transport-Security", val)
			}

			// Content Security Policy.
			if cfg.CSPEnabled {
				policy := cfg.CSPPolicy
				if policy == "" {
					policy = defaultCSP()
				}
				if cfg.CSPReportURI != "" {
					policy += fmt.Sprintf("; report-uri %s", cfg.CSPReportURI)
				}
				h.Set("Content-Security-Policy", policy)
			}

			// X-Frame-Options.
			if cfg.XFrameOptions != "" {
				h.Set("X-Frame-Options", cfg.XFrameOptions)
			}

			// X-Content-Type-Options.
			if cfg.XContentTypeOptions {
				h.Set("X-Content-Type-Options", "nosniff")
			}

			// Referrer-Policy.
			if cfg.ReferrerPolicy != "" {
				h.Set("Referrer-Policy", cfg.ReferrerPolicy)
			}

			// Permissions-Policy.
			if cfg.PermissionsPolicy != "" {
				h.Set("Permissions-Policy", cfg.PermissionsPolicy)
			}

			// Cross-Origin-Opener-Policy.
			if cfg.CrossOriginOpener != "" {
				h.Set("Cross-Origin-Opener-Policy", cfg.CrossOriginOpener)
			}

			next.ServeHTTP(w, r)
		})
	}
}

// defaultCSP returns a reasonable default Content Security Policy.
func defaultCSP() string {
	return "default-src 'self'; " +
		"script-src 'self'; " +
		"style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; " +
		"font-src 'self' https://fonts.gstatic.com; " +
		"img-src 'self' data: blob:; " +
		"connect-src 'self'; " +
		"frame-ancestors 'none'; " +
		"base-uri 'self'; " +
		"form-action 'self'"
}
