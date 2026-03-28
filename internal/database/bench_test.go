package database

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"fmt"
	"math/rand/v2"
	"path/filepath"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/id"
)

// benchDB creates a fresh SQLite database for benchmarking with schema applied.
func benchDB(b *testing.B) *sql.DB {
	b.Helper()
	dir := b.TempDir()
	dbPath := filepath.Join(dir, "bench.db")
	db, err := Open("sqlite://" + dbPath)
	if err != nil {
		b.Fatalf("open: %v", err)
	}
	if err := EnsureSchema(db); err != nil {
		b.Fatalf("schema: %v", err)
	}
	b.Cleanup(func() {
		db.SQL().Exec("PRAGMA wal_checkpoint(TRUNCATE)")
		db.Close()
	})
	return db.SQL()
}

// seedIdentities pre-populates n identities and returns their IDs.
func seedIdentities(b *testing.B, db *sql.DB, n int) []string {
	b.Helper()
	ids := make([]string, n)
	now := time.Now().UTC().Format("2006-01-02 15:04:05")
	tx, _ := db.Begin()
	for i := 0; i < n; i++ {
		ids[i] = id.New()
		tx.Exec(
			`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, data, created_at, updated_at)
			 VALUES (?, '0', ?, ?, 'active', '{}', '{}', '{}', ?, ?)`,
			ids[i], fmt.Sprintf("user-%d", i), fmt.Sprintf("User %d", i), now, now)
	}
	tx.Commit()
	return ids
}

// seedSessionToken inserts a session+token pair and returns the raw token string.
func seedSessionToken(b *testing.B, db *sql.DB, entityID string) string {
	b.Helper()
	raw := "zit_ses_" + id.New()
	h := sha256.Sum256([]byte(raw))
	hash := hex.EncodeToString(h[:])
	sessionID := id.New()
	tokenID := id.New()
	now := time.Now().UTC().Format("2006-01-02 15:04:05")
	expiresAt := time.Now().UTC().Add(24 * time.Hour).Format("2006-01-02 15:04:05")

	db.Exec(
		`INSERT INTO sessions (id, entity_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at)
		 VALUES (?, ?, '0', ?, 'bench', '127.0.0.1', '{}', ?, ?)`,
		sessionID, entityID, hash, now, expiresAt)
	db.Exec(
		`INSERT INTO tokens (id, type, token_hash, entity_id, session_id, scopes, expires_at, created_at)
		 VALUES (?, 'session', ?, ?, ?, '[]', ?, ?)`,
		tokenID, hash, entityID, sessionID, expiresAt, now)
	return raw
}

const seedSize = 1000

// ──────────────────────────────────────────────────────────────
// Layer 1: Single-operation benchmarks
// ──────────────────────────────────────────────────────────────

func BenchmarkDBInsertIdentity(b *testing.B) {
	db := benchDB(b)
	now := time.Now().UTC().Format("2006-01-02 15:04:05")

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		newID := id.New()
		_, err := db.Exec(
			`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, data, created_at, updated_at)
			 VALUES (?, '0', ?, ?, 'active', '{}', '{}', '{}', ?, ?)`,
			newID, fmt.Sprintf("bench-insert-%d", i), fmt.Sprintf("Bench Insert %d", i), now, now)
		if err != nil {
			b.Fatalf("insert: %v", err)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "identities/sec")
}

func BenchmarkDBGetIdentity(b *testing.B) {
	db := benchDB(b)
	ids := seedIdentities(b, db, seedSize)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		target := ids[rand.IntN(len(ids))]
		var identifier, displayName string
		err := db.QueryRow(
			`SELECT identifier, display_name FROM entities WHERE id = ?`, target,
		).Scan(&identifier, &displayName)
		if err != nil {
			b.Fatalf("get: %v", err)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "reads/sec")
}

func BenchmarkDBListIdentities(b *testing.B) {
	db := benchDB(b)
	seedIdentities(b, db, seedSize)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		offset := rand.IntN(seedSize - 50)
		rows, err := db.Query(
			`SELECT id, identifier, display_name, state, created_at FROM entities
			 ORDER BY created_at DESC LIMIT 50 OFFSET ?`, offset)
		if err != nil {
			b.Fatalf("list: %v", err)
		}
		count := 0
		for rows.Next() {
			var id, ident, name, state, created string
			rows.Scan(&id, &ident, &name, &state, &created)
			count++
		}
		rows.Close()
		if count == 0 {
			b.Fatalf("no rows returned at offset %d", offset)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "pages/sec")
}

