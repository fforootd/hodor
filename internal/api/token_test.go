package api

import (
	"context"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
)

// --- Pure function tests ---

func TestGeneratePrefixedToken_Format(t *testing.T) {
	for _, prefix := range []string{PrefixSession, PrefixPAT, PrefixOpaque} {
		raw, hash, err := generatePrefixedToken(prefix)
		if err != nil {
			t.Fatalf("prefix %q: unexpected error: %v", prefix, err)
		}
		if !strings.HasPrefix(raw, prefix) {
			t.Errorf("prefix %q: raw token %q does not start with prefix", prefix, raw)
		}
		// prefix + 64 hex chars = total length.
		expectedLen := len(prefix) + 64
		if len(raw) != expectedLen {
			t.Errorf("prefix %q: expected len %d, got %d", prefix, expectedLen, len(raw))
		}
		if len(hash) != 64 {
			t.Errorf("prefix %q: expected hash len 64, got %d", prefix, len(hash))
		}
	}
}

func TestGeneratePrefixedToken_Uniqueness(t *testing.T) {
	seen := make(map[string]bool, 1000)
	for i := 0; i < 1000; i++ {
		raw, _, err := generatePrefixedToken(PrefixSession)
		if err != nil {
			t.Fatalf("iteration %d: %v", i, err)
		}
		if seen[raw] {
			t.Fatalf("duplicate token at iteration %d: %s", i, raw)
		}
		seen[raw] = true
	}
}

func TestHashToken_Deterministic(t *testing.T) {
	input := "zit_ses_abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
	h1 := hashToken(input)
	h2 := hashToken(input)
	if h1 != h2 {
		t.Errorf("expected deterministic hash, got %q and %q", h1, h2)
	}
}

func TestHashToken_Avalanche(t *testing.T) {
	h1 := hashToken("token_a")
	h2 := hashToken("token_b")
	if h1 == h2 {
		t.Error("different inputs produced same hash")
	}
	// Check at least half the characters differ (avalanche property).
	diff := 0
	for i := 0; i < len(h1) && i < len(h2); i++ {
		if h1[i] != h2[i] {
			diff++
		}
	}
	if diff < 16 {
		t.Errorf("expected significant hash difference, only %d/64 chars differ", diff)
	}
}

// --- Resolution tests (require DB) ---

func newTokenTestDB(t *testing.T) *database.DB {
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
	t.Cleanup(func() {
		db.SQL().Exec("PRAGMA wal_checkpoint(TRUNCATE)")
		db.Close()
	})
	return db
}

func seedIdentity(t *testing.T, db *database.DB) string {
	t.Helper()
	now := sqliteDatetime(time.Now())
	entityID := id.New()
	_, err := db.SQL().Exec(
		`INSERT INTO entities (id, org_id, identifier, state, profile, metadata, created_at, updated_at)
		 VALUES (?, '1', 'test@test.com', 'active', '{}', '{}', ?, ?)`,
		entityID, now, now)
	if err != nil {
		t.Fatalf("seed identity: %v", err)
	}
	return entityID
}

// sqliteDatetime formats a time in SQLite's datetime() compatible format.
// SECURITY NOTE: SQLite's datetime('now') returns 'YYYY-MM-DD HH:MM:SS' format,
// so all timestamps stored for comparison must use the same format.
func sqliteDatetime(t time.Time) string {
	return t.UTC().Format("2006-01-02 15:04:05")
}

func seedSessionToken(t *testing.T, db *database.DB, entityID string, expiresAt, revokedAt string) string {
	t.Helper()
	raw, hash, err := generatePrefixedToken(PrefixSession)
	if err != nil {
		t.Fatalf("generate token: %v", err)
	}
	now := sqliteDatetime(time.Now())
	sessionID := id.New()
	tokenID := id.New()

	var revokedSession, revokedToken *string
	if revokedAt != "" {
		revokedSession = &revokedAt
		revokedToken = &revokedAt
	}

	_, err = db.SQL().Exec(
		`INSERT INTO sessions (id, entity_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at, revoked_at)
		 VALUES (?, ?, '1', ?, 'test', '127.0.0.1', '{}', ?, ?, ?)`,
		sessionID, entityID, hash, now, expiresAt, revokedSession)
	if err != nil {
		t.Fatalf("seed session: %v", err)
	}

	_, err = db.SQL().Exec(
		`INSERT INTO tokens (id, type, token_hash, entity_id, session_id, scopes, expires_at, created_at, revoked_at)
		 VALUES (?, 'session', ?, ?, ?, '[]', ?, ?, ?)`,
		tokenID, hash, entityID, sessionID, expiresAt, now, revokedToken)
	if err != nil {
		t.Fatalf("seed token: %v", err)
	}

	return raw
}

