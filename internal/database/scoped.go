package database

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"github.com/zitadel/zitadel/internal/httputil"
)

// ScopedDB wraps a *sql.DB with an instance ID for multi-tenant query scoping.
// Handlers obtain a ScopedDB via db.Scoped(ctx) and use InstanceID() when
// constructing queries. The instance ID is explicit in every query — no hidden
// SQL rewriting.
//
// Example:
//
//	scoped := a.db.Scoped(r.Context())
//	rows, err := scoped.QueryContext(ctx,
//	    scoped.Rebind(`SELECT id, name FROM orgs WHERE instance_id = ? AND id > ? LIMIT ?`),
//	    scoped.InstanceID(), cursor, limit)
type ScopedDB struct {
	db         *sql.DB
	instanceID string
	dialect    string
	onWrite    func()
}

// InstanceID returns the instance ID for this scoped connection.
// Use this value in query arguments: WHERE instance_id = ?
func (s *ScopedDB) InstanceID() string {
	return s.instanceID
}

// Dialect returns the SQL dialect.
func (s *ScopedDB) Dialect() string {
	return s.dialect
}

// IsSQLiteCompat reports whether the dialect uses SQLite SQL syntax.
func (s *ScopedDB) IsSQLiteCompat() bool {
	return s.dialect == "sqlite" || s.dialect == "d1" || s.dialect == "libsql"
}

// Rebind rewrites "?" placeholders for the current dialect.
func (s *ScopedDB) Rebind(query string) string {
	return RebindPlaceholders(query, s.dialect)
}

// Placeholder returns the dialect-specific parameter placeholder.
func (s *ScopedDB) Placeholder(n int) string {
	if s.dialect == "postgres" {
		return fmt.Sprintf("$%d", n)
	}
	return "?"
}

// JSONExtract returns the dialect-specific JSON extraction syntax.
func (s *ScopedDB) JSONExtract(column, path string) string {
	if s.dialect == "postgres" {
		return fmt.Sprintf("%s->>'%s'", column, path)
	}
	return fmt.Sprintf("json_extract(%s, '$.%s')", column, path)
}

// TimestampNow returns the dialect-specific current timestamp expression.
func (s *ScopedDB) TimestampNow() string {
	if s.dialect == "postgres" {
		return "NOW()"
	}
	return "datetime('now')"
}

// QueryContext executes a query that returns rows.
func (s *ScopedDB) QueryContext(ctx context.Context, query string, args ...any) (*sql.Rows, error) {
	return s.db.QueryContext(ctx, query, args...)
}

// QueryRowContext executes a query that returns at most one row.
func (s *ScopedDB) QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row {
	return s.db.QueryRowContext(ctx, query, args...)
}

// ExecContext executes a query that doesn't return rows.
func (s *ScopedDB) ExecContext(ctx context.Context, query string, args ...any) (sql.Result, error) {
	return s.db.ExecContext(ctx, query, args...)
}

// BeginTx starts a scoped transaction.
func (s *ScopedDB) BeginTx(ctx context.Context, opts *sql.TxOptions) (*ScopedTx, error) {
	tx, err := s.db.BeginTx(ctx, opts)
	if err != nil {
		return nil, err
	}
	return &ScopedTx{
		tx:         tx,
		instanceID: s.instanceID,
		dialect:    s.dialect,
		onWrite:    s.onWrite,
	}, nil
}

// SQL returns the underlying *sql.DB. Use sparingly — prefer scoped methods.
// Flagged by the audit test (Phase 7) when used outside the allowlist.
func (s *ScopedDB) SQL() *sql.DB {
	return s.db
}

// ---------------------------------------------------------------------------
// ScopedTx — scoped transaction
// ---------------------------------------------------------------------------

// ScopedTx wraps a *sql.Tx with an instance ID.
type ScopedTx struct {
	tx         *sql.Tx
	instanceID string
	dialect    string
	onWrite    func()
}

// InstanceID returns the instance ID for this transaction.
func (s *ScopedTx) InstanceID() string {
	return s.instanceID
}

// Dialect returns the SQL dialect.
func (s *ScopedTx) Dialect() string {
	return s.dialect
}

// Rebind rewrites "?" placeholders for the current dialect.
func (s *ScopedTx) Rebind(query string) string {
	return RebindPlaceholders(query, s.dialect)
}

// Placeholder returns the dialect-specific parameter placeholder.
func (s *ScopedTx) Placeholder(n int) string {
	if s.dialect == "postgres" {
		return fmt.Sprintf("$%d", n)
	}
	return "?"
}

// JSONExtract returns the dialect-specific JSON extraction syntax.
func (s *ScopedTx) JSONExtract(column, path string) string {
	if s.dialect == "postgres" {
		return fmt.Sprintf("%s->>'%s'", column, path)
	}
	return fmt.Sprintf("json_extract(%s, '$.%s')", column, path)
}

