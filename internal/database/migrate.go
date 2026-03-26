package database

import (
	"database/sql"
	"fmt"
	"log"

	"github.com/pressly/goose/v3"
)

// Migrate runs all pending migrations against the database.
func Migrate(db *DB) error {
	goose.SetLogger(goose.NopLogger())

	if err := goose.SetDialect(gooseDialect(db.dialect)); err != nil {
		return fmt.Errorf("set goose dialect: %w", err)
	}

	// Go-registered migrations don't need a directory, but goose requires
	// a non-empty string. Use "." — it won't find SQL files and that's fine.
	if err := goose.Up(db.sql, "."); err != nil {
		return fmt.Errorf("run migrations: %w", err)
	}

	log.Printf("migrations complete (dialect=%s)", db.dialect)
	return nil
}

func gooseDialect(dialect string) string {
	switch dialect {
	case "postgres":
		return "postgres"
	default:
		return "sqlite3"
	}
}

// detectDialect determines the dialect from a *sql.DB connection.
func detectDialect(db *sql.DB) string {
	var result string
	err := db.QueryRow("PRAGMA journal_mode").Scan(&result)
	if err == nil {
		return "sqlite"
	}
	return "postgres"
}
