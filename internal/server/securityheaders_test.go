package server

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/zitadel/zitadel/internal/config"
)

func TestSecurityHeaders_Defaults(t *testing.T) {
	cfg := config.SecurityHeadersConfig{
		HSTSEnabled:         true,
		HSTSMaxAge:          63072000,
		HSTSSubdomains:      true,
		CSPEnabled:          true,
		XFrameOptions:       "DENY",
		XContentTypeOptions: true,
		ReferrerPolicy:      "strict-origin-when-cross-origin",
		PermissionsPolicy:   "camera=(), microphone=()",
		CrossOriginOpener:   "same-origin",
	}

	middleware := SecurityHeaders(cfg, true)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
	}))

	r := httptest.NewRequest("GET", "/", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	tests := []struct {
		header string
		want   string
	}{
		{"Strict-Transport-Security", "max-age=63072000; includeSubDomains"},
		{"X-Frame-Options", "DENY"},
		{"X-Content-Type-Options", "nosniff"},
		{"Referrer-Policy", "strict-origin-when-cross-origin"},
		{"Permissions-Policy", "camera=(), microphone=()"},
		{"Cross-Origin-Opener-Policy", "same-origin"},
	}

	for _, tt := range tests {
		got := w.Header().Get(tt.header)
		if got != tt.want {
			t.Errorf("%s = %q, want %q", tt.header, got, tt.want)
		}
	}

	// CSP should be set
	csp := w.Header().Get("Content-Security-Policy")
	if csp == "" {
		t.Error("Content-Security-Policy not set")
	}
}

func TestSecurityHeaders_NoHSTSWithoutTLS(t *testing.T) {
	cfg := config.SecurityHeadersConfig{
		HSTSEnabled: true,
		HSTSMaxAge:  63072000,
	}

	// isSecure = false
	middleware := SecurityHeaders(cfg, false)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
	}))

	r := httptest.NewRequest("GET", "/", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	if got := w.Header().Get("Strict-Transport-Security"); got != "" {
		t.Errorf("HSTS should not be set without TLS, got %q", got)
	}
}

func TestSecurityHeaders_CSPOverride(t *testing.T) {
	cfg := config.SecurityHeadersConfig{
		CSPEnabled: true,
		CSPPolicy:  "default-src 'none'",
	}

	middleware := SecurityHeaders(cfg, false)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
	}))

	r := httptest.NewRequest("GET", "/", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	got := w.Header().Get("Content-Security-Policy")
	if got != "default-src 'none'" {
		t.Errorf("CSP = %q, want custom override", got)
	}
}

func TestSecurityHeaders_HSTSPreload(t *testing.T) {
	cfg := config.SecurityHeadersConfig{
		HSTSEnabled:    true,
		HSTSMaxAge:     63072000,
		HSTSSubdomains: true,
		HSTSPreload:    true,
	}

	middleware := SecurityHeaders(cfg, true)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
	}))

	r := httptest.NewRequest("GET", "/", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	got := w.Header().Get("Strict-Transport-Security")
	want := "max-age=63072000; includeSubDomains; preload"
	if got != want {
		t.Errorf("HSTS = %q, want %q", got, want)
	}
}
