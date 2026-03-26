package database

import (
	"database/sql"
	"fmt"

	"github.com/pressly/goose/v3"
)

func init() {
	goose.AddMigrationNoTx(up005Entities, down005Entities)
}

// up005Entities creates the multi-tenancy entity tables:
// instances (top-level isolation), orgs (grouping within instance),
// groups (RBAC grouping). Also adds instance_id to the existing domains table.
func up005Entities(db *sql.DB) error {
	dialect := detectDialect(db)

	bigint := "INTEGER"
	textJSON := "TEXT DEFAULT '{}'"
	timestampDefault := "TEXT NOT NULL DEFAULT (datetime('now'))"

	if dialect == "postgres" {
		bigint = "BIGINT"
		textJSON = "JSONB DEFAULT '{}'"
		timestampDefault = "TIMESTAMPTZ NOT NULL DEFAULT NOW()"
	}

	statements := []string{
		// Instances — virtual identity systems (top-level isolation).
		// Single-instance mode: one auto-created instance on first boot.
		// Multi-instance mode: opt-in via config for cloud/enterprise.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS instances (
			id         %s PRIMARY KEY,
			name       TEXT NOT NULL,
			state      TEXT NOT NULL DEFAULT 'active',
			settings   %s,
			created_at %s,
			updated_at %s
		)`, bigint, textJSON, timestampDefault, timestampDefault),

		// Orgs — groupings within an instance.
		// Each org can have its own identities, apps, policies.
		// Default org is auto-created with the instance.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS orgs (
			id          %s PRIMARY KEY,
			instance_id %s NOT NULL REFERENCES instances(id),
			name        TEXT NOT NULL,
			state       TEXT NOT NULL DEFAULT 'active',
			metadata    %s,
			created_at  %s,
			updated_at  %s
		)`, bigint, bigint, textJSON, timestampDefault, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_orgs_instance ON orgs(instance_id)`,

		// Groups — RBAC grouping within an org.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS groups (
			id         %s PRIMARY KEY,
			org_id     %s NOT NULL REFERENCES orgs(id),
			name       TEXT NOT NULL,
			metadata   %s,
			created_at %s,
			updated_at %s
		)`, bigint, bigint, textJSON, timestampDefault, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_groups_org ON groups(org_id)`,

		// Update domains table: add instance_id column.
		// Domains can now map to an instance (always) and optionally to an org
		// (for per-org login pages). The existing org_id column already exists.
		fmt.Sprintf(`ALTER TABLE domains ADD COLUMN instance_id %s DEFAULT 0`, bigint),

		`CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id)`,

		// Seed default instance and default org.
		`INSERT OR IGNORE INTO instances (id, name, created_at, updated_at)
			VALUES (1, 'default', datetime('now'), datetime('now'))`,

		`INSERT OR IGNORE INTO orgs (id, instance_id, name, created_at, updated_at)
			VALUES (1, 1, 'default', datetime('now'), datetime('now'))`,
	}

	for _, stmt := range statements {
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("migration 005: %w\nSQL: %s", err, stmt)
		}
	}
	return nil
}

func down005Entities(db *sql.DB) error {
	for _, t := range []string{"groups", "orgs", "instances"} {
		if _, err := db.Exec("DROP TABLE IF EXISTS " + t); err != nil {
			return err
		}
	}
	return nil
}
