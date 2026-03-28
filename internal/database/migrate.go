package database

import (
	"embed"
	"fmt"
	"io/fs"
	"log"

	"github.com/pressly/goose/v3"
)

//go:embed migrations/sqlite/*.sql
var sqliteMigrations embed.FS

//go:embed migrations/postgres/*.sql
var postgresMigrations embed.FS

// Migrate runs all pending Goose migrations for the current dialect.
// On first run against a fresh DB this applies all migrations.
// On subsequent runs it applies only new ones (tracked in goose_db_version).
func Migrate(db *DB) error {
	dialect := db.gooseDialect()
	migrations := db.migrationFS()
	dir := db.migrationDir()

	goose.SetLogger(goose.NopLogger()) // suppress noisy per-migration logs

	if err := goose.SetDialect(dialect); err != nil {
		return fmt.Errorf("goose set dialect %s: %w", dialect, err)
	}

	// goose.SetBaseFS expects the root FS; the dir is relative within it.
	goose.SetBaseFS(migrations)

	if err := goose.Up(db.sql, dir); err != nil {
		return fmt.Errorf("goose up (%s): %w", dialect, err)
	}

	// Log final version.
	ver, err := goose.GetDBVersion(db.sql)
	if err == nil {
		log.Printf("schema ready (dialect=%s, version=%d)", db.dialect, ver)
	} else {
		log.Printf("schema ready (dialect=%s)", db.dialect)
	}
	return nil
}

// MigrateDown rolls back the last migration. Useful for testing.
func MigrateDown(db *DB) error {
	dialect := db.gooseDialect()
	migrations := db.migrationFS()
	dir := db.migrationDir()

	if err := goose.SetDialect(dialect); err != nil {
		return fmt.Errorf("goose set dialect: %w", err)
	}
	goose.SetBaseFS(migrations)
	return goose.Down(db.sql, dir)
}

// MigrateStatus prints the status of all migrations. Useful for debugging.
func MigrateStatus(db *DB) error {
	dialect := db.gooseDialect()
	migrations := db.migrationFS()
	dir := db.migrationDir()

	if err := goose.SetDialect(dialect); err != nil {
		return fmt.Errorf("goose set dialect: %w", err)
	}
	goose.SetBaseFS(migrations)
	return goose.Status(db.sql, dir)
}

func (d *DB) migrationFS() fs.FS {
	if d.dialect == "postgres" {
		return postgresMigrations
	}
	return sqliteMigrations
}

func (d *DB) gooseDialect() string {
	if d.dialect == "postgres" {
		return "postgres"
	}
	return "sqlite3"
}

func (d *DB) migrationDir() string {
	return "migrations/" + d.dialect
}
