package logging

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

// openTestDestDB creates a temporary SQLite database simulating the analytics backend.
func openTestDestDB(t *testing.T) *sql.DB {
	t.Helper()
	path := filepath.Join(t.TempDir(), "dest.db")
	db, err := sql.Open("sqlite", path+"?_journal=WAL&_busy_timeout=5000")
	if err != nil {
		t.Fatal(err)
	}
	db.SetMaxOpenConns(1)

	// Create a minimal events table for drainer to INSERT into.
	_, err = db.Exec(`
		CREATE TABLE events (
			id TEXT PRIMARY KEY,
			event_type TEXT NOT NULL,
			category TEXT NOT NULL DEFAULT '',
			org_id TEXT NOT NULL DEFAULT '0',
			actor_id TEXT NOT NULL DEFAULT '',
			actor_type TEXT NOT NULL DEFAULT '',
			aggregate_id TEXT NOT NULL DEFAULT '',
			aggregate_type TEXT NOT NULL DEFAULT '',
			payload TEXT NOT NULL DEFAULT '{}',
			metadata TEXT NOT NULL DEFAULT '{}',
			trace_id TEXT NOT NULL DEFAULT '',
			span_id TEXT NOT NULL DEFAULT '',
			parent_span_id TEXT NOT NULL DEFAULT '',
			session_id TEXT NOT NULL DEFAULT '',
			created_at TEXT NOT NULL
		)
	`)
	if err != nil {
		t.Fatal(err)
	}
	return db
}

func TestDrainer_FlushBatch(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	dest := openTestDestDB(t)
	defer dest.Close()

	// Write 10 records to cache.
	for i := 0; i < 10; i++ {
		if err := cache.Write(testRecord("log.info")); err != nil {
			t.Fatal(err)
		}
	}

	// Create drainer and flush once.
	drainer := NewDrainer(cache, dest, time.Hour, 100)
	drainer.flush()

	// Cache should be empty (all drained).
	if cache.Count() != 0 {
		t.Errorf("expected cache empty after drain, got %d", cache.Count())
	}

	// Dest should have 10 rows.
	var count int
	dest.QueryRow(`SELECT COUNT(*) FROM events`).Scan(&count)
	if count != 10 {
		t.Errorf("expected 10 events in dest, got %d", count)
	}

	// Verify event_type and category were preserved.
	var eventType, category string
	dest.QueryRow(`SELECT event_type, category FROM events LIMIT 1`).Scan(&eventType, &category)
	if eventType != "log.info" {
		t.Errorf("expected event_type 'log.info', got %q", eventType)
	}
	if category != "log" {
		t.Errorf("expected category 'log', got %q", category)
	}
}

func TestDrainer_CircuitBreaker(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	// Use a broken destination (closed DB).
	dest := openTestDestDB(t)
	dest.Close() // close it so all writes fail

	// Write records.
	for i := 0; i < 5; i++ {
		if err := cache.Write(testRecord("log.error")); err != nil {
			t.Fatal(err)
		}
	}

	// Create drainer with low failure threshold.
	drainer := NewDrainer(cache, dest, time.Hour, 100)
	drainer.cb = NewCircuitBreaker(2, time.Minute)

	// Flush multiple times — should fail and trip the breaker.
	drainer.flush()
	drainer.flush()

	// CB should be open now.
	if drainer.cb.State() != "open" {
		t.Errorf("expected CB open, got %s", drainer.cb.State())
	}

	// Cache should still have all records (nothing drained).
	if cache.Count() != 5 {
		t.Errorf("expected 5 records still in cache, got %d", cache.Count())
	}

	// Flush should be skipped due to open CB.
	drainer.flush()
	if cache.Count() != 5 {
		t.Errorf("expected 5 records after skipped flush, got %d", cache.Count())
	}
}

func TestDrainer_ShutdownDrain(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	dest := openTestDestDB(t)
	defer dest.Close()

	for i := 0; i < 3; i++ {
		if err := cache.Write(testRecord("log.warn")); err != nil {
			t.Fatal(err)
		}
	}

	ctx, cancel := context.WithCancel(context.Background())
	drainer := NewDrainer(cache, dest, time.Hour, 100) // long interval — won't tick

	// Run in background, immediately cancel.
	done := make(chan struct{})
	go func() {
		drainer.Run(ctx)
		close(done)
	}()

	// Cancel triggers shutdown drain.
	cancel()

	select {
	case <-done:
		// good
	case <-time.After(2 * time.Second):
		t.Fatal("drainer did not shut down in time")
	}

	// Verify final drain happened.
	if cache.Count() != 0 {
		t.Errorf("expected cache empty after shutdown drain, got %d", cache.Count())
	}
}

func TestDrainer_EmptyCache(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	dest := openTestDestDB(t)
	defer dest.Close()

	drainer := NewDrainer(cache, dest, time.Hour, 100)

	// Flush empty cache — should not panic or error.
	drainer.flush()

	var count int
	dest.QueryRow(`SELECT COUNT(*) FROM events`).Scan(&count)
	if count != 0 {
		t.Errorf("expected 0 events in dest, got %d", count)
	}
}
