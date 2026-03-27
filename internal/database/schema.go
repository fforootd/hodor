package database

import (
	_ "embed"
	"fmt"
	"log"
	"strings"
)

//go:embed schema.sql
var schemaDDL string

// EnsureSchema executes all DDL statements from schema.sql to initialise or
// update the database schema. Every statement uses IF NOT EXISTS so the
// function is safe to call on every startup.
//
// NOTE: schema.sql is currently SQLite-only (uses datetime(), TEXT types).
// Postgres support will require a separate DDL file.
func EnsureSchema(db *DB) error {
	if db.dialect == "postgres" {
		return fmt.Errorf("schema.sql is SQLite-only; Postgres DDL not yet implemented")
	}

	stmts := strings.Split(schemaDDL, ";")
	for _, stmt := range stmts {
		stmt = strings.TrimSpace(stmt)
		if stmt == "" || stmt == "\n" {
			continue
		}
		if _, err := db.sql.Exec(stmt); err != nil {
			return fmt.Errorf("schema exec: %w", err)
		}
	}
	log.Printf("schema ready (dialect=%s)", db.dialect)
	return nil
}
