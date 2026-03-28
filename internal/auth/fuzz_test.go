package auth

import (
	"testing"
)

// FuzzExtractHash stresses the extractHash function with arbitrary JSON.
// Invariant: must never panic.
func FuzzExtractHash(f *testing.F) {
	// Seed corpus with edge cases.
	f.Add(`{"hash":"$argon2id$v=19$m=65536,t=1,p=4$salt$hash"}`)
	f.Add(`{"hash":""}`)
	f.Add(`{}`)
	f.Add(`invalid`)
	f.Add(`[]`)
	f.Add(`null`)
	f.Add(`""`)
	f.Add(`{"hash":null}`)
	f.Add(`{"hash":true}`)
	f.Add(`{"hash":12345}`)
	f.Add(`{"hash":"'; DROP TABLE passwords;--"}`)
	f.Add(string(make([]byte, 10000)))

	f.Fuzz(func(t *testing.T, input string) {
		// Must not panic.
		extractHash(input)
	})
}

// FuzzPasswordHash stresses the password hashing function.
// Invariant: must never panic; hash→verify must round-trip.
func FuzzPasswordHash(f *testing.F) {
	f.Add("simple-password")
	f.Add("")
	f.Add("unicode-пароль-🔑")
	f.Add(string(make([]byte, 128)))

	db := newTestDB(&testing.T{})
	pw := NewPasswords(db)

	f.Fuzz(func(t *testing.T, password string) {
		// Skip very large passwords that would make argon2 slow.
		if len(password) > 256 {
			t.Skip("skipping very large password")
		}

		encoded, err := pw.Hash(password)
		if err != nil {
			t.Fatalf("Hash: %v", err)
		}

		ok, _, err := pw.Verify(encoded, password)
		if err != nil {
			t.Fatalf("Verify: %v", err)
		}
		if !ok {
			t.Fatalf("password %q should verify against its own hash", password)
		}
	})
}
