package database

import (
	"database/sql"
	"fmt"

	"github.com/pressly/goose/v3"
)

func init() {
	goose.AddMigrationNoTx(up002TraceID, down002TraceID)
}

// up002TraceID adds trace_id and session_id columns to events for correlation.
// trace_id: OTel-compatible trace correlation across HTTP spans, errors, alerts.
// session_id: roots every authenticated operation to its session.
func up002TraceID(db *sql.DB) error {
	statements := []string{
		`ALTER TABLE events ADD COLUMN trace_id TEXT DEFAULT ''`,
		`ALTER TABLE events ADD COLUMN session_id INTEGER DEFAULT 0`,
	}
	for _, stmt := range statements {
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("migration 002: %w\nSQL: %s", err, stmt)
		}
	}
	return nil
}

func down002TraceID(db *sql.DB) error {
	// SQLite doesn't support DROP COLUMN before 3.35.0; safe to skip.
	return nil
}