// TimestampNow returns the dialect-specific current timestamp expression.
func (s *ScopedTx) TimestampNow() string {
	if s.dialect == "postgres" {
		return "NOW()"
	}
	return "datetime('now')"
}

// QueryContext executes a query that returns rows within the transaction.
func (s *ScopedTx) QueryContext(ctx context.Context, query string, args ...any) (*sql.Rows, error) {
	return s.tx.QueryContext(ctx, query, args...)
}

// QueryRowContext executes a query that returns at most one row.
func (s *ScopedTx) QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row {
	return s.tx.QueryRowContext(ctx, query, args...)
}

// ExecContext executes a query that doesn't return rows.
func (s *ScopedTx) ExecContext(ctx context.Context, query string, args ...any) (sql.Result, error) {
	return s.tx.ExecContext(ctx, query, args...)
}

// Commit commits the transaction and signals write notification.
func (s *ScopedTx) Commit() error {
	if err := s.tx.Commit(); err != nil {
		return err
	}
	if s.onWrite != nil {
		s.onWrite()
	}
	return nil
}

// Rollback aborts the transaction.
func (s *ScopedTx) Rollback() error {
	return s.tx.Rollback()
}

// Tx returns the underlying *sql.Tx. Use sparingly.
func (s *ScopedTx) Tx() *sql.Tx {
	return s.tx
}

// ---------------------------------------------------------------------------
// Execer interface — shared by ScopedDB and ScopedTx for emitEvent etc.
// ---------------------------------------------------------------------------

// ScopedExecer is implemented by both ScopedDB and ScopedTx, allowing
// functions like emitEvent to work with either.
type ScopedExecer interface {
	ExecContext(ctx context.Context, query string, args ...any) (sql.Result, error)
	InstanceID() string
	Rebind(query string) string
}

// Verify interface compliance at compile time.
var (
	_ ScopedExecer = (*ScopedDB)(nil)
	_ ScopedExecer = (*ScopedTx)(nil)
)

// ---------------------------------------------------------------------------
// DB.Scoped() — the entry point
// ---------------------------------------------------------------------------

// Scoped returns a ScopedDB bound to the instance ID from the request context.
// This is the primary entry point for all database operations in handlers.
//
//	scoped := a.db.Scoped(r.Context())
func (d *DB) Scoped(ctx context.Context) *ScopedDB {
	return &ScopedDB{
		db:         d.sql,
		instanceID: httputil.InstanceIDFromContext(ctx),
		dialect:    d.dialect,
		onWrite:    d.onWrite,
	}
}

// ScopedDefault returns a ScopedDB bound to DefaultInstanceID.
// Use for startup operations (seed, bootstrap, migrations) that run
// outside of HTTP request context.
func (d *DB) ScopedDefault() *ScopedDB {
	return &ScopedDB{
		db:         d.sql,
		instanceID: httputil.DefaultInstanceID,
		dialect:    d.dialect,
		onWrite:    d.onWrite,
	}
}

// ---------------------------------------------------------------------------
// Rebind helper for ScopedDB (used by patchBuilder etc.)
// ---------------------------------------------------------------------------

// BindQueryForDialect is a package-level helper for code that needs rebinding
// without a ScopedDB/ScopedTx (e.g., patchBuilder).
func BindQueryForDialect(query, dialect string) string {
	return RebindPlaceholders(query, dialect)
}

// IsSQLiteCompatDialect checks if a dialect string is SQLite-compatible.
func IsSQLiteCompatDialect(dialect string) bool {
	return dialect == "sqlite" || dialect == "d1" || dialect == "libsql"
}

// bindQueryForDialect is an internal alias used by existing helpers.go code.
func bindQueryForDialect(query, dialect string) string {
	return RebindPlaceholders(query, dialect)
}

// placeholderForDialect returns the placeholder syntax for a dialect.
func placeholderForDialect(n int, dialect string) string {
	if dialect == "postgres" {
		return fmt.Sprintf("$%d", n)
	}
	return "?"
}

// jsonExtractForDialect returns the JSON extraction syntax for a dialect.
func jsonExtractForDialect(column, path, dialect string) string {
	if dialect == "postgres" {
		return fmt.Sprintf("%s->>'%s'", column, path)
	}
	return fmt.Sprintf("json_extract(%s, '$.%s')", column, path)
}

// timestampNowForDialect returns the current timestamp expression for a dialect.
func timestampNowForDialect(dialect string) string {
	if dialect == "postgres" {
		return "NOW()"
	}
	return "datetime('now')"
}

// Ensure unused import doesn't cause issues
var _ = strings.Builder{}
