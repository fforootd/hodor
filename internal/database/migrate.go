package database

import (
	"context"
	"embed"
	"fmt"
	"io/fs"

	"github.com/pressly/goose/v3"
	"github.com/pressly/goose/v3/lock"

	"github.com/zitadel/zitadel/internal/logging"
)

//go:embed migrations/sqlite/*.sql
var sqliteMigrations embed.FS

//go:embed migrations/postgres/*.sql
var postgresMigrations embed.FS

// Migrate runs all pending Goose migrations for the current dialect.
// On first run against a fresh DB this applies all migrations.
// On subsequent runs it applies only new ones (tracked in goose_db_version).
//
// For Postgres, an advisory lock is acquired so concurrent Migrate() calls
// (e.g., multiple K8s init containers) are safe — only one runs DDL at a time,
// others block until the lock is released.
func Migrate(db *DB) error {
	if db.dialect == "postgres" {
		return migratePostgres(db)
	}
	return migrateSQLite(db)
}

// migrateSQLite uses the global goose API (no advisory lock needed —
// SQLite is single-process by nature).
func migrateSQLite(db *DB) error {
	goose.SetLogger(goose.NopLogger())

	if err := goose.SetDialect("sqlite3"); err != nil {
		return fmt.Errorf("goose set dialect sqlite3: %w", err)
	}

	goose.SetBaseFS(sqliteMigrations)

	if err := goose.Up(db.sql, "migrations/sqlite"); err != nil {
		return fmt.Errorf("goose up (sqlite): %w", err)
	}

	ver, err := goose.GetDBVersion(db.sql)
	if err == nil {
		logging.Printf("schema ready (dialect=sqlite, version=%d)", ver)
	} else {
		logging.Printf("schema ready (dialect=sqlite)")
	}
	return nil
}

// migratePostgres uses the goose.Provider API with a session-level advisory
// lock to prevent concurrent DDL from multiple processes.
func migratePostgres(db *DB) error {
	sessionLocker, err := lock.NewPostgresSessionLocker()
	if err != nil {
		return fmt.Errorf("create postgres session locker: %w", err)
	}

	migrationFS, err := fs.Sub(postgresMigrations, "migrations/postgres")
	if err != nil {
		return fmt.Errorf("sub fs for postgres migrations: %w", err)
	}

	provider, err := goose.NewProvider(
		goose.DialectPostgres,
		db.sql,
		migrationFS,
		goose.WithSessionLocker(sessionLocker),
	)
	if err != nil {
		return fmt.Errorf("create goose provider (postgres): %w", err)
	}

	results, err := provider.Up(context.Background())
	if err != nil {
		return fmt.Errorf("goose up (postgres): %w", err)
	}

	applied := 0
	for _, r := range results {
		if r.Error != nil {
			return fmt.Errorf("migration %s failed: %w", r.Source.Path, r.Error)
		}
		applied++
	}

	ver, verErr := provider.GetDBVersion(context.Background())
	if verErr == nil {
		logging.Printf("schema ready (dialect=postgres, version=%d, applied=%d)", ver, applied)
	} else {
		logging.Printf("schema ready (dialect=postgres, applied=%d)", applied)
	}
	return nil
}

// CheckVersion performs a read-only check of the current schema version
// against the target (highest embedded migration). Returns an error if the
// database schema is behind the binary's expectations.
//
// Use this in 'zitadel start' when migrate mode is "check" — it ensures
// the schema is compatible without running DDL.
func CheckVersion(db *DB) error {
	current, target, err := VersionInfo(db)
	if err != nil {
		return err
	}

	if current < target {
		return fmt.Errorf(
			"schema version %d is behind target %d — run 'zitadel migrate' first\n"+
				"  Hint: use 'zitadel start --auto-migrate' to auto-migrate",
			current, target,
		)
	}
	if current > target {
		logging.Printf("WARN: schema version %d is ahead of binary target %d — binary may be outdated", current, target)
	}
	return nil
}

// VersionInfo returns the current database schema version and the target
// version (highest embedded migration number) for the current dialect.
func VersionInfo(db *DB) (current, target int64, err error) {
	goose.SetLogger(goose.NopLogger())

	dialect := db.gooseDialect()
	if err := goose.SetDialect(dialect); err != nil {
		return 0, 0, fmt.Errorf("goose set dialect %s: %w", dialect, err)
	}
	goose.SetBaseFS(db.migrationFS())

	current, err = goose.GetDBVersion(db.sql)
	if err != nil {
		return 0, 0, fmt.Errorf("get current schema version: %w", err)
	}

	// Collect all embedded migrations to find the highest version.
	migrations, collectErr := goose.CollectMigrations(db.migrationDir(), 0, goose.MaxVersion)
	if collectErr != nil {
		return current, 0, fmt.Errorf("collect migrations: %w", collectErr)
	}
	if len(migrations) > 0 {
		target = migrations[len(migrations)-1].Version
	}

	return current, target, nil
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

// MigrateStatus prints the status of all migrations to stdout.
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
