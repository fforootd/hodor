package database

import (
	"context"
	"database/sql/driver"
	"io"
	"testing"
)

func TestParseTursoSyncURL(t *testing.T) {
	remoteURL, localPath, partial := parseTursoSyncURL("turso+sync://example.turso.io?path=/tmp/edge.db&prefix_size=4096&segment_size=2048&prefetch=0")
	if remoteURL != "libsql://example.turso.io" {
		t.Fatalf("remoteURL = %q, want libsql://example.turso.io", remoteURL)
	}
	if localPath != "/tmp/edge.db" {
		t.Fatalf("localPath = %q, want /tmp/edge.db", localPath)
	}
	if partial.BootstrapStrategyPrefix != 4096 {
		t.Fatalf("BootstrapStrategyPrefix = %d, want 4096", partial.BootstrapStrategyPrefix)
	}
	if partial.SegmentSize != 2048 {
		t.Fatalf("SegmentSize = %d, want 2048", partial.SegmentSize)
	}
	if partial.Prefetch {
		t.Fatal("Prefetch = true, want false")
	}
}

func TestIsReadQuery(t *testing.T) {
	for _, query := range []string{"SELECT 1", " pragma table_info('users')", "WITH cte AS (SELECT 1) SELECT * FROM cte"} {
		if !isReadQuery(query) {
			t.Fatalf("query %q should be treated as read", query)
		}
	}
	for _, query := range []string{"INSERT INTO users VALUES (1)", "UPDATE users SET name = 'x'", "DELETE FROM users"} {
		if isReadQuery(query) {
			t.Fatalf("query %q should be treated as write", query)
		}
	}
}

func TestSplitRWConnRoutesReadAndWrite(t *testing.T) {
	readConn := &testDriverConn{}
	writeConn := &testDriverConn{}
	writeSignals := 0

	conn := &splitRWConn{
		read:  readConn,
		write: writeConn,
		onWrite: func() {
			writeSignals++
		},
	}

	if _, err := conn.ExecContext(context.Background(), "SELECT 1", nil); err != nil {
		t.Fatalf("exec read query: %v", err)
	}
	if len(readConn.execQueries) != 1 || len(writeConn.execQueries) != 0 {
		t.Fatalf("unexpected exec routing read=%d write=%d", len(readConn.execQueries), len(writeConn.execQueries))
	}
	if writeSignals != 0 {
		t.Fatalf("writeSignals = %d, want 0 after read", writeSignals)
	}

	if _, err := conn.ExecContext(context.Background(), "INSERT INTO users VALUES (?)", nil); err != nil {
		t.Fatalf("exec write query: %v", err)
	}
	if len(writeConn.execQueries) != 1 {
		t.Fatalf("len(write exec queries) = %d, want 1", len(writeConn.execQueries))
	}
	if writeSignals != 1 {
		t.Fatalf("writeSignals = %d, want 1 after write", writeSignals)
	}

	if _, err := conn.QueryContext(context.Background(), "SELECT id FROM users", nil); err != nil {
		t.Fatalf("query context: %v", err)
	}
	if len(readConn.queryQueries) != 1 {
		t.Fatalf("len(read query queries) = %d, want 1", len(readConn.queryQueries))
	}

	tx, err := conn.BeginTx(context.Background(), driver.TxOptions{})
	if err != nil {
		t.Fatalf("begin tx: %v", err)
	}
	if err := tx.Commit(); err != nil {
		t.Fatalf("commit tx: %v", err)
	}
	if writeSignals != 2 {
		t.Fatalf("writeSignals = %d, want 2 after commit", writeSignals)
	}
}

func TestSignalPullCoalesces(t *testing.T) {
	pullCh = make(chan struct{}, 1)
	SignalPull()
	SignalPull()
	if got := len(pullCh); got != 1 {
		t.Fatalf("len(pullCh) = %d, want 1", got)
	}
}

type testDriverConn struct {
	execQueries  []string
	queryQueries []string
}

func (c *testDriverConn) Prepare(query string) (driver.Stmt, error) { return testDriverStmt{}, nil }
func (c *testDriverConn) Close() error                              { return nil }
func (c *testDriverConn) Begin() (driver.Tx, error)                 { return testDriverTx{}, nil }
func (c *testDriverConn) BeginTx(context.Context, driver.TxOptions) (driver.Tx, error) {
	return testDriverTx{}, nil
}

func (c *testDriverConn) ExecContext(_ context.Context, query string, _ []driver.NamedValue) (driver.Result, error) {
	c.execQueries = append(c.execQueries, query)
	return driver.RowsAffected(1), nil
}

func (c *testDriverConn) QueryContext(_ context.Context, query string, _ []driver.NamedValue) (driver.Rows, error) {
	c.queryQueries = append(c.queryQueries, query)
	return testDriverRows{}, nil
}

type testDriverStmt struct{}

func (testDriverStmt) Close() error                               { return nil }
func (testDriverStmt) NumInput() int                              { return -1 }
func (testDriverStmt) Exec([]driver.Value) (driver.Result, error) { return driver.RowsAffected(1), nil }
func (testDriverStmt) Query([]driver.Value) (driver.Rows, error)  { return testDriverRows{}, nil }

type testDriverTx struct{}

func (testDriverTx) Commit() error   { return nil }
func (testDriverTx) Rollback() error { return nil }

type testDriverRows struct{}

func (testDriverRows) Columns() []string         { return []string{"id"} }
func (testDriverRows) Close() error              { return nil }
func (testDriverRows) Next([]driver.Value) error { return io.EOF }
