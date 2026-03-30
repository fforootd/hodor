// Package crypto provides shared cryptographic utilities for Zitadel.
// It centralises token hashing, random generation, encoding, and
// application-level envelope encryption (AES-256-GCM) so that
// security-critical operations are implemented once and reused everywhere.
package crypto

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"fmt"
)

// HashTokenHex returns the SHA-256 hex digest of a raw token string.
// Used for storing token hashes in the database.
func HashTokenHex(raw string) string {
	h := sha256.Sum256([]byte(raw))
	return hex.EncodeToString(h[:])
}

// HashTokenBase64URL returns the SHA-256 base64url (no padding) digest.
// Used for PKCE S256 code challenges.
func HashTokenBase64URL(raw string) string {
	h := sha256.Sum256([]byte(raw))
	return base64.RawURLEncoding.EncodeToString(h[:])
}

// RandomHex generates nBytes of cryptographically secure random bytes
// and returns them as a hex-encoded string (2×nBytes characters).
func RandomHex(nBytes int) (string, error) {
	b := make([]byte, nBytes)
	if _, err := rand.Read(b); err != nil {
		return "", fmt.Errorf("crypto.RandomHex: %w", err)
	}
	return hex.EncodeToString(b), nil
}

// RandomBase64URL generates nBytes of cryptographically secure random bytes
// and returns them as a base64url-encoded string (no padding).
func RandomBase64URL(nBytes int) (string, error) {
	b := make([]byte, nBytes)
	if _, err := rand.Read(b); err != nil {
		return "", fmt.Errorf("crypto.RandomBase64URL: %w", err)
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}

// MustRandomHex is like RandomHex but panics on failure.
// Use in init paths or where a failure is truly unrecoverable.
func MustRandomHex(nBytes int) string {
	s, err := RandomHex(nBytes)
	if err != nil {
		panic(err)
	}
	return s
}
