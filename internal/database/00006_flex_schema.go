package database

import (
	"database/sql"
	"encoding/json"
	"fmt"

	"github.com/pressly/goose/v3"
)

func init() {
	goose.AddMigrationNoTx(up006FlexSchema, down006FlexSchema)
}

// up006FlexSchema introduces the flexible schema model:
//   - Adds `schemas` table (JSON Schema registry per org/type).
//   - Adds `entity_indexes` table (promoted fields for search).
//   - Adds `data` + `schema_id` columns to identities.
//   - Migrates existing display_name/profile/metadata into the data JSON column.
func up006FlexSchema(db *sql.DB) error {
	dialect := detectDialect(db)

	bigint := "INTEGER"
	textJSON := "TEXT DEFAULT '{}'"
	timestampDefault := "TEXT NOT NULL DEFAULT (datetime('now'))"

	if dialect == "postgres" {
		bigint = "BIGINT"
		textJSON = "JSONB DEFAULT '{}'"
		timestampDefault = "TIMESTAMPTZ NOT NULL DEFAULT NOW()"
	}

	// Phase 1: Create new tables.
	schemaStatements := []string{
		// JSON Schema registry — customers define schemas per entity type + org.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS schemas (
			id         TEXT PRIMARY KEY,
			type       TEXT NOT NULL,
			org_id     %s NOT NULL DEFAULT 1,
			schema     TEXT NOT NULL,
			version    INTEGER DEFAULT 1,
			created_at %s
		)`, bigint, timestampDefault),

		`CREATE UNIQUE INDEX IF NOT EXISTS idx_schema_type_org ON schemas(type, org_id)`,

		// Promoted indexes — extracts x-indexed fields for O(log N) lookups.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS entity_indexes (
			entity_type TEXT NOT NULL,
			entity_id   %s NOT NULL,
			field       TEXT NOT NULL,
			value       TEXT NOT NULL,
			PRIMARY KEY (entity_type, entity_id, field)
		)`, bigint),

		`CREATE INDEX IF NOT EXISTS idx_ei_lookup ON entity_indexes(entity_type, field, value)`,
	}
	for _, stmt := range schemaStatements {
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("migration 006 create tables: %w\nSQL: %s", err, stmt)
		}
	}

	// Phase 2: Add new columns to identities.
	alterStatements := []string{
		fmt.Sprintf(`ALTER TABLE identities ADD COLUMN data %s`, textJSON),
		`ALTER TABLE identities ADD COLUMN schema_id TEXT DEFAULT ''`,
	}
	for _, stmt := range alterStatements {
		if _, err := db.Exec(stmt); err != nil {
			// Ignore "duplicate column" errors (idempotent re-runs).
			if !isDuplicateColumn(err) {
				return fmt.Errorf("migration 006 alter: %w\nSQL: %s", err, stmt)
			}
		}
	}

	// Phase 3: Migrate existing identity data into the `data` JSON column.
	// Merge display_name + profile + metadata → data.
	rows, err := db.Query(`SELECT id, display_name, profile, metadata FROM identities WHERE data = '{}' OR data IS NULL`)
	if err != nil {
		return fmt.Errorf("migration 006 select: %w", err)
	}
	defer rows.Close()

	type idRow struct {
		id          int64
		displayName sql.NullString
		profile     sql.NullString
		metadata    sql.NullString
	}
	var toMigrate []idRow
	for rows.Next() {
		var r idRow
		if err := rows.Scan(&r.id, &r.displayName, &r.profile, &r.metadata); err != nil {
			return fmt.Errorf("migration 006 scan: %w", err)
		}
		toMigrate = append(toMigrate, r)
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("migration 006 rows error: %w", err)
	}
	rows.Close()

	for _, r := range toMigrate {
		merged := make(map[string]any)

		// Parse existing profile JSON into the merged map.
		if r.profile.Valid && r.profile.String != "" && r.profile.String != "{}" {
			var profileData map[string]any
			if err := json.Unmarshal([]byte(r.profile.String), &profileData); err == nil {
				for k, v := range profileData {
					merged[k] = v
				}
			}
		}

		// Parse existing metadata JSON.
		if r.metadata.Valid && r.metadata.String != "" && r.metadata.String != "{}" {
			var metaData map[string]any
			if err := json.Unmarshal([]byte(r.metadata.String), &metaData); err == nil {
				merged["metadata"] = metaData
			}
		}

		// Add display_name.
		if r.displayName.Valid && r.displayName.String != "" {
			merged["display_name"] = r.displayName.String
		}

		dataJSON, err := json.Marshal(merged)
		if err != nil {
			return fmt.Errorf("migration 006 marshal id=%d: %w", r.id, err)
		}

		if _, err := db.Exec(`UPDATE identities SET data = ? WHERE id = ?`, string(dataJSON), r.id); err != nil {
			return fmt.Errorf("migration 006 update id=%d: %w", r.id, err)
		}
	}

	return nil
}

func down006FlexSchema(db *sql.DB) error {
	for _, t := range []string{"entity_indexes", "schemas"} {
		if _, err := db.Exec("DROP TABLE IF EXISTS " + t); err != nil {
			return err
		}
	}
	return nil
}

func isDuplicateColumn(err error) bool {
	if err == nil {
		return false
	}
	s := err.Error()
	// SQLite
	if contains(s, "duplicate column") {
		return true
	}
	// Postgres
	if contains(s, "already exists") {
		return true
	}
	return false
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && (s == sub || len(s) > 0 && containsInner(s, sub))
}

func containsInner(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
