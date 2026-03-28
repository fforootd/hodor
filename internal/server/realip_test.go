package server

import (
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
)

func mustParseCIDR(cidr string) *net.IPNet {
	_, n, err := net.ParseCIDR(cidr)
	if err != nil {
		panic(err)
	}
	return n
}

func TestRealIP_NoConfig(t *testing.T) {
	middleware := RealIP(nil)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ip := FromContext(r)
		w.Write([]byte(ip))
	}))

	r := httptest.NewRequest("GET", "/", nil)
	r.RemoteAddr = "1.2.3.4:5678"
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	if w.Body.String() != "1.2.3.4" {
		t.Errorf("got %q, want 1.2.3.4", w.Body.String())
	}
}

func TestRealIP_TrustedProxy_Standard(t *testing.T) {
	cfg := &RealIPConfig{
		TrustedCIDRs: []*net.IPNet{mustParseCIDR("10.0.0.0/8")},
		Mode:         "standard",
	}
	middleware := RealIP(cfg)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ip := FromContext(r)
		w.Write([]byte(ip))
	}))

	r := httptest.NewRequest("GET", "/", nil)
	r.RemoteAddr = "10.0.0.1:5678" // Trusted proxy
	r.Header.Set("X-Forwarded-For", "203.0.113.50, 10.0.0.2")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	// Should return rightmost untrusted IP (203.0.113.50)
	if w.Body.String() != "203.0.113.50" {
		t.Errorf("got %q, want 203.0.113.50", w.Body.String())
	}
}

func TestRealIP_UntrustedProxy_Ignored(t *testing.T) {
	cfg := &RealIPConfig{
		TrustedCIDRs: []*net.IPNet{mustParseCIDR("10.0.0.0/8")},
		Mode:         "standard",
	}
	middleware := RealIP(cfg)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ip := FromContext(r)
		w.Write([]byte(ip))
	}))

	r := httptest.NewRequest("GET", "/", nil)
	r.RemoteAddr = "172.16.0.1:5678" // NOT a trusted proxy
	r.Header.Set("X-Forwarded-For", "1.2.3.4") // Spoofed
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	// Should use RemoteAddr, NOT the spoofed XFF
	if w.Body.String() != "172.16.0.1" {
		t.Errorf("got %q, want 172.16.0.1 (spoofed XFF ignored)", w.Body.String())
	}
}

func TestRealIP_Cloudflare(t *testing.T) {
	cfg := &RealIPConfig{
		TrustedCIDRs: []*net.IPNet{mustParseCIDR("10.0.0.0/8")},
		Mode:         "cloudflare",
	}
	middleware := RealIP(cfg)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ip := FromContext(r)
		w.Write([]byte(ip))
	}))

	r := httptest.NewRequest("GET", "/", nil)
	r.RemoteAddr = "10.0.0.1:5678"
	r.Header.Set("Cf-Connecting-Ip", "198.51.100.42")
	r.Header.Set("X-Forwarded-For", "198.51.100.42, 10.0.0.1")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	// CF-Connecting-IP takes priority in cloudflare mode
	if w.Body.String() != "198.51.100.42" {
		t.Errorf("got %q, want 198.51.100.42", w.Body.String())
	}
}

func TestRealIP_CustomHeader(t *testing.T) {
	cfg := &RealIPConfig{
		TrustedCIDRs: []*net.IPNet{mustParseCIDR("10.0.0.0/8")},
		Mode:         "custom",
		CustomHeader: "X-Custom-Client-IP",
	}
	middleware := RealIP(cfg)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ip := FromContext(r)
		w.Write([]byte(ip))
	}))

	r := httptest.NewRequest("GET", "/", nil)
	r.RemoteAddr = "10.0.0.1:5678"
	r.Header.Set("X-Custom-Client-Ip", "192.0.2.77")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	if w.Body.String() != "192.0.2.77" {
		t.Errorf("got %q, want 192.0.2.77", w.Body.String())
	}
}

func TestRealIP_JP3ATags(t *testing.T) {
	cfg := &RealIPConfig{
		TrustedCIDRs: []*net.IPNet{mustParseCIDR("10.0.0.0/8")},
		Mode:         "standard",
	}
	middleware := RealIP(cfg)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		meta := ProxyMetaFromContext(r)
		if meta == nil {
			t.Fatal("ProxyMeta is nil")
		}
		if meta.ClientID != "client-123" {
			t.Errorf("ClientID = %q, want client-123", meta.ClientID)
		}
		if meta.DeviceFingerprint != "fp-abc" {
			t.Errorf("DeviceFingerprint = %q, want fp-abc", meta.DeviceFingerprint)
		}
		w.Write([]byte("ok"))
	}))

	r := httptest.NewRequest("GET", "/", nil)
	r.RemoteAddr = "10.0.0.1:5678"
	r.Header.Set("X-Forwarded-For", "1.2.3.4")
	r.Header.Set("Jp3a-Client-Id", "client-123")
	r.Header.Set("Jp3a-Device-Fingerprint", "fp-abc")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)
}

func TestRealIP_MultiHop_RightmostUntrusted(t *testing.T) {
	cfg := &RealIPConfig{
		TrustedCIDRs: []*net.IPNet{
			mustParseCIDR("10.0.0.0/8"),
			mustParseCIDR("172.16.0.0/12"),
		},
		Mode: "standard",
	}
	middleware := RealIP(cfg)
	handler := middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ip := FromContext(r)
		w.Write([]byte(ip))
	}))

	r := httptest.NewRequest("GET", "/", nil)
	r.RemoteAddr = "10.0.0.1:5678"
	// Multi-hop: client → CDN (public) → LB (10.x) → proxy (172.x) → server
	r.Header.Set("X-Forwarded-For", "spoofed.fake, 203.0.113.99, 10.0.0.5, 172.16.0.2")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)

	// Should pick 203.0.113.99 (rightmost untrusted), NOT spoofed.fake
	if w.Body.String() != "203.0.113.99" {
		t.Errorf("got %q, want 203.0.113.99", w.Body.String())
	}
}
