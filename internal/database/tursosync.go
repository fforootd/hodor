package database

import (
	"context"
	"database/sql"
	"database/sql/driver"
	"fmt"
	"log/slog"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/tursodatabase/libsql-client-go/libsql"
	turso "turso.tech/database/tursogo"
)

const (
	defaultTursoSyncMaxOpenConns    = 10
	defaultTursoSyncMaxIdleConns    = 5
	defaultTursoSyncConnMaxLifetime = time.Hour
	pullInterval                    = 1 * time.Second
)

// TursoSyncDB wraps a split read/write database:
//   - Reads  → local in-memory partial replica (sub-ms via turso+sync)
//   - Writes → remote Turso primary (via HTTP libsql driver, immediate consistency)
//
// After each write, the local replica pulls to stay current.
type TursoSyncDB struct {
	*DB // exposes the read-path *sql.DB as the "main" database

	writeDB    *sql.DB          // HTTP connection to Turso primary (for writes)
	syncEngine *turso.TursoSyncDb // sync engine for pull
	pullMu     sync.Mutex
}

// Pull fetches remote changes into the local read replica.
func (t *TursoSyncDB) Pull(ctx context.Context) (bool, error) {
	t.pullMu.Lock()
	defer t.pullMu.Unlock()
	return t.syncEngine.Pull(ctx)
}

// Stats returns sync statistics.
func (t *TursoSyncDB) Stats(ctx context.Context) (turso.TursoSyncDbStats, error) {
	return t.syncEngine.Stats(ctx)
}

// WriteDB returns the HTTP-connected *sql.DB for direct writes to remote.
func (t *TursoSyncDB) WriteDB() *sql.DB {
	return t.writeDB
}

// Close closes both the read and write connections.
func (t *TursoSyncDB) Close() error {
	var errs []error
	if err := t.DB.Close(); err != nil {
		errs = append(errs, err)
	}
	if err := t.writeDB.Close(); err != nil {
		errs = append(errs, err)
	}
	if len(errs) > 0 {
		return errs[0]
	}
	return nil
}

// isTursoSyncURL checks for the turso+sync:// scheme.
func isTursoSyncURL(connStr string) bool {
	return strings.HasPrefix(connStr, "turso+sync://")
}

// parseTursoSyncURL extracts the remote URL and options.
func parseTursoSyncURL(connStr string) (remoteURL string, localPath string, partial turso.TursoPartialSyncConfig) {
	raw := strings.TrimPrefix(connStr, "turso+sync://")

	parts := strings.SplitN(raw, "?", 2)
	remoteURL = "libsql://" + parts[0]
	localPath = ":memory:"

	// Default: partial sync with prefix bootstrap
	partial = turso.TursoPartialSyncConfig{
		BootstrapStrategyPrefix: 128 * 1024,
		Prefetch:                true,
	}

	if len(parts) == 2 {
		for _, kv := range strings.Split(parts[1], "&") {
			pair := strings.SplitN(kv, "=", 2)
			if len(pair) != 2 {
				continue
			}
			switch pair[0] {
			case "path":
				localPath = pair[1]
			case "prefix_size":
				var size int
				if _, err := fmt.Sscanf(pair[1], "%d", &size); err == nil {
					partial.BootstrapStrategyPrefix = size
				}
			case "segment_size":
				var size int
				if _, err := fmt.Sscanf(pair[1], "%d", &size); err == nil {
					partial.SegmentSize = size
				}
			case "prefetch":
				partial.Prefetch = pair[1] == "true" || pair[1] == "1"
			}
		}
	}

	return remoteURL, localPath, partial
}

