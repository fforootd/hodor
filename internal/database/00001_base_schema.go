package database

import (
	"database/sql"
	"fmt"

	"github.com/pressly/goose/v3"
)

func init() {
	goose.AddMigrationNoTx(up001Base, down001Base)
}

// up001Base creates the base schema for ZITADEL v2.
// Tables: identities, identity_capabilities, identity_credentials,
//
//	sessions, events, domains, notification_templates, magic_tokens.
func up001Base(db *sql.DB) error {
	dialect := detectDialect(db)

	// Dialect-specific type helpers.
	textJSON := "TEXT DEFAULT '{}'"
	timestamp := "TEXT NOT NULL"
	timestampDefault := "TEXT NOT NULL DEFAULT (datetime('now'))"
	bigint := "INTEGER"

	if dialect == "postgres" {
		textJSON = "JSONB DEFAULT '{}'"
		timestamp = "TIMESTAMPTZ NOT NULL"
		timestampDefault = "TIMESTAMPTZ NOT NULL DEFAULT NOW()"
		bigint = "BIGINT"
	}

	statements := []string{
		// Identities — the universal identity table (ADR-001).
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS identities (
			id           %s PRIMARY KEY,
			org_id       %s NOT NULL DEFAULT 0,
			identifier   TEXT NOT NULL,
			display_name TEXT,
			state        TEXT NOT NULL DEFAULT 'active',
			profile      %s,
			metadata     %s,
			created_at   %s,
			updated_at   %s
		)`, bigint, bigint, textJSON, textJSON, timestampDefault, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_identities_org ON identities(org_id)`,
		`CREATE UNIQUE INDEX IF NOT EXISTS idx_identities_identifier ON identities(org_id, identifier)`,

		// Identity capabilities — junction table for hot-path indexed checks.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS identity_capabilities (
			identity_id %s NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
			capability  TEXT NOT NULL,
			PRIMARY KEY (identity_id, capability)
		)`, bigint),

		`CREATE INDEX IF NOT EXISTS idx_caps_capability ON identity_capabilities(capability)`,

		// Identity credentials — type-specific credential data.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS identity_credentials (
			id              %s PRIMARY KEY,
			identity_id     %s NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
			credential_type TEXT NOT NULL,
			credential_data %s,
			created_at      %s
		)`, bigint, bigint, textJSON, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_creds_identity ON identity_credentials(identity_id)`,

		// Sessions.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS sessions (
			id          %s PRIMARY KEY,
			identity_id %s NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
			org_id      %s NOT NULL DEFAULT 0,
			token_hash  TEXT NOT NULL,
			user_agent  TEXT,
			ip_address  TEXT,
			metadata    %s,
			created_at  %s,
			expires_at  %s,
			revoked_at  %s
		)`, bigint, bigint, bigint, textJSON, timestampDefault, timestamp,
			func() string {
				if dialect == "postgres" {
					return "TIMESTAMPTZ"
				}
				return "TEXT"
			}()),

		`CREATE INDEX IF NOT EXISTS idx_sessions_identity ON sessions(identity_id)`,
		`CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash)`,

		// Events — append-only event log (the queue IS the table).
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS events (
			id             %s PRIMARY KEY,
			event_type     TEXT NOT NULL,
			org_id         %s NOT NULL DEFAULT 0,
			actor_id       %s,
			actor_type     TEXT,
			aggregate_id   %s,
			aggregate_type TEXT,
			payload        %s,
			metadata       %s,
			created_at     %s
		)`, bigint, bigint, bigint, bigint, textJSON, textJSON, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type)`,
		`CREATE INDEX IF NOT EXISTS idx_events_org ON events(org_id)`,
		`CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_type, aggregate_id)`,
		`CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at)`,

		// Domains — external domain → org mapping.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS domains (
			domain     TEXT PRIMARY KEY,
			org_id     %s NOT NULL,
			verified   INTEGER NOT NULL DEFAULT 0,
			is_primary INTEGER NOT NULL DEFAULT 0,
			created_at %s
		)`, bigint, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_domains_org ON domains(org_id)`,

		// Notification templates — per-org, per-language.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS notification_templates (
			id       %s PRIMARY KEY,
			org_id   %s,
			channel  TEXT NOT NULL,
			event    TEXT NOT NULL,
			language TEXT NOT NULL DEFAULT 'en',
			subject  TEXT,
			body     TEXT NOT NULL
		)`, bigint, bigint),

		`CREATE UNIQUE INDEX IF NOT EXISTS idx_notif_tpl_unique ON notification_templates(org_id, channel, event, language)`,

		// Magic tokens — single-use tokens for magic link auth.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS magic_tokens (
			token       TEXT PRIMARY KEY,
			identity_id %s NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
			expires_at  %s,
			used_at     %s,
			session_id  %s
		)`, bigint, timestamp,
			func() string {
				if dialect == "postgres" {
					return "TIMESTAMPTZ"
				}
				return "TEXT"
			}(),
			func() string {
				if dialect == "postgres" {
					return "BIGINT"
				}
				return "INTEGER"
			}()),

		// Event consumer cursors — tracks each async consumer's position.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS consumer_cursors (
			consumer_name TEXT PRIMARY KEY,
			last_event_id %s NOT NULL DEFAULT 0,
			updated_at    %s
		)`, bigint, timestampDefault),
	}

	for _, stmt := range statements {
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("migration 001 (%s): %w\nSQL: %s", dialect, err, stmt)
		}
	}

	return nil
}

// down001Base drops all base tables.
func down001Base(db *sql.DB) error {
	tables := []string{
		"consumer_cursors",
		"magic_tokens",
		"notification_templates",
		"domains",
		"events",
		"sessions",
		"identity_credentials",
		"identity_capabilities",
		"identities",
	}
	for _, t := range tables {
		if _, err := db.Exec(fmt.Sprintf("DROP TABLE IF EXISTS %s CASCADE", t)); err != nil {
			return fmt.Errorf("drop %s: %w", t, err)
		}
	}
	return nil
}
