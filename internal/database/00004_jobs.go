package database

import (
	"database/sql"
	"fmt"

	"github.com/pressly/goose/v3"
)

func init() {
	goose.AddMigrationNoTx(up004Jobs, down004Jobs)
}

// up004Jobs creates the jobs and retention_policies tables.
// Jobs are first-class artifacts: each background worker is a row with cron schedule.
// Retention policies define per-event-type TTLs for OLTP and lake storage.
func up004Jobs(db *sql.DB) error {
	statements := []string{
		// Jobs table
		`CREATE TABLE IF NOT EXISTS jobs (
			name         TEXT PRIMARY KEY,
			display_name TEXT NOT NULL,
			description  TEXT DEFAULT '',
			cron         TEXT NOT NULL,
			enabled      INTEGER DEFAULT 1,
			last_run_at  TEXT,
			next_run_at  TEXT,
			last_status  TEXT DEFAULT 'idle',
			last_error   TEXT DEFAULT '',
			run_count    INTEGER DEFAULT 0,
			config_json  TEXT DEFAULT '{}',
			created_at   TEXT NOT NULL DEFAULT (datetime('now'))
		)`,

		// Retention policies table
		`CREATE TABLE IF NOT EXISTS retention_policies (
			id            INTEGER PRIMARY KEY AUTOINCREMENT,
			event_pattern TEXT NOT NULL,
			oltp_ttl      TEXT NOT NULL,
			lake_ttl      TEXT NOT NULL,
			priority      INTEGER DEFAULT 0,
			created_at    TEXT NOT NULL DEFAULT (datetime('now'))
		)`,

		// Seed default jobs
		`INSERT OR IGNORE INTO jobs (name, display_name, description, cron) VALUES
			('lake_writer', 'Lake Writer', 'Drains events from OLTP buffer to Parquet files', '*/1 * * * *'),
			('session_gc',  'Session GC',  'Cleans revoked and expired sessions',             '*/15 * * * *'),
			('event_gc',    'Event GC',    'Deletes OLTP events past retention (shipped to lake)', '0 * * * *')`,

		// Seed default retention policies (higher priority = matched first)
		`INSERT OR IGNORE INTO retention_policies (event_pattern, oltp_ttl, lake_ttl, priority) VALUES
			('auth.login_failure', '30d', '365d', 100),
			('auth.*',             '14d', '365d', 90),
			('session.*',          '7d',  '90d',  80),
			('identity.*',         '30d', '0',    70),
			('event.*',            '3d',  '30d',  60),
			('*',                  '14d', '365d', 0)`,
	}
	for _, stmt := range statements {
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("migration 004: %w\nSQL: %s", err, stmt)
		}
	}
	return nil
}

func down004Jobs(db *sql.DB) error {
	for _, t := range []string{"retention_policies", "jobs"} {
		if _, err := db.Exec("DROP TABLE IF EXISTS " + t); err != nil {
			return err
		}
	}
	return nil
}
