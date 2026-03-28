package session

import (
	"strings"
	"testing"
)

// FuzzCookieVerify stresses the cookie verification path with arbitrary inputs.
// Invariant: must never panic.
func FuzzCookieVerify(f *testing.F) {
	// Seed corpus.
	f.Add("")
	f.Add("not-base64")
	f.Add("aGVsbG8ud29ybGQ") // "hello.world" base64
	f.Add("dG9rZW4uZmFrZXNpZw")
	f.Add(string(make([]byte, 1000)))

	secrets := []string{"test-secret"}

	f.Fuzz(func(t *testing.T, input string) {
		// Must not panic.
		verify(input, secrets)
	})
}

// FuzzCookieSign stresses the sign function with arbitrary tokens.
// Invariant: must never panic.
func FuzzCookieSign(f *testing.F) {
	f.Add("")
	f.Add("normal-token")
	f.Add("token-with-special-chars-!@#$%^&*()")
	f.Add(string(make([]byte, 10000)))

	f.Fuzz(func(t *testing.T, token string) {
		// Must not panic.
		sign(token, "test-secret")
	})
}

// FuzzCookieSignVerifyRoundTrip ensures sign→verify always round-trips.
// Invariant: if sign succeeds, verify must succeed with the same secret.
// NOTE: Tokens must NOT contain dots — the cookie format uses dot as
// separator (token.hmac_signature). All Zitadel tokens are hex-encoded.
func FuzzCookieSignVerifyRoundTrip(f *testing.F) {
	f.Add("simple-token")
	f.Add("")
	f.Add("unicode-🔐-token")
	f.Add("zit_ses_abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890")

	f.Fuzz(func(t *testing.T, token string) {
		// Skip tokens with dots — the cookie format uses dot as separator.
		if strings.Contains(token, ".") {
			t.Skip("dot in token breaks cookie format by design")
		}
		secret := "round-trip-secret"
		signed := sign(token, secret)
		got, ok := verify(signed, []string{secret})
		if !ok {
			t.Fatalf("verify failed for token %q", token)
		}
		if got != token {
			t.Fatalf("round-trip mismatch: got %q, want %q", got, token)
		}
	})
}
