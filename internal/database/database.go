// Package database provides a unified interface for SQLite, Postgres, and
// Cloudflare D1. The dialect is auto-detected from the connection string:
//   - "sqlite://..." or "" → SQLite (WAL mode, pure Go)
//   - "postgres://..." → PostgreSQL (pgx)
//   - "d1://..." → Cloudflare D1 (via HTTP proxy, SQLite-compatible dialect)
package database

import (
	"database/sql"
	"fmt"
	"strings"
	"time"

	// Register the D1 driver so sql.Open("d1", ...) works.
	_ "github.com/zitadel/zitadel/internal/database/d1driver"
)

// DB wraps a *sql.DB with dialect awareness.
type DB struct {
	sql     *sql.DB
	dialect string // "sqlite", "postgres", or "d1"
}

// SQL returns the underlying *sql.DB.
func (d *DB) SQL() *sql.DB {
	return d.sql
}

// Dialect returns "sqlite", "postgres", or "d1".
// D1 uses SQLite-compatible SQL syntax.
func (d *DB) Dialect() string {
	return d.dialect
}

// IsSQLiteCompat reports whether the dialect uses SQLite SQL syntax.
// Both "sqlite" and "d1" return true.
func (d *DB) IsSQLiteCompat() bool {
	return d.dialect == "sqlite" || d.dialect == "d1"
}

// Close closes the database connection.
func (d *DB) Close() error {
	return d.sql.Close()
}

// Open connects to a database based on the connection string.
// Empty string or "sqlite://..." opens SQLite; "postgres://..." opens Postgres;
// "d1://..." opens a Cloudflare D1 database via the HTTP proxy driver.
func Open(connStr string) (*DB, error) {
	switch {
	case connStr == "" || strings.HasPrefix(connStr, "sqlite://"):
		return openSQLite(connStr)
	case strings.HasPrefix(connStr, "postgres://") || strings.HasPrefix(connStr, "postgresql://"):
		return openPostgres(connStr)
	case strings.HasPrefix(connStr, "d1://"):
		return openD1(connStr)
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
// For SQLite and D1, pool settings are ignored.
func OpenWithConfig(connStr string, pool PoolConfig) (*DB, error) {
	switch {
	case connStr == "" || strings.HasPrefix(connStr, "sqlite://"):
		return openSQLite(connStr)
	case strings.HasPrefix(connStr, "postgres://") || strings.HasPrefix(connStr, "postgresql://"):
		return openPostgresWithPool(connStr, pool.MaxOpenConns, pool.MaxIdleConns, pool.ConnMaxLifetime)
	case strings.HasPrefix(connStr, "d1://"):
		return openD1(connStr)
	default:
		return nil, fmt.Errorf("unsupported database URL scheme: %s", connStr)
	}
}

// openD1 connects to a Cloudflare D1 database via the HTTP proxy driver.
// The connection string format is: d1://hostname (e.g., d1://d1.local)
// which gets translated to http://hostname for the driver.
func openD1(connStr string) (*DB, error) {
	// d1://d1.local → http://d1.local
	proxyURL := "http://" + strings.TrimPrefix(connStr, "d1://")

	sqlDB, err := sql.Open("d1", proxyURL)
	if err != nil {
		return nil, fmt.Errorf("open d1 %s: %w", proxyURL, err)
	}

	// D1 handles connection pooling on the Cloudflare side.
	sqlDB.SetMaxOpenConns(4)

	return &DB{sql: sqlDB, dialect: "d1"}, nil
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

// Rebind rewrites generic "?" placeholders into the current dialect's format.
// SQLite-compatible dialects keep "?" while Postgres uses numbered placeholders.
func (d *DB) Rebind(query string) string {
	return RebindPlaceholders(query, d.dialect)
}

// RebindPlaceholders rewrites generic "?" placeholders into the given dialect's
// placeholder syntax.
func RebindPlaceholders(query, dialect string) string {
	if dialect != "postgres" {
		return query
	}
	var out strings.Builder
	out.Grow(len(query) + 8)
	index := 1
	for _, ch := range query {
		if ch == '?' {
			out.WriteString(fmt.Sprintf("$%d", index))
			index++
			continue
		}
		out.WriteRune(ch)
	}
	return out.String()
}