func seedPATToken(t *testing.T, db *database.DB, entityID string, expiresAt, revokedAt string) string {
	t.Helper()
	raw, hash, err := generatePrefixedToken(PrefixPAT)
	if err != nil {
		t.Fatalf("generate PAT: %v", err)
	}
	now := sqliteDatetime(time.Now())
	tokenID := id.New()

	var revoked *string
	if revokedAt != "" {
		revoked = &revokedAt
	}

	var expires *string
	if expiresAt != "" {
		expires = &expiresAt
	}

	_, err = db.SQL().Exec(
		`INSERT INTO tokens (id, type, token_hash, entity_id, name, scopes, expires_at, created_at, revoked_at)
		 VALUES (?, 'pat', ?, ?, 'test-pat', '["admin"]', ?, ?, ?)`,
		tokenID, hash, entityID, expires, now, revoked)
	if err != nil {
		t.Fatalf("seed PAT: %v", err)
	}

	return raw
}

func TestResolveToken_Dispatches(t *testing.T) {
	db := newTokenTestDB(t)
	entityID := seedIdentity(t, db)
	ctx := context.Background()

	future := sqliteDatetime(time.Now().Add(24 * time.Hour))

	// Session token.
	sesToken := seedSessionToken(t, db, entityID, future, "")
	info, err := resolveToken(ctx, db.SQL(), sesToken)
	if err != nil {
		t.Fatalf("resolve session token: %v", err)
	}
	if info.TokenType != TokenTypeSession {
		t.Errorf("expected type %q, got %q", TokenTypeSession, info.TokenType)
	}
	if info.EntityID != entityID {
		t.Errorf("expected entityID %s, got %s", entityID, info.EntityID)
	}

	// PAT token.
	patToken := seedPATToken(t, db, entityID, "", "")
	info, err = resolveToken(ctx, db.SQL(), patToken)
	if err != nil {
		t.Fatalf("resolve PAT token: %v", err)
	}
	if info.TokenType != TokenTypePAT {
		t.Errorf("expected type %q, got %q", TokenTypePAT, info.TokenType)
	}
}

func TestResolveToken_UnknownPrefix(t *testing.T) {
	db := newTokenTestDB(t)
	ctx := context.Background()

	// Unknown prefix falls back to legacy resolver; should fail gracefully.
	_, err := resolveToken(ctx, db.SQL(), "unknown_prefix_token_12345")
	if err == nil {
		t.Error("expected error for unknown token, got nil")
	}
}

func TestResolveSessionToken_Expired(t *testing.T) {
	db := newTokenTestDB(t)
	entityID := seedIdentity(t, db)
	ctx := context.Background()

	past := sqliteDatetime(time.Now().Add(-1 * time.Hour))
	token := seedSessionToken(t, db, entityID, past, "")

	_, err := resolveToken(ctx, db.SQL(), token)
	if err == nil {
		t.Error("expected error for expired session token, got nil")
	}
}

func TestResolveSessionToken_Revoked(t *testing.T) {
	db := newTokenTestDB(t)
	entityID := seedIdentity(t, db)
	ctx := context.Background()

	future := sqliteDatetime(time.Now().Add(24 * time.Hour))
	now := sqliteDatetime(time.Now())
	token := seedSessionToken(t, db, entityID, future, now)

	_, err := resolveToken(ctx, db.SQL(), token)
	if err == nil {
		t.Error("expected error for revoked session token, got nil")
	}
}

func TestResolvePATToken_Valid(t *testing.T) {
	db := newTokenTestDB(t)
	entityID := seedIdentity(t, db)
	ctx := context.Background()

	token := seedPATToken(t, db, entityID, "", "")
	info, err := resolveToken(ctx, db.SQL(), token)
	if err != nil {
		t.Fatalf("resolve valid PAT: %v", err)
	}
	if info.EntityID != entityID {
		t.Errorf("expected entityID %s, got %s", entityID, info.EntityID)
	}
	if info.TokenType != TokenTypePAT {
		t.Errorf("expected type %q, got %q", TokenTypePAT, info.TokenType)
	}
}

func TestResolvePATToken_Expired(t *testing.T) {
	db := newTokenTestDB(t)
	entityID := seedIdentity(t, db)
	ctx := context.Background()

	past := sqliteDatetime(time.Now().Add(-1 * time.Hour))
	token := seedPATToken(t, db, entityID, past, "")

	_, err := resolveToken(ctx, db.SQL(), token)
	if err == nil {
		t.Error("expected error for expired PAT, got nil")
	}
}

func TestResolvePATToken_Revoked(t *testing.T) {
	db := newTokenTestDB(t)
	entityID := seedIdentity(t, db)
	ctx := context.Background()

	now := sqliteDatetime(time.Now())
	token := seedPATToken(t, db, entityID, "", now)

	_, err := resolveToken(ctx, db.SQL(), token)
	if err == nil {
		t.Error("expected error for revoked PAT, got nil")
	}
}
