package session

import (
	"net/http"
	"net/http/httptest"
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