func BenchmarkDBUpdateIdentity(b *testing.B) {
	db := benchDB(b)
	ids := seedIdentities(b, db, seedSize)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		target := ids[rand.IntN(len(ids))]
		_, err := db.Exec(
			`UPDATE entities SET display_name = ?, updated_at = datetime('now') WHERE id = ?`,
			fmt.Sprintf("Updated %d", i), target)
		if err != nil {
			b.Fatalf("update: %v", err)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "updates/sec")
}

func BenchmarkDBInsertSession(b *testing.B) {
	db := benchDB(b)
	ids := seedIdentities(b, db, 100)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		entityID := ids[rand.IntN(len(ids))]
		sessionID := id.New()
		tokenID := id.New()
		hash := fmt.Sprintf("hash-%s", id.New())
		now := time.Now().UTC().Format("2006-01-02 15:04:05")
		expiresAt := time.Now().UTC().Add(24 * time.Hour).Format("2006-01-02 15:04:05")

		tx, _ := db.BeginTx(context.Background(), nil)
		tx.Exec(
			`INSERT INTO sessions (id, entity_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at)
			 VALUES (?, ?, '0', ?, 'bench', '127.0.0.1', '{}', ?, ?)`,
			sessionID, entityID, hash, now, expiresAt)
		tx.Exec(
			`INSERT INTO tokens (id, type, token_hash, entity_id, session_id, scopes, expires_at, created_at)
			 VALUES (?, 'session', ?, ?, ?, '[]', ?, ?)`,
			tokenID, hash, entityID, sessionID, expiresAt, now)
		tx.Commit()
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "sessions/sec")
}

func BenchmarkDBResolveToken(b *testing.B) {
	db := benchDB(b)
	ids := seedIdentities(b, db, 100)

	// Pre-seed tokens to resolve.
	tokens := make([]string, 200)
	for i := range tokens {
		tokens[i] = seedSessionToken(b, db, ids[rand.IntN(len(ids))])
	}

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		raw := tokens[rand.IntN(len(tokens))]
		h := sha256.Sum256([]byte(raw))
		hash := hex.EncodeToString(h[:])

		var entityID, tokenType string
		var expiresAt sql.NullString
		err := db.QueryRow(
			`SELECT t.entity_id, t.type, t.expires_at
			 FROM tokens t WHERE t.token_hash = ? AND t.revoked_at IS NULL`,
			hash,
		).Scan(&entityID, &tokenType, &expiresAt)
		if err != nil {
			b.Fatalf("resolve: %v", err)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "tokens/sec")
}

// ──────────────────────────────────────────────────────────────
// Layer 1: Concurrency benchmarks (vCPU scaling focus)
// ──────────────────────────────────────────────────────────────

func BenchmarkDBConcurrentReads(b *testing.B) {
	db := benchDB(b)
	ids := seedIdentities(b, db, seedSize)

	b.ResetTimer()
	b.ReportAllocs()

	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			target := ids[rand.IntN(len(ids))]
			var identifier string
			db.QueryRow(`SELECT identifier FROM entities WHERE id = ?`, target).Scan(&identifier)
		}
	})
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "reads/sec")
}

func BenchmarkDBConcurrentMixed(b *testing.B) {
	db := benchDB(b)
	ids := seedIdentities(b, db, seedSize)

	b.ResetTimer()
	b.ReportAllocs()

	b.RunParallel(func(pb *testing.PB) {
		i := 0
		for pb.Next() {
			if i%5 == 0 {
				// 20% writes
				newID := id.New()
				db.Exec(
					`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, data, created_at, updated_at)
					 VALUES (?, '0', ?, 'Bench Mixed', 'active', '{}', '{}', '{}', datetime('now'), datetime('now'))`,
					newID, fmt.Sprintf("mixed-%s", newID))
			} else {
				// 80% reads
				target := ids[rand.IntN(len(ids))]
				var identifier string
				db.QueryRow(`SELECT identifier FROM entities WHERE id = ?`, target).Scan(&identifier)
			}
			i++
		}
	})
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "ops/sec")
}
