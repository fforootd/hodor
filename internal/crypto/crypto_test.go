package crypto

import (
	"encoding/hex"
	"testing"
)

func TestHashTokenHex(t *testing.T) {
	// Known SHA-256 of "hello" in hex.
	got := HashTokenHex("hello")
	want := "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
	if got != want {
		t.Errorf("HashTokenHex(\"hello\") = %s, want %s", got, want)
	}
}

func TestHashTokenBase64URL(t *testing.T) {
	got := HashTokenBase64URL("hello")
	// SHA-256 of "hello" in base64url (no padding).
	want := "LPJNul-wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ"
	if got != want {
		t.Errorf("HashTokenBase64URL(\"hello\") = %s, want %s", got, want)
	}
}

func TestRandomHex(t *testing.T) {
	a, err := RandomHex(32)
	if err != nil {
		t.Fatalf("RandomHex(32) error: %v", err)
	}
	if len(a) != 64 {
		t.Errorf("RandomHex(32) length = %d, want 64", len(a))
	}
	// Verify it's valid hex.
	if _, err := hex.DecodeString(a); err != nil {
		t.Errorf("RandomHex(32) not valid hex: %v", err)
	}

	b, _ := RandomHex(32)
	if a == b {
		t.Error("two RandomHex calls should not produce identical output")
	}
}

func TestRandomBase64URL(t *testing.T) {
	s, err := RandomBase64URL(32)
	if err != nil {
		t.Fatalf("RandomBase64URL(32) error: %v", err)
	}
	if len(s) == 0 {
		t.Error("RandomBase64URL(32) returned empty string")
	}
}

func TestMustRandomHex(t *testing.T) {
	// Should not panic with valid input.
	s := MustRandomHex(16)
	if len(s) != 32 {
		t.Errorf("MustRandomHex(16) length = %d, want 32", len(s))
	}
}
