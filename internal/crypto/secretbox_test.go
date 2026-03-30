package crypto_test

import (
	"bytes"
	"testing"

	"github.com/zitadel/zitadel/internal/crypto"
)

func TestSecretBoxRoundTrip(t *testing.T) {
	keys := map[string]string{
		"key_v1": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
	}
	sb, err := crypto.NewSecretBox("key_v1", keys)
	if err != nil {
		t.Fatalf("NewSecretBox: %v", err)
	}

	original := []byte("this is a signing key payload")
	sealed, err := sb.Seal(original)
	if err != nil {
		t.Fatalf("Seal: %v", err)
	}

	if sealed.KeyID != "key_v1" {
		t.Errorf("KeyID = %q, want %q", sealed.KeyID, "key_v1")
	}
	if len(sealed.Nonce) == 0 {
		t.Error("Nonce is empty")
	}
	if bytes.Equal(sealed.Ciphertext, original) {
		t.Error("Ciphertext equals plaintext — encryption did nothing")
	}

	decrypted, err := sb.Open(sealed.Ciphertext, sealed.Nonce, sealed.KeyID)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if !bytes.Equal(decrypted, original) {
		t.Errorf("Round-trip failed: got %q, want %q", decrypted, original)
	}
}

func TestSecretBoxPlaintextMode(t *testing.T) {
	sb, err := crypto.NewSecretBox("", nil)
	if err != nil {
		t.Fatalf("NewSecretBox: %v", err)
	}

	if !sb.Plaintext() {
		t.Error("Expected Plaintext() == true")
	}

	original := []byte("plaintext secret")
	sealed, err := sb.Seal(original)
	if err != nil {
		t.Fatalf("Seal: %v", err)
	}
	if sealed.KeyID != "" {
		t.Errorf("KeyID = %q, want empty", sealed.KeyID)
	}
	if !bytes.Equal(sealed.Ciphertext, original) {
		t.Error("Plaintext mode should pass data through unchanged")
	}

	decrypted, err := sb.Open(sealed.Ciphertext, sealed.Nonce, sealed.KeyID)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if !bytes.Equal(decrypted, original) {
		t.Error("Plaintext round-trip failed")
	}
}

func TestSecretBoxWrongKey(t *testing.T) {
	keys := map[string]string{
		"key_v1": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
		"key_v2": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
	}
	sb, err := crypto.NewSecretBox("key_v1", keys)
	if err != nil {
		t.Fatalf("NewSecretBox: %v", err)
	}

	sealed, err := sb.Seal([]byte("secret"))
	if err != nil {
		t.Fatalf("Seal: %v", err)
	}

	// Try to decrypt with the wrong key
	_, err = sb.Open(sealed.Ciphertext, sealed.Nonce, "key_v2")
	if err == nil {
		t.Error("Expected error when decrypting with wrong key")
	}
}

func TestSecretBoxTamperedCiphertext(t *testing.T) {
	keys := map[string]string{
		"key_v1": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
	}
	sb, err := crypto.NewSecretBox("key_v1", keys)
	if err != nil {
		t.Fatalf("NewSecretBox: %v", err)
	}

	sealed, err := sb.Seal([]byte("secret"))
	if err != nil {
		t.Fatalf("Seal: %v", err)
	}

	// Flip a byte in the ciphertext
	sealed.Ciphertext[0] ^= 0xFF

	_, err = sb.Open(sealed.Ciphertext, sealed.Nonce, sealed.KeyID)
	if err == nil {
		t.Error("Expected error when ciphertext is tampered")
	}
}

func TestSecretBoxKeyRotation(t *testing.T) {
	keys := map[string]string{
		"key_v1": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
		"key_v2": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
	}

	// Encrypt with v1
	sbV1, err := crypto.NewSecretBox("key_v1", keys)
	if err != nil {
		t.Fatalf("NewSecretBox v1: %v", err)
	}
	sealed, err := sbV1.Seal([]byte("rotate me"))
	if err != nil {
		t.Fatalf("Seal: %v", err)
	}

	// Later, active key rotated to v2, but v1 still in ring for decryption
	sbV2, err := crypto.NewSecretBox("key_v2", keys)
	if err != nil {
		t.Fatalf("NewSecretBox v2: %v", err)
	}

	// Can still decrypt old data
	plaintext, err := sbV2.Open(sealed.Ciphertext, sealed.Nonce, sealed.KeyID)
	if err != nil {
		t.Fatalf("Open old data with new box: %v", err)
	}
	if string(plaintext) != "rotate me" {
		t.Errorf("got %q, want %q", plaintext, "rotate me")
	}

	// Re-encrypt with v2
	reSealed, err := sbV2.Seal(plaintext)
	if err != nil {
		t.Fatalf("Re-Seal: %v", err)
	}
	if reSealed.KeyID != "key_v2" {
		t.Errorf("KeyID = %q, want %q", reSealed.KeyID, "key_v2")
	}

	// Verify the re-encrypted data
	final, err := sbV2.Open(reSealed.Ciphertext, reSealed.Nonce, reSealed.KeyID)
	if err != nil {
		t.Fatalf("Open re-encrypted: %v", err)
	}
	if string(final) != "rotate me" {
		t.Errorf("got %q, want %q", final, "rotate me")
	}
}

func TestSecretBoxMissingActiveKey(t *testing.T) {
	keys := map[string]string{
		"key_v1": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
	}
	_, err := crypto.NewSecretBox("nonexistent", keys)
	if err == nil {
		t.Error("Expected error for missing active key")
	}
}

func TestSecretBoxInvalidKeyLength(t *testing.T) {
	keys := map[string]string{
		"short": "0123456789abcdef",
	}
	_, err := crypto.NewSecretBox("short", keys)
	if err == nil {
		t.Error("Expected error for short key")
	}
}

func TestSecretBoxUnknownKeyID(t *testing.T) {
	keys := map[string]string{
		"key_v1": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
	}
	sb, err := crypto.NewSecretBox("key_v1", keys)
	if err != nil {
		t.Fatalf("NewSecretBox: %v", err)
	}

	_, err = sb.Open([]byte("data"), []byte("nonce12bytes"), "unknown_key")
	if err == nil {
		t.Error("Expected error for unknown key ID")
	}
}
