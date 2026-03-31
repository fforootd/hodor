package database

import (
	"context"
	"database/sql"
	"database/sql/driver"
	"testing"
)

func TestLibsqlTxCompatConnectorDowngradesIsolation(t *testing.T) {
	baseConn := &fakeLibSQLConn{}
	connector := libsqlTxCompatConnector{base: fakeLibSQLConnector{conn: baseConn}}

	conn, err := connector.Connect(context.Background())
	if err != nil {
		t.Fatalf("Connect() error = %v", err)
	}

	beginTxConn, ok := conn.(driver.ConnBeginTx)
	if !ok {
		t.Fatal("wrapped connection does not implement driver.ConnBeginTx")
	}

	_, err = beginTxConn.BeginTx(context.Background(), driver.TxOptions{
		Isolation: driver.IsolationLevel(sql.LevelReadCommitted),
	})
	if err != nil {
		t.Fatalf("BeginTx() error = %v", err)
	}

	if len(baseConn.beginTxCalls) != 1 {
		t.Fatalf("expected 1 BeginTx call, got %d", len(baseConn.beginTxCalls))
	}
	if got := baseConn.beginTxCalls[0].Isolation; got != driver.IsolationLevel(sql.LevelDefault) {
		t.Fatalf("BeginTx isolation = %v, want LevelDefault", got)
	}
}

func TestLibsqlTxCompatConnectorKeepsDefaultIsolation(t *testing.T) {
	baseConn := &fakeLibSQLConn{}
	connector := libsqlTxCompatConnector{base: fakeLibSQLConnector{conn: baseConn}}

	conn, err := connector.Connect(context.Background())
	if err != nil {
		t.Fatalf("Connect() error = %v", err)
	}

	beginTxConn, ok := conn.(driver.ConnBeginTx)
	if !ok {
		t.Fatal("wrapped connection does not implement driver.ConnBeginTx")
	}

	_, err = beginTxConn.BeginTx(context.Background(), driver.TxOptions{})
	if err != nil {
		t.Fatalf("BeginTx() error = %v", err)
	}

	if len(baseConn.beginTxCalls) != 1 {
		t.Fatalf("expected 1 BeginTx call, got %d", len(baseConn.beginTxCalls))
	}
	if got := baseConn.beginTxCalls[0].Isolation; got != driver.IsolationLevel(sql.LevelDefault) {
		t.Fatalf("BeginTx isolation = %v, want LevelDefault", got)
	}
}

type fakeLibSQLConnector struct {
	conn driver.Conn
}

func (c fakeLibSQLConnector) Connect(context.Context) (driver.Conn, error) {
	return c.conn, nil
}

func (fakeLibSQLConnector) Driver() driver.Driver {
	return fakeLibSQLDriver{}
}

type fakeLibSQLDriver struct{}

func (fakeLibSQLDriver) Open(string) (driver.Conn, error) {
	return nil, nil
}

type fakeLibSQLConn struct {
	beginTxCalls []driver.TxOptions
}

func (c *fakeLibSQLConn) Prepare(string) (driver.Stmt, error) {
	return nil, driver.ErrSkip
}

func (c *fakeLibSQLConn) Close() error {
	return nil
}

func (c *fakeLibSQLConn) Begin() (driver.Tx, error) {
	return fakeLibSQLTx{}, nil
}

func (c *fakeLibSQLConn) BeginTx(_ context.Context, opts driver.TxOptions) (driver.Tx, error) {
	c.beginTxCalls = append(c.beginTxCalls, opts)
	return fakeLibSQLTx{}, nil
}

type fakeLibSQLTx struct{}

func (fakeLibSQLTx) Commit() error {
	return nil
}

func (fakeLibSQLTx) Rollback() error {
	return nil
}
