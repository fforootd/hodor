package database

import (
	_ "embed"
	"fmt"
	"log"
	"strings"
)

//go:embed schema.sql
var schemaDDL string

// EnsureSchema executes the consolidated schema DDL against the database.
// All statements use IF NOT EXISTS / OR IGNORE for idempotent re-runs.
// This replaces the previous goose-based migration system for the POC.
func EnsureSchema(db *DB) error {
	// Split on semicolons and execute each statement.
	stmts := strings.Split(schemaDDL, ";")
	for _, stmt := range stmts {
		stmt = strings.TrimSpace(stmt)
		if stmt == "" || stmt == "\n" {
			continue
		}
		if _, err := db.sql.Exec(stmt); err != nil {
			return fmt.Errorf("schema exec: %w\nSQL: %s", err, truncate(stmt, 200))
		}
	}

	log.Printf("schema ready (dialect=%s)", db.dialect)
	return nil
}

func truncate(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max] + "..."
}
