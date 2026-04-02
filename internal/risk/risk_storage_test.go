package risk

import (
	"context"
	"database/sql"
	"testing"

	"github.com/zitadel/zitadel/internal/httputil"

	_ "modernc.org/sqlite"
)

func newRiskTestDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open sqlite: %v", err)
	}
	statements := []string{
		`CREATE TABLE events (
			instance_id TEXT NOT NULL,
			event_type TEXT NOT NULL,
			actor_id TEXT NOT NULL DEFAULT '',
			fingerprint TEXT NOT NULL DEFAULT '',
			created_at TEXT NOT NULL
		)`,
		`CREATE TABLE sessions (
			instance_id TEXT NOT NULL,
			user_id TEXT NOT NULL,
			revoked_at TEXT,
			created_at TEXT NOT NULL,
			ip_address TEXT NOT NULL DEFAULT '',
			user_agent TEXT NOT NULL DEFAULT ''
		)`,
		`CREATE TABLE tokens (
			instance_id TEXT NOT NULL,
			user_id TEXT NOT NULL,
			revoked_at TEXT,
			created_at TEXT NOT NULL
		)`,
	}
	for _, stmt := range statements {
		if _, err := db.Exec(stmt); err != nil {
			t.Fatalf("create schema: %v", err)
		}
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func TestLoadHistory_IsTenantScoped(t *testing.T) {
	db := newRiskTestDB(t)
	engine := &Engine{db: db}

	now := "2099-01-01T00:00:00Z"
	if _, err := db.Exec(
		`INSERT INTO events (instance_id, event_type, actor_id, fingerprint, created_at) VALUES
		 ('tenant_a', 'auth.login_failed', 'user_a', 'fp_a', ?),
		 ('tenant_a', 'auth.login_failed', 'user_a', 'fp_a', ?),
		 ('tenant_b', 'auth.login_failed', 'user_b', 'fp_b', ?)`,
		now, now, now,
	); err != nil {
		t.Fatalf("insert events: %v", err)
	}
	if _, err := db.Exec(
		`INSERT INTO sessions (instance_id, user_id, revoked_at, created_at, ip_address, user_agent) VALUES
		 ('tenant_a', 'user_a', ?, ?, '10.0.0.1', 'ua-a'),
		 ('tenant_a', 'user_a', NULL, ?, '10.0.0.1', 'ua-a'),
		 ('tenant_b', 'user_b', ?, ?, '10.0.0.2', 'ua-b')`,
		now, now, now, now, now,
	); err != nil {
		t.Fatalf("insert sessions: %v", err)
	}
	if _, err := db.Exec(
		`INSERT INTO tokens (instance_id, user_id, revoked_at, created_at) VALUES
		 ('tenant_a', 'user_a', ?, ?),
		 ('tenant_b', 'user_b', ?, ?)`,
		now, now, now, now,
	); err != nil {
		t.Fatalf("insert tokens: %v", err)
	}

	ctxA := httputil.WithInstanceID(context.Background(), "tenant_a")
	ctxB := httputil.WithInstanceID(context.Background(), "tenant_b")

	historyA, err := engine.loadHistory(ctxA, Input{
		UserID:    "user_a",
		IPAddress: "10.0.0.9",
		UserAgent: "ua-new",
		Signals: Signals{
			VisitorID: "fp_a",
		},
	})
	if err != nil {
		t.Fatalf("loadHistory tenant_a: %v", err)
	}
	if historyA.recentLoginFailures != 2 {
		t.Fatalf("tenant_a recentLoginFailures = %d, want 2", historyA.recentLoginFailures)
	}
	if historyA.recentSessionRevokes != 1 {
		t.Fatalf("tenant_a recentSessionRevokes = %d, want 1", historyA.recentSessionRevokes)
	}
	if historyA.recentTokenRevokes != 1 {
		t.Fatalf("tenant_a recentTokenRevokes = %d, want 1", historyA.recentTokenRevokes)
	}
	if !historyA.knownFingerprint {
		t.Fatal("tenant_a knownFingerprint = false, want true")
	}
	if !historyA.newIPOrUA {
		t.Fatal("tenant_a newIPOrUA = false, want true")
	}

	historyB, err := engine.loadHistory(ctxB, Input{
		UserID: "user_a",
		Signals: Signals{
			VisitorID: "fp_a",
		},
	})
	if err != nil {
		t.Fatalf("loadHistory tenant_b: %v", err)
	}
	if historyB.recentLoginFailures != 0 {
		t.Fatalf("tenant_b recentLoginFailures = %d, want 0", historyB.recentLoginFailures)
	}
	if historyB.recentSessionRevokes != 0 {
		t.Fatalf("tenant_b recentSessionRevokes = %d, want 0", historyB.recentSessionRevokes)
	}
	if historyB.recentTokenRevokes != 0 {
		t.Fatalf("tenant_b recentTokenRevokes = %d, want 0", historyB.recentTokenRevokes)
	}
	if historyB.knownFingerprint {
		t.Fatal("tenant_b knownFingerprint = true, want false")
	}
}
