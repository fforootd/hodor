package database

import (
	"context"
	"database/sql"
	"database/sql/driver"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/tursodatabase/libsql-client-go/libsql"
)

const (
	defaultLibSQLMaxOpenConns    = 10
	defaultLibSQLMaxIdleConns    = 5
	defaultLibSQLConnMaxLifetime = time.Hour
)

func isLibSQLURL(connStr string) bool {
	return strings.HasPrefix(connStr, "libsql://") ||
		strings.HasPrefix(connStr, "https://") ||
		strings.HasPrefix(connStr, "http://") ||
		strings.HasPrefix(connStr, "wss://") ||
		strings.HasPrefix(connStr, "ws://")
}

func openLibSQL(connStr string) (*DB, error) {
	return openLibSQLWithPool(connStr, defaultLibSQLMaxOpenConns, defaultLibSQLMaxIdleConns, defaultLibSQLConnMaxLifetime)
}

func openLibSQLForFGA(connStr string) (*DB, error) {
	return openLibSQLWithPoolOptions(
		connStr,
		defaultLibSQLMaxOpenConns,
		defaultLibSQLMaxIdleConns,
		defaultLibSQLConnMaxLifetime,
		libsqlOpenOptions{downgradeTxIsolation: true},
	)
}

func openLibSQLWithPool(connStr string, maxOpen, maxIdle int, connMaxLifetime time.Duration) (*DB, error) {
	return openLibSQLWithPoolOptions(
		connStr,
		maxOpen,
		maxIdle,
		connMaxLifetime,
		libsqlOpenOptions{},
	)
}

type libsqlOpenOptions struct {
	downgradeTxIsolation bool
}

func openLibSQLWithPoolOptions(connStr string, maxOpen, maxIdle int, connMaxLifetime time.Duration, opts libsqlOpenOptions) (*DB, error) {
	sqlDB, err := openLibSQLDB(connStr, opts)
	if err != nil {
		return nil, err
	}

	if maxOpen <= 0 {
		maxOpen = defaultLibSQLMaxOpenConns
	}
	if maxIdle <= 0 {
		maxIdle = defaultLibSQLMaxIdleConns
	}
	if connMaxLifetime <= 0 {
		connMaxLifetime = defaultLibSQLConnMaxLifetime
	}

	sqlDB.SetMaxOpenConns(maxOpen)
	sqlDB.SetMaxIdleConns(maxIdle)
	sqlDB.SetConnMaxLifetime(connMaxLifetime)

	if err := sqlDB.Ping(); err != nil {
		sqlDB.Close()
		return nil, fmt.Errorf("ping libsql: %w", err)
	}

	return &DB{sql: sqlDB, dialect: "libsql"}, nil
}

func openLibSQLDB(connStr string, opts libsqlOpenOptions) (*sql.DB, error) {
	authToken := strings.TrimSpace(firstNonEmpty(
		os.Getenv("ZITADEL_DATABASE_AUTH_TOKEN"),
		os.Getenv("TURSO_AUTH_TOKEN"),
	))

	connectorOptions := make([]libsql.Option, 0, 1)
	if authToken != "" {
		connectorOptions = append(connectorOptions, libsql.WithAuthToken(authToken))
	}

	connector, err := libsql.NewConnector(connStr, connectorOptions...)
	if err != nil {
		return nil, fmt.Errorf("new libsql connector for %s: %w", connStr, err)
	}
	if opts.downgradeTxIsolation {
		connector = libsqlTxCompatConnector{base: connector}
	}

	return sql.OpenDB(connector), nil
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}

type libsqlTxCompatConnector struct {
	base driver.Connector
}

func (c libsqlTxCompatConnector) Connect(ctx context.Context) (driver.Conn, error) {
	conn, err := c.base.Connect(ctx)
	if err != nil {
		return nil, err
	}

	beginTx, ok := conn.(driver.ConnBeginTx)
	if !ok {
		return conn, nil
	}

	return &libsqlTxCompatConn{Conn: conn, beginTx: beginTx}, nil
}

func (c libsqlTxCompatConnector) Driver() driver.Driver {
	return c.base.Driver()
}

type libsqlTxCompatConn struct {
	driver.Conn
	beginTx driver.ConnBeginTx
}

func (c *libsqlTxCompatConn) BeginTx(ctx context.Context, opts driver.TxOptions) (driver.Tx, error) {
	if opts.Isolation != driver.IsolationLevel(sql.LevelDefault) {
		opts.Isolation = driver.IsolationLevel(sql.LevelDefault)
	}
	return c.beginTx.BeginTx(ctx, opts)
}

func (c *libsqlTxCompatConn) CheckNamedValue(value *driver.NamedValue) error {
	checker, ok := c.Conn.(driver.NamedValueChecker)
	if !ok {
		return driver.ErrSkip
	}
	return checker.CheckNamedValue(value)
}

func (c *libsqlTxCompatConn) ExecContext(ctx context.Context, query string, args []driver.NamedValue) (driver.Result, error) {
	execer, ok := c.Conn.(driver.ExecerContext)
	if !ok {
		return nil, driver.ErrSkip
	}
	return execer.ExecContext(ctx, query, args)
}

func (c *libsqlTxCompatConn) Ping(ctx context.Context) error {
	pinger, ok := c.Conn.(driver.Pinger)
	if !ok {
		return nil
	}
	return pinger.Ping(ctx)
}

func (c *libsqlTxCompatConn) PrepareContext(ctx context.Context, query string) (driver.Stmt, error) {
	preparer, ok := c.Conn.(driver.ConnPrepareContext)
	if !ok {
		return c.Conn.Prepare(query)
	}
	return preparer.PrepareContext(ctx, query)
}

func (c *libsqlTxCompatConn) QueryContext(ctx context.Context, query string, args []driver.NamedValue) (driver.Rows, error) {
	queryer, ok := c.Conn.(driver.QueryerContext)
	if !ok {
		return nil, driver.ErrSkip
	}
	return queryer.QueryContext(ctx, query, args)
}

func (c *libsqlTxCompatConn) ResetSession(ctx context.Context) error {
	resetter, ok := c.Conn.(driver.SessionResetter)
	if !ok {
		return nil
	}
	return resetter.ResetSession(ctx)
}

func (c *libsqlTxCompatConn) IsValid() bool {
	validator, ok := c.Conn.(driver.Validator)
	if !ok {
		return true
	}
	return validator.IsValid()
}
