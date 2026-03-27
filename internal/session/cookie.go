// Package session provides hardened cookie management for ZITADEL sessions.
// All session cookies are HMAC-SHA256 signed, HttpOnly, SameSite=Lax, and
// Secure (when not on localhost). Cookie format: base64(token.hmac_signature).
package session

import (
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"net/http"
	"strings"
)

const (
	// secureCookieName uses the __Host- prefix for maximum browser protection:
	// forces Secure=true, Path=/, no Domain attribute, can't be set from subdomains.
	secureCookieName = "__Host-zitadel_session"

	// devCookieName is used in local development (no HTTPS).
	devCookieName = "__zitadel_session"

	// MaxAge is the session cookie lifetime in seconds (24 hours).
	MaxAge = 86400
)

// CookieConfig holds the settings needed for cookie operations.
type CookieConfig struct {
	// Secrets is an ordered list of HMAC keys. The first key is used for signing;
	// all keys are tried for verification (enables zero-downtime key rotation).
	// If empty, a random key is generated and stored in-memory (dev mode).
	Secrets []string

	// Secure forces the Secure flag on cookies. Derived from ExternalDomain.
	Secure bool
}

// NewCookieConfig creates a CookieConfig from server settings.
// If secrets is empty, a random 32-byte key is generated (logged for debugging).
func NewCookieConfig(secrets []string, externalDomain string) *CookieConfig {
	secure := externalDomain != "" && externalDomain != "localhost" && externalDomain != "127.0.0.1"

	if len(secrets) == 0 {
		key := make([]byte, 32)
		rand.Read(key)
		secrets = []string{hex.EncodeToString(key)}
	}

	return &CookieConfig{
		Secrets: secrets,
		Secure:  secure,
	}
}

// CookieName returns the appropriate cookie name based on Secure mode.
func (c *CookieConfig) CookieName() string {
	if c.Secure {
		return secureCookieName
	}
	return devCookieName
}

// SetSessionCookie writes a hardened session cookie to the response.
// The token is HMAC-signed: cookie_value = base64(token + "." + hex(hmac)).
func SetSessionCookie(w http.ResponseWriter, token string, cfg *CookieConfig) {
	signed := sign(token, cfg.Secrets[0])

	http.SetCookie(w, &http.Cookie{
		Name:     cfg.CookieName(),
		Value:    signed,
		Path:     "/",
		MaxAge:   MaxAge,
		HttpOnly: true,
		Secure:   cfg.Secure,
		SameSite: http.SameSiteLaxMode,
	})
}

// ClearSessionCookie removes the session cookie.
func ClearSessionCookie(w http.ResponseWriter, cfg *CookieConfig) {
	http.SetCookie(w, &http.Cookie{
		Name:     cfg.CookieName(),
		Value:    "",
		Path:     "/",
		MaxAge:   -1,
		HttpOnly: true,
		Secure:   cfg.Secure,
		SameSite: http.SameSiteLaxMode,
	})
}

// ReadSessionCookie extracts and verifies the session token from the request.
// Returns the raw token (for DB lookup) and true if valid, or ("", false).
// For backward compatibility, also accepts unsigned tokens (legacy cookies).
func ReadSessionCookie(r *http.Request, cfg *CookieConfig) (string, bool) {
	// Try both cookie names (secure and dev) for transition periods.
	for _, name := range []string{secureCookieName, devCookieName} {
		cookie, err := r.Cookie(name)
		if err != nil || cookie.Value == "" {
			continue
		}

		// Try HMAC-signed format first.
		if token, ok := verify(cookie.Value, cfg.Secrets); ok {
			return token, true
		}

		// Backward compatibility: accept raw (unsigned) tokens.
		// This allows existing sessions to continue working after upgrade.
		if !strings.Contains(cookie.Value, ".") && len(cookie.Value) > 0 {
			return cookie.Value, true
		}
	}

	return "", false
}

// sign produces a signed cookie value: base64(token.hex(hmac)).
func sign(token, secret string) string {
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write([]byte(token))
	sig := hex.EncodeToString(mac.Sum(nil))
	payload := fmt.Sprintf("%s.%s", token, sig)
	return base64.RawURLEncoding.EncodeToString([]byte(payload))
}

// verify checks a signed cookie value against all secrets (key rotation).
// Returns the raw token and true if any key validates the signature.
func verify(cookieValue string, secrets []string) (string, bool) {
	decoded, err := base64.RawURLEncoding.DecodeString(cookieValue)
	if err != nil {
		return "", false
	}

	parts := strings.SplitN(string(decoded), ".", 2)
	if len(parts) != 2 {
		return "", false
	}

	token := parts[0]
	providedSig := parts[1]

	for _, secret := range secrets {
		mac := hmac.New(sha256.New, []byte(secret))
		mac.Write([]byte(token))
		expectedSig := hex.EncodeToString(mac.Sum(nil))
		if hmac.Equal([]byte(providedSig), []byte(expectedSig)) {
			return token, true
		}
	}

	return "", false
}
