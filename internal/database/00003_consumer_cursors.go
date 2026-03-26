package database

import (
	"database/sql"
	"fmt"

	"github.com/pressly/goose/v3"
)

func init() {
	goose.AddMigrationNoTx(up003ConsumerCursors, down003ConsumerCursors)
}

// up003ConsumerCursors creates the consumer_cursors table used by async workers
// (Lake Writer, Notification Workers, Threat Workers) to track their processing
// position in the events table. Each consumer maintains its own cursor.
func up003ConsumerCursors(db *sql.DB) error {
	statements := []string{
		`CREATE TABLE IF NOT EXISTS consumer_cursors (
			consumer_name TEXT PRIMARY KEY,
			last_event_id INTEGER NOT NULL DEFAULT 0,
			updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
		)`,
	}
	for _, stmt := range statements {
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("migration 003: %w\nSQL: %s", err, stmt)
		}
	}
	return nil
}

func down003ConsumerCursors(db *sql.DB) error {
	_, err := db.Exec(`DROP TABLE IF EXISTS consumer_cursors`)
	return err
}