func openTursoSync(connStr string) (*TursoSyncDB, error) {
	remoteURL, localPath, partialCfg := parseTursoSyncURL(connStr)

	authToken := strings.TrimSpace(firstNonEmpty(
		os.Getenv("ZITADEL_DATABASE_AUTH_TOKEN"),
		os.Getenv("TURSO_AUTH_TOKEN"),
	))

	// ── Read path: local partial sync replica ────────────────────────────
	syncDB, err := turso.NewTursoSyncDb(context.Background(), turso.TursoSyncDbConfig{
		Path:                    localPath,
		RemoteUrl:               remoteURL,
		AuthToken:               authToken,
		PartialSyncExperimental: partialCfg,
		BusyTimeout:             10000,
	})
	if err != nil {
		return nil, fmt.Errorf("turso sync read replica: %w", err)
	}

	readDB, err := syncDB.Connect(context.Background())
	if err != nil {
		return nil, fmt.Errorf("turso sync read connect: %w", err)
	}
	readDB.SetMaxOpenConns(defaultTursoSyncMaxOpenConns)
	readDB.SetMaxIdleConns(defaultTursoSyncMaxIdleConns)
	readDB.SetConnMaxLifetime(defaultTursoSyncConnMaxLifetime)

	// ── Write path: HTTP direct to Turso primary ─────────────────────────
	writeConnector, err := newHTTPWriteConnector(remoteURL, authToken)
	if err != nil {
		readDB.Close()
		return nil, fmt.Errorf("turso http write connector: %w", err)
	}

	// Verify write path works
	writeDB := sql.OpenDB(writeConnector)
	writeDB.SetMaxOpenConns(defaultTursoSyncMaxOpenConns)
	if err := writeDB.Ping(); err != nil {
		readDB.Close()
		writeDB.Close()
		return nil, fmt.Errorf("turso write ping: %w", err)
	}

	initPullChannel()

	tsdb := &TursoSyncDB{
		DB:         &DB{sql: readDB, dialect: "libsql"},
		writeDB:    writeDB,
		syncEngine: syncDB,
	}

	// Build split read/write *sql.DB: SELECT → local, writes → remote
	readConnector := &extractConnector{db: readDB}
	splitDB := sql.OpenDB(&splitRWConnector{
		readBase:  readConnector,
		writeBase: writeConnector,
		onWrite:   SignalPull,
	})
	splitDB.SetMaxOpenConns(defaultTursoSyncMaxOpenConns)
	splitDB.SetMaxIdleConns(defaultTursoSyncMaxIdleConns)
	splitDB.SetConnMaxLifetime(defaultTursoSyncConnMaxLifetime)

	// Override the DB's sql with the split router
	tsdb.DB.sql = splitDB

	// Pull periodically so reads see recent writes.
	go tsdb.pullLoop()

	slog.Info("turso split read/write ready",
		"read", "local partial sync (in-memory)",
		"write", "HTTP to "+remoteURL,
	)

	return tsdb, nil
}

// extractConnector wraps an existing *sql.DB as a driver.Connector.
type extractConnector struct {
	db *sql.DB
}

func (c *extractConnector) Connect(ctx context.Context) (driver.Conn, error) {
	conn, err := c.db.Conn(ctx)
	if err != nil {
		return nil, err
	}
	var driverConn driver.Conn
	err = conn.Raw(func(dc interface{}) error {
		var ok bool
		driverConn, ok = dc.(driver.Conn)
		if !ok {
			return fmt.Errorf("underlying connection is not driver.Conn")
		}
		return nil
	})
	if err != nil {
		conn.Close()
		return nil, err
	}
	// Note: we intentionally don't close conn — the driver.Conn is borrowed
	// from the pool and will be managed by the splitRW pool.
	return driverConn, nil
}

func (c *extractConnector) Driver() driver.Driver { return nil }

// newHTTPWriteConnector creates a libsql HTTP connector for writes to the Turso primary.
func newHTTPWriteConnector(remoteURL, authToken string) (driver.Connector, error) {
	connectorOptions := make([]libsql.Option, 0, 1)
	if authToken != "" {
		connectorOptions = append(connectorOptions, libsql.WithAuthToken(authToken))
	}

	connector, err := libsql.NewConnector(remoteURL, connectorOptions...)
	if err != nil {
		return nil, fmt.Errorf("libsql write connector: %w", err)
	}

	// Wrap to downgrade tx isolation (libsql doesn't support non-default levels)
	return libsqlTxCompatConnector{base: connector}, nil
}

// pullLoop periodically pulls from remote to keep the local read replica fresh.
// Also pulls immediately when signaled after a write.
func (t *TursoSyncDB) pullLoop() {
	ticker := time.NewTicker(pullInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			// Periodic pull
		case <-pullCh:
			// Triggered after a write
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		if _, err := t.Pull(ctx); err != nil {
			slog.Warn("turso: pull failed", "error", err)
		}
		cancel()
	}
}

// ── Signal mechanism for pull-after-write ────────────────────────────────

var pullCh chan struct{}

// SignalPull requests an immediate pull on the read replica.
// Called after writes to ensure read-after-write consistency.
// Non-blocking, coalesces multiple signals. No-op if turso+sync isn't active.
func SignalPull() {
	if pullCh == nil {
		return
	}
	select {
	case pullCh <- struct{}{}:
	default:
	}
}

// SignalPush triggers a pull to refresh the local replica after remote writes.
// Kept as the public API — callers signal "I wrote something", and the
// implementation pulls from remote to update the local read cache.
func SignalPush() {
	SignalPull()
}

func initPullChannel() {
	if pullCh == nil {
		pullCh = make(chan struct{}, 1)
	}
}

func openTursoSyncForFGA(connStr string) (*TursoSyncDB, error) {
	return openTursoSync(connStr)
}

func dirOf(path string) string {
	for i := len(path) - 1; i >= 0; i-- {
		if path[i] == '/' {
			return path[:i]
		}
	}
	return "."
}

// ── Split read/write SQL wrapper ─────────────────────────────────────────
// splitRWConnector routes queries to read or write backends.
// SELECT/PRAGMA → read (local replica), everything else → write (remote).

type splitRWConnector struct {
	readBase  driver.Connector
	writeBase driver.Connector
	onWrite   func() // called after write operations
}

