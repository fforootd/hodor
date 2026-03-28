// Package database provides a unified interface for SQLite and Postgres.
// The dialect is auto-detected from the connection string:
//   - "sqlite://..." or "" → SQLite (WAL mode, pure Go)
//   - "postgres://..." → PostgreSQL (pgx)
package database

import (
	"database/sql"
	"fmt"
	"strings"
	"time"
)

// DB wraps a *sql.DB with dialect awareness.
type DB struct {
	sql     *sql.DB
	dialect string // "sqlite" or "postgres"
}

// SQL returns the underlying *sql.DB.
func (d *DB) SQL() *sql.DB {
	return d.sql
}

// Dialect returns "sqlite" or "postgres".
func (d *DB) Dialect() string {
	return d.dialect
}

// Close closes the database connection.
func (d *DB) Close() error {
	return d.sql.Close()
}

// Open connects to a database based on the connection string.
// Empty string or "sqlite://..." opens SQLite; "postgres://..." opens Postgres.
func Open(connStr string) (*DB, error) {
	switch {
	case connStr == "" || strings.HasPrefix(connStr, "sqlite://"):
		return openSQLite(connStr)
	case strings.HasPrefix(connStr, "postgres://") || strings.HasPrefix(connStr, "postgresql://"):
		return openPostgres(connStr)
	default:
		return nil, fmt.Errorf("unsupported database URL scheme: %s", connStr)
	}
}

// PoolConfig holds connection pool settings for Postgres.
type PoolConfig struct {
	MaxOpenConns    int
	MaxIdleConns    int
	ConnMaxLifetime time.Duration
}

// OpenWithConfig connects to a database with explicit pool settings.
// For SQLite, pool settings are ignored (SQLite manages its own connections).
func OpenWithConfig(connStr string, pool PoolConfig) (*DB, error) {
	switch {
	case connStr == "" || strings.HasPrefix(connStr, "sqlite://"):
		return openSQLite(connStr)
	case strings.HasPrefix(connStr, "postgres://") || strings.HasPrefix(connStr, "postgresql://"):
		return openPostgresWithPool(connStr, pool.MaxOpenConns, pool.MaxIdleConns, pool.ConnMaxLifetime)
	default:
		return nil, fmt.Errorf("unsupported database URL scheme: %s", connStr)
	}
}

// JSONExtract returns the dialect-specific JSON extraction syntax.
// SQLite: json_extract(col, '$.path')
// Postgres: col->>'path'
func (d *DB) JSONExtract(column, path string) string {
	switch d.dialect {
	case "postgres":
		return fmt.Sprintf("%s->>'%s'", column, path)
	default:
		return fmt.Sprintf("json_extract(%s, '$.%s')", column, path)
	}
}

// Placeholder returns the dialect-specific parameter placeholder.
// SQLite: ? (positional)
// Postgres: $1, $2, ... (numbered)
func (d *DB) Placeholder(n int) string {
	switch d.dialect {
	case "postgres":
		return fmt.Sprintf("$%d", n)
	default:
		return "?"
	}
}

// TimestampNow returns the dialect-specific current timestamp expression.
func (d *DB) TimestampNow() string {
	switch d.dialect {
	case "postgres":
		return "NOW()"
	default:
		return "datetime('now')"
	}
}
