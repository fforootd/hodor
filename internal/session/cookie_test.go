package session

import (
	"encoding/base64"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestSignVerify_RoundTrip(t *testing.T) {
	cfg := NewCookieConfig([]string{"test-secret-key-1"}, "localhost")
	token := "abc123def456"

	signed := sign(token, cfg.Secrets[0])

	got, ok := verify(signed, cfg.Secrets)
	if !ok {
		t.Fatal("expected valid signature")
	}
	if got != token {
		t.Errorf("expected %q, got %q", token, got)
	}
}

func TestSignVerify_WrongKey(t *testing.T) {
	signed := sign("token123", "key-A")
	_, ok := verify(signed, []string{"key-B"})
	if ok {
		t.Fatal("expected invalid signature with wrong key")
	}
}

func TestSignVerify_KeyRotation(t *testing.T) {
	// Sign with old key.
	signed := sign("token123", "old-key")

	// Verify with [new-key, old-key] — should succeed.
	got, ok := verify(signed, []string{"new-key", "old-key"})
	if !ok {
		t.Fatal("expected valid signature with rotated keys")
	}
	if got != "token123" {
		t.Errorf("expected token123, got %q", got)
	}
}

func TestSetSessionCookie_DevMode(t *testing.T) {
	cfg := NewCookieConfig(nil, "localhost")

	w := httptest.NewRecorder()
	SetSessionCookie(w, "my-token", cfg)

	cookies := w.Result().Cookies()
	if len(cookies) != 1 {
		t.Fatalf("expected 1 cookie, got %d", len(cookies))
	}

	c := cookies[0]
	if c.Name != devCookieName {
		t.Errorf("expected %q, got %q", devCookieName, c.Name)
	}
	if c.Secure {
		t.Error("expected Secure=false in dev mode")
	}
	if !c.HttpOnly {
		t.Error("expected HttpOnly=true")
	}
	if c.SameSite != http.SameSiteLaxMode {
		t.Error("expected SameSite=Lax")
	}
	if c.MaxAge != MaxAge {
		t.Errorf("expected MaxAge=%d, got %d", MaxAge, c.MaxAge)
	}
}

func TestSetSessionCookie_SecureMode(t *testing.T) {
	cfg := NewCookieConfig([]string{"secret"}, "example.com")

	w := httptest.NewRecorder()
	SetSessionCookie(w, "my-token", cfg)

	cookies := w.Result().Cookies()
	c := cookies[0]
	if c.Name != secureCookieName {
		t.Errorf("expected %q, got %q", secureCookieName, c.Name)
	}
	if !c.Secure {
		t.Error("expected Secure=true in production mode")
	}
}

func TestClearSessionCookie(t *testing.T) {
	cfg := NewCookieConfig(nil, "localhost")

	w := httptest.NewRecorder()
	ClearSessionCookie(w, cfg)

	cookies := w.Result().Cookies()
	if len(cookies) != 1 {
		t.Fatalf("expected 1 cookie, got %d", len(cookies))
	}
	if cookies[0].MaxAge != -1 {
		t.Errorf("expected MaxAge=-1, got %d", cookies[0].MaxAge)
	}
}

func TestReadSessionCookie_Signed(t *testing.T) {
	cfg := NewCookieConfig([]string{"my-secret"}, "localhost")

	w := httptest.NewRecorder()
	SetSessionCookie(w, "real-token", cfg)

	// Build a request with the cookie.
	req := httptest.NewRequest("GET", "/", nil)
	for _, c := range w.Result().Cookies() {
		req.AddCookie(c)
	}

	token, ok := ReadSessionCookie(req, cfg)
	if !ok {
		t.Fatal("expected valid cookie")
	}
	if token != "real-token" {
		t.Errorf("expected real-token, got %q", token)
	}
}

func TestReadSessionCookie_LegacyUnsigned_Rejected(t *testing.T) {
	cfg := NewCookieConfig([]string{"my-secret"}, "localhost")

	req := httptest.NewRequest("GET", "/", nil)
	req.AddCookie(&http.Cookie{Name: devCookieName, Value: "raw-legacy-token"})

	_, ok := ReadSessionCookie(req, cfg)
	if ok {
		t.Fatal("expected unsigned cookie to be REJECTED — raw cookies are no longer accepted")
	}
}

func TestReadSessionCookie_Empty(t *testing.T) {
	cfg := NewCookieConfig(nil, "localhost")
	req := httptest.NewRequest("GET", "/", nil)

	_, ok := ReadSessionCookie(req, cfg)
	if ok {
		t.Fatal("expected no cookie")
	}
}

// --- OWASP COOK: Tamper Resistance Tests ---

func TestVerify_TamperedSignature(t *testing.T) {
	signed := sign("my-token", "secret-key")
	// Decode, tamper with the HMAC portion, re-encode.
	decoded, _ := base64.RawURLEncoding.DecodeString(signed)
	parts := strings.SplitN(string(decoded), ".", 2)
	tampered := parts[0] + "." + "ff" + parts[1][2:] // corrupt 2 chars of sig
	reencoded := base64.RawURLEncoding.EncodeToString([]byte(tampered))

	_, ok := verify(reencoded, []string{"secret-key"})
	if ok {
		t.Fatal("tampered signature should be rejected")
	}
}

func TestVerify_TamperedToken(t *testing.T) {
	signed := sign("original-token", "secret-key")
	decoded, _ := base64.RawURLEncoding.DecodeString(signed)
	parts := strings.SplitN(string(decoded), ".", 2)
	tampered := "modified-token." + parts[1]
	reencoded := base64.RawURLEncoding.EncodeToString([]byte(tampered))

	_, ok := verify(reencoded, []string{"secret-key"})
	if ok {
		t.Fatal("tampered token body should be rejected")
	}
}

func TestVerify_TruncatedPayload(t *testing.T) {
	// Cookie with no dot separator.
	encoded := base64.RawURLEncoding.EncodeToString([]byte("nodot"))
	_, ok := verify(encoded, []string{"key"})
	if ok {
		t.Fatal("truncated payload (no separator) should be rejected")
	}
}

func TestVerify_InvalidBase64(t *testing.T) {
	_, ok := verify("!!!not-base64!!!", []string{"key"})
	if ok {
		t.Fatal("invalid base64 should be rejected gracefully")
	}
}

func TestVerify_EmptySecretList(t *testing.T) {
	signed := sign("token", "key")
	_, ok := verify(signed, []string{})
	if ok {
		t.Fatal("empty secret list should return false")
	}
}

func TestCookieName_HostPrefix(t *testing.T) {
	cfg := NewCookieConfig([]string{"secret"}, "example.com")
	if cfg.CookieName() != secureCookieName {
		t.Errorf("expected %q, got %q", secureCookieName, cfg.CookieName())
	}
	if !strings.HasPrefix(secureCookieName, "__Host-") {
		t.Errorf("secure cookie name %q should have __Host- prefix", secureCookieName)
	}
}

func TestSetSessionCookie_Path(t *testing.T) {
	cfg := NewCookieConfig(nil, "localhost")
	w := httptest.NewRecorder()
	SetSessionCookie(w, "tok", cfg)

	cookies := w.Result().Cookies()
	if len(cookies) != 1 {
		t.Fatalf("expected 1 cookie, got %d", len(cookies))
	}
	if cookies[0].Path != "/" {
		t.Errorf("expected Path=/, got %q", cookies[0].Path)
	}
}

func TestNewCookieConfig_GeneratesKey(t *testing.T) {
	cfg := NewCookieConfig(nil, "localhost")
	if len(cfg.Secrets) != 1 {
		t.Fatalf("expected 1 auto-generated secret, got %d", len(cfg.Secrets))
	}
	// hex-encoded 32 bytes = 64 chars.
	if len(cfg.Secrets[0]) != 64 {
		t.Errorf("expected auto-generated key of 64 hex chars, got %d", len(cfg.Secrets[0]))
	}
}

func TestReadSessionCookie_BothNames(t *testing.T) {
	cfg := NewCookieConfig([]string{"secret"}, "localhost") // dev mode

	// Set with the secure cookie name (simulating a transition).
	signed := sign("secure-token", "secret")
	req := httptest.NewRequest("GET", "/", nil)
	req.AddCookie(&http.Cookie{Name: secureCookieName, Value: signed})

	token, ok := ReadSessionCookie(req, cfg)
	if !ok {
		t.Fatal("expected to read token from secure cookie name even in dev mode")
	}
	if token != "secure-token" {
		t.Errorf("expected secure-token, got %q", token)
	}
}

func TestReadSessionCookie_UnsignedRejected(t *testing.T) {
	cfg := NewCookieConfig([]string{"my-secret"}, "localhost")

	req := httptest.NewRequest("GET", "/", nil)
	req.AddCookie(&http.Cookie{Name: devCookieName, Value: "raw-legacy-token"})

	_, ok := ReadSessionCookie(req, cfg)
	if ok {
		t.Fatal("unsigned cookie should be REJECTED — raw cookies are no longer accepted")
	}
}
