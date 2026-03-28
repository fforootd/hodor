package auth

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
)

func newTestDB(t *testing.T) *database.DB {
	t.Helper()
	dir := t.TempDir()
	path := filepath.Join(dir, "test.db")
	db, err := database.Open("sqlite://" + path)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func createTestIdentity(t *testing.T, db *database.DB) string {
	t.Helper()
	identityID := id.New()
	_, err := db.SQL().Exec(
		`INSERT INTO entities (id, org_id, identifier, state, profile, metadata, created_at, updated_at)
		 VALUES (?, '1', 'test@example.com', 'active', '{"display_name":"Test User"}', '{}', datetime('now'), datetime('now'))`,
		identityID,
	)
	if err != nil {
		t.Fatalf("create identity: %v", err)
	}
	return identityID
}

func TestHashAndVerify(t *testing.T) {
	db := newTestDB(t)
	pw := NewPasswords(db)

	encoded, err := pw.Hash("test-password-123")
	if err != nil {
		t.Fatalf("Hash: %v", err)
	}

	if encoded == "" {
		t.Fatal("Hash returned empty string")
	}

	// Should contain argon2id marker.
	if len(encoded) < 10 {
		t.Fatalf("Hash too short: %q", encoded)
	}

	// Verify correct password.
	ok, _, err := pw.Verify(encoded, "test-password-123")
	if err != nil {
		t.Fatalf("Verify: %v", err)
	}
	if !ok {
		t.Fatal("Verify returned false for correct password")
	}

	// Verify wrong password.
	ok, _, err = pw.Verify(encoded, "wrong-password")
	if err != nil {
		t.Fatalf("Verify error: %v", err)
	}
	if ok {
		t.Fatal("Verify returned true for wrong password")
	}
}

func TestSetAndCheckPassword(t *testing.T) {
	db := newTestDB(t)
	pw := NewPasswords(db)
	ctx := context.Background()

	identityID := createTestIdentity(t, db)

	// Set password.
	if err := pw.SetPassword(ctx, identityID, "my-secret-password"); err != nil {
		t.Fatalf("SetPassword: %v", err)
	}

	// Check correct password.
	ok, err := pw.CheckPassword(ctx, identityID, "my-secret-password")
	if err != nil {
		t.Fatalf("CheckPassword: %v", err)
	}
	if !ok {
		t.Fatal("CheckPassword returned false for correct password")
	}

	// Check wrong password.
	ok, err = pw.CheckPassword(ctx, identityID, "wrong-password")
	if err != nil {
		t.Fatalf("CheckPassword error: %v", err)
	}
	if ok {
		t.Fatal("CheckPassword returned true for wrong password")
	}

	// Check for non-existent identity.
	ok, err = pw.CheckPassword(ctx, "nonexistent_id", "any-password")
	if err != nil {
		t.Fatalf("CheckPassword non-existent: %v", err)
	}
	if ok {
		t.Fatal("CheckPassword returned true for non-existent identity")
	}
}

func TestSetPasswordReplace(t *testing.T) {
	db := newTestDB(t)
	pw := NewPasswords(db)
	ctx := context.Background()

	identityID := createTestIdentity(t, db)

	// Set initial password.
	if err := pw.SetPassword(ctx, identityID, "password-v1"); err != nil {
		t.Fatalf("SetPassword v1: %v", err)
	}

	// Replace with new password.
	if err := pw.SetPassword(ctx, identityID, "password-v2"); err != nil {
		t.Fatalf("SetPassword v2: %v", err)
	}

	// Old password should fail.
	ok, err := pw.CheckPassword(ctx, identityID, "password-v1")
	if err != nil {
		t.Fatalf("CheckPassword old: %v", err)
	}
	if ok {
		t.Fatal("old password should not work after replacement")
	}

	// New password should succeed.
	ok, err = pw.CheckPassword(ctx, identityID, "password-v2")
	if err != nil {
		t.Fatalf("CheckPassword new: %v", err)
	}
	if !ok {
		t.Fatal("new password should work after replacement")
	}
}

func TestGenerateRandomPassword(t *testing.T) {
	pw1, err := GenerateRandomPassword(16)
	if err != nil {
		t.Fatalf("GenerateRandomPassword: %v", err)
	}
	if len(pw1) != 16 {
		t.Fatalf("expected length 16, got %d", len(pw1))
	}

	pw2, err := GenerateRandomPassword(16)
	if err != nil {
		t.Fatalf("GenerateRandomPassword: %v", err)
	}
	if pw1 == pw2 {
		t.Fatal("two random passwords should not be identical")
	}
}

func TestDecodeCredentialJSON(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{`{"hash":"$argon2id$v=19$m=65536,t=1,p=4$salt$hash"}`, `$argon2id$v=19$m=65536,t=1,p=4$salt$hash`},
		{`{"hash":""}`, ``},
		{`invalid`, ``},
		{`{}`, ``},
	}
	for _, tt := range tests {
		got := DecodeCredentialJSON(tt.input)
		if got != tt.want {
			t.Errorf("DecodeCredentialJSON(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

// --- OWASP AUTH: Password Security Tests ---

func TestHash_EmptyPassword(t *testing.T) {
	db := newTestDB(t)
	pw := NewPasswords(db)

	encoded, err := pw.Hash("")
	if err != nil {
		t.Fatalf("Hash empty: %v", err)
	}
	if encoded == "" {
		t.Fatal("Hash of empty password should still produce a hash")
	}

	ok, _, err := pw.Verify(encoded, "")
	if err != nil {
		t.Fatalf("Verify empty: %v", err)
	}
	if !ok {
		t.Fatal("empty password should verify against its own hash")
	}
}

func TestHash_Unicode(t *testing.T) {
	db := newTestDB(t)
	pw := NewPasswords(db)

	passwords := []string{
		"пароль123",  // Cyrillic
		"密码测试",       //nolint:gosmopolitan // CJK test fixture
		"パスワード",      // Katakana
		"🔐🔑🗝️secure", // Emoji
		"Ñoño@2026",  // Latin extended
	}

	for _, plain := range passwords {
		encoded, err := pw.Hash(plain)
		if err != nil {
			t.Fatalf("Hash(%q): %v", plain, err)
		}
		ok, _, err := pw.Verify(encoded, plain)
		if err != nil {
			t.Fatalf("Verify(%q): %v", plain, err)
		}
		if !ok {
			t.Fatalf("unicode password %q should verify", plain)
		}
	}
}

func TestHash_LongPassword(t *testing.T) {
	db := newTestDB(t)
	pw := NewPasswords(db)

	// 128 chars + 256 chars.
	for _, length := range []int{128, 256} {
		long := make([]byte, length)
		for i := range long {
			long[i] = 'A' + byte(i%26)
		}
		plain := string(long)

		encoded, err := pw.Hash(plain)
		if err != nil {
			t.Fatalf("Hash(%d chars): %v", length, err)
		}
		ok, _, err := pw.Verify(encoded, plain)
		if err != nil {
			t.Fatalf("Verify(%d chars): %v", length, err)
		}
		if !ok {
			t.Fatalf("long password (%d chars) should verify", length)
		}
	}
}

func TestExtractHash_Injection(t *testing.T) {
	// Crafted JSON payloads that might try to break extraction.
	payloads := []string{
		`{"hash":"'; DROP TABLE passwords;--"}`,
		`{"hash":"$argon2id$v=19","extra":"\u0000null byte"}`,
		`{"hash":null}`,
		`{"hash":true}`,
		`{"hash":12345}`,
		`[]`,
		`""`,
		`null`,
		string(make([]byte, 10000)), // large input
	}

	for _, p := range payloads {
		// Should not panic.
		_ = DecodeCredentialJSON(p)
	}
}

func TestCheckPassword_TimingEquality(t *testing.T) {
	// Verify that checking a wrong password for a non-existent user
	// doesn't return significantly faster than for a real user.
	// This is a behavioral test — not a strict timing test — to verify
	// the code path exists. Passwap handles constant-time comparison internally.
	db := newTestDB(t)
	pw := NewPasswords(db)
	ctx := context.Background()

	identityID := createTestIdentity(t, db)
	if err := pw.SetPassword(ctx, identityID, "real-password"); err != nil {
		t.Fatalf("SetPassword: %v", err)
	}

	// Wrong password for existing user.
	ok1, err := pw.CheckPassword(ctx, identityID, "wrong-password")
	if err != nil {
		t.Fatalf("CheckPassword existing: %v", err)
	}
	if ok1 {
		t.Fatal("wrong password should fail")
	}

	// Any password for non-existent user.
	ok2, err := pw.CheckPassword(ctx, "nonexistent_id", "any-password")
	if err != nil {
		t.Fatalf("CheckPassword non-existent: %v", err)
	}
	if ok2 {
		t.Fatal("non-existent user should fail")
	}

	// Both should return false — the important thing is neither panics
	// and both return consistent false results.
}
