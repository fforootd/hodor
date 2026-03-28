package logging

import (
	"database/sql"
	"fmt"
	"sync"
	"time"

	_ "modernc.org/sqlite"
)

// CacheRecord is a single log/event record stored in the local cache.
type CacheRecord struct {
	ID        int64
	EventType string
	Category  string
	Stream    string
	Level     string
	Payload   string
	ActorID   string
	TraceID   string
	SpanID    string
	SessionID string
	CreatedAt string
}

// Cache is a local SQLite database used as a durable buffer for analytics writes.
// It acts as a ring buffer with a configurable maximum row count.
// The cache file (zitadel-cache.db) survives process restarts and can be
// placed on tmpfs for in-memory speed in multi-machine deployments.
type Cache struct {
	db      *sql.DB
	maxRows int
	mu      sync.Mutex
}

// OpenCache opens or creates a local SQLite cache database.
// The path should point to a file like "zitadel-cache.db".
// maxRows controls the ring buffer size (0 = unlimited).
func OpenCache(path string, maxRows int) (*Cache, error) {
	db, err := sql.Open("sqlite", path+"?_journal=WAL&_busy_timeout=5000&_sync=NORMAL")
	if err != nil {
		return nil, fmt.Errorf("open cache db: %w", err)
	}
	db.SetMaxOpenConns(1) // SQLite single-writer

	// Create the buffer table.
	_, err = db.Exec(`
		CREATE TABLE IF NOT EXISTS log_buffer (
			id         INTEGER PRIMARY KEY AUTOINCREMENT,
			event_type TEXT NOT NULL,
			category   TEXT NOT NULL DEFAULT '',
			stream     TEXT NOT NULL DEFAULT '',
			level      TEXT NOT NULL DEFAULT 'info',
			payload    TEXT NOT NULL DEFAULT '{}',
			actor_id   TEXT NOT NULL DEFAULT '',
			trace_id   TEXT NOT NULL DEFAULT '',
			span_id    TEXT NOT NULL DEFAULT '',
			session_id TEXT NOT NULL DEFAULT '',
			created_at TEXT NOT NULL DEFAULT (datetime('now'))
		)
	`)
	if err != nil {
		db.Close()
		return nil, fmt.Errorf("create log_buffer table: %w", err)
	}

	return &Cache{db: db, maxRows: maxRows}, nil
}

// Write inserts a record into the local cache.
func (c *Cache) Write(rec CacheRecord) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	_, err := c.db.Exec(
		`INSERT INTO log_buffer (event_type, category, stream, level, payload, actor_id, trace_id, span_id, session_id, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		rec.EventType, rec.Category, rec.Stream, rec.Level, rec.Payload,
		rec.ActorID, rec.TraceID, rec.SpanID, rec.SessionID,
		rec.CreatedAt,
	)
	return err
}

// ReadBatch reads up to n records from the cache, oldest first.
func (c *Cache) ReadBatch(n int) ([]CacheRecord, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	rows, err := c.db.Query(
		`SELECT id, event_type, category, stream, level, payload, actor_id, trace_id, span_id, session_id, created_at
		 FROM log_buffer ORDER BY id ASC LIMIT ?`, n)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var records []CacheRecord
	for rows.Next() {
		var r CacheRecord
		if err := rows.Scan(&r.ID, &r.EventType, &r.Category, &r.Stream, &r.Level,
			&r.Payload, &r.ActorID, &r.TraceID, &r.SpanID, &r.SessionID, &r.CreatedAt); err != nil {
			return nil, err
		}
		records = append(records, r)
	}
	return records, rows.Err()
}

// Delete removes records by ID after they've been successfully drained.
func (c *Cache) Delete(ids []int64) error {
	if len(ids) == 0 {
		return nil
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	// Build a batch delete. For simplicity, use a loop — SQLite handles this
	// efficiently since these are sequential integer PKs.
	tx, err := c.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	stmt, err := tx.Prepare(`DELETE FROM log_buffer WHERE id = ?`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	for _, id := range ids {
		if _, err := stmt.Exec(id); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// Trim enforces the ring buffer maximum by deleting the oldest rows.
func (c *Cache) Trim() error {
	if c.maxRows <= 0 {
		return nil // unlimited
	}
	c.mu.Lock()
	defer c.mu.Unlock()

	_, err := c.db.Exec(
		`DELETE FROM log_buffer WHERE id NOT IN (
			SELECT id FROM log_buffer ORDER BY id DESC LIMIT ?
		)`, c.maxRows)
	return err
}

// Count returns the number of records in the buffer.
func (c *Cache) Count() int {
	var count int
	c.db.QueryRow(`SELECT COUNT(*) FROM log_buffer`).Scan(&count)
	return count
}

// Close closes the cache database.
func (c *Cache) Close() error {
	return c.db.Close()
}

// createdAtNow returns the current time in the format used by the cache.
func createdAtNow() string {
	return time.Now().UTC().Format("2006-01-02T15:04:05Z")
}