func (c *splitRWConnector) Connect(ctx context.Context) (driver.Conn, error) {
	readConn, err := c.readBase.Connect(ctx)
	if err != nil {
		return nil, fmt.Errorf("read connect: %w", err)
	}
	writeConn, err := c.writeBase.Connect(ctx)
	if err != nil {
		return nil, fmt.Errorf("write connect: %w", err)
	}
	return &splitRWConn{read: readConn, write: writeConn, onWrite: c.onWrite}, nil
}

func (c *splitRWConnector) Driver() driver.Driver { return nil }

type splitRWConn struct {
	read    driver.Conn
	write   driver.Conn
	onWrite func()
}

func (c *splitRWConn) Prepare(query string) (driver.Stmt, error) {
	if isReadQuery(query) {
		return c.read.Prepare(query)
	}
	return c.write.Prepare(query)
}

func (c *splitRWConn) Close() error {
	e1 := c.read.Close()
	e2 := c.write.Close()
	if e1 != nil {
		return e1
	}
	return e2
}

func (c *splitRWConn) Begin() (driver.Tx, error) {
	// Transactions go to write path (they may contain writes)
	return c.write.Begin() //nolint:staticcheck
}

func (c *splitRWConn) BeginTx(ctx context.Context, opts driver.TxOptions) (driver.Tx, error) {
	if bt, ok := c.write.(driver.ConnBeginTx); ok {
		tx, err := bt.BeginTx(ctx, opts)
		if err != nil {
			return nil, err
		}
		return &pullAfterCommitTx{Tx: tx, onWrite: c.onWrite}, nil
	}
	tx, err := c.write.Begin() //nolint:staticcheck
	if err != nil {
		return nil, err
	}
	return &pullAfterCommitTx{Tx: tx, onWrite: c.onWrite}, nil
}

func (c *splitRWConn) ExecContext(ctx context.Context, query string, args []driver.NamedValue) (driver.Result, error) {
	if isReadQuery(query) {
		if e, ok := c.read.(driver.ExecerContext); ok {
			return e.ExecContext(ctx, query, args)
		}
		return nil, driver.ErrSkip
	}
	if e, ok := c.write.(driver.ExecerContext); ok {
		result, err := e.ExecContext(ctx, query, args)
		if err == nil && c.onWrite != nil {
			c.onWrite()
		}
		return result, err
	}
	return nil, driver.ErrSkip
}

func (c *splitRWConn) QueryContext(ctx context.Context, query string, args []driver.NamedValue) (driver.Rows, error) {
	// Reads go to local replica
	if q, ok := c.read.(driver.QueryerContext); ok {
		return q.QueryContext(ctx, query, args)
	}
	return nil, driver.ErrSkip
}

func (c *splitRWConn) Ping(ctx context.Context) error {
	if p, ok := c.read.(driver.Pinger); ok {
		return p.Ping(ctx)
	}
	return nil
}

func (c *splitRWConn) PrepareContext(ctx context.Context, query string) (driver.Stmt, error) {
	if isReadQuery(query) {
		if p, ok := c.read.(driver.ConnPrepareContext); ok {
			return p.PrepareContext(ctx, query)
		}
		return c.read.Prepare(query)
	}
	if p, ok := c.write.(driver.ConnPrepareContext); ok {
		return p.PrepareContext(ctx, query)
	}
	return c.write.Prepare(query)
}

func (c *splitRWConn) CheckNamedValue(value *driver.NamedValue) error {
	// Try write first (more permissive), fall back to read
	if checker, ok := c.write.(driver.NamedValueChecker); ok {
		return checker.CheckNamedValue(value)
	}
	if checker, ok := c.read.(driver.NamedValueChecker); ok {
		return checker.CheckNamedValue(value)
	}
	return driver.ErrSkip
}

func (c *splitRWConn) ResetSession(ctx context.Context) error {
	if r, ok := c.read.(driver.SessionResetter); ok {
		_ = r.ResetSession(ctx)
	}
	if r, ok := c.write.(driver.SessionResetter); ok {
		_ = r.ResetSession(ctx)
	}
	return nil
}

func (c *splitRWConn) IsValid() bool {
	readOK := true
	writeOK := true
	if v, ok := c.read.(driver.Validator); ok {
		readOK = v.IsValid()
	}
	if v, ok := c.write.(driver.Validator); ok {
		writeOK = v.IsValid()
	}
	return readOK && writeOK
}

// pullAfterCommitTx wraps a write transaction and triggers a pull after commit.
type pullAfterCommitTx struct {
	driver.Tx
	onWrite func()
}

func (tx *pullAfterCommitTx) Commit() error {
	if err := tx.Tx.Commit(); err != nil {
		return err
	}
	if tx.onWrite != nil {
		tx.onWrite()
	}
	return nil
}

// isReadQuery returns true for queries that only read data.
func isReadQuery(query string) bool {
	q := strings.TrimSpace(strings.ToUpper(query))
	return strings.HasPrefix(q, "SELECT") ||
		strings.HasPrefix(q, "PRAGMA") ||
		strings.HasPrefix(q, "EXPLAIN") ||
		strings.HasPrefix(q, "WITH") // CTEs are reads
}
