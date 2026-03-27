package database

import (
	"database/sql"
	"fmt"
	"net/url"
	"strings"

	_ "modernc.org/sqlite" // Pure Go SQLite driver.
)

func openSQLite(connStr string) (*DB, error) {
	// Strip sqlite:// prefix to get the file path.
	path := strings.TrimPrefix(connStr, "sqlite://")
	if path == "" {
		path = "./zitadel.db"
	}

	// URL-encode the path to handle special characters (e.g., '#' in
	// Go test temp dir names like "FuzzAPIJSON/seed#0") that would
	// otherwise be interpreted as URI fragment delimiters.
	escapedPath := url.PathEscape(path)
	// PathEscape encodes '/' too, but SQLite file: URIs need literal slashes.
	escapedPath = strings.ReplaceAll(escapedPath, "%2F", "/")

	// WAL mode for concurrent reads, busy timeout for write contention,
	// foreign keys enforcement, immediate txlock to prevent SQLITE_BUSY deadlocks.
	dsn := fmt.Sprintf("file:%s?_pragma=journal_mode(WAL)&_pragma=busy_timeout(10000)&_pragma=foreign_keys(ON)&_txlock=immediate", escapedPath)

	sqlDB, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("open sqlite %s: %w", path, err)
	}

	// WAL supports concurrent reads with a single writer.
	// Large pool so scheduler, SSE streams, and API handlers don't deadlock.
	sqlDB.SetMaxOpenConns(16)

	if err := sqlDB.Ping(); err != nil {
		sqlDB.Close()
		return nil, fmt.Errorf("ping sqlite: %w", err)
	}

	return &DB{sql: sqlDB, dialect: "sqlite"}, nil
}
