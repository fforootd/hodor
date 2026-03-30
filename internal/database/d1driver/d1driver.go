// Package d1driver implements a database/sql driver that sends queries to
// a Cloudflare D1 database via an HTTP proxy.
//
// In a Cloudflare Containers deployment, the container can't access D1
// bindings directly — those are only available in the Workers runtime.
// Instead, the Worker exposes an outbound HTTP handler (via outboundByHost)
// that intercepts HTTP calls from the container and forwards them to D1.
//
// The container makes HTTP requests to a virtual hostname (e.g., http://d1.local/query),
// and the Worker's outbound handler translates them into env.DB.prepare().bind().run()
// calls using the real D1 binding.
//
// Wire protocol (HTTP):
//
//	POST http://d1.local/query   → D1PreparedStatement.run()  (returns rows)
//	POST http://d1.local/exec    → D1PreparedStatement.run()  (returns changes)
//
// Request body:
//
//	{ "sql": "SELECT ...", "params": [1, "foo"] }
//
// Response body mirrors D1Result:
//
//	{
//	  "success": true,
//	  "results": [ {"id": 1, "name": "foo"} ],
//	  "meta": {
//	    "changes": 0,
//	    "last_row_id": 0,
//	    "rows_read": 1,
//	    "duration": 0.25
//	  }
//	}
//
// Usage:
//
//	import _ "github.com/zitadel/zitadel/internal/database/d1driver"
//	db, err := sql.Open("d1", "http://d1.local")
//
// Or via the database package's URL scheme detection:
//
//	db, err := database.Open("d1://d1.local")
package d1driver

import (
	"bytes"
	"database/sql"
	"database/sql/driver"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
)

func init() {
	sql.Register("d1", &Driver{})
}

// ── Driver ─────────────────────────────────────────────────────────────────

// Driver implements database/sql/driver.Driver.
type Driver struct{}

func (d *Driver) Open(name string) (driver.Conn, error) {
	// name is the proxy base URL, e.g., "http://d1.local"
	baseURL := strings.TrimRight(name, "/")
	return &conn{baseURL: baseURL, client: &http.Client{}}, nil
}

// ── Conn ───────────────────────────────────────────────────────────────────

type conn struct {
	baseURL string
	client  *http.Client
	closed  bool
	mu      sync.Mutex
}

func (c *conn) Prepare(query string) (driver.Stmt, error) {
	return &stmt{conn: c, query: query}, nil
}

func (c *conn) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.closed = true
	return nil
}

func (c *conn) Begin() (driver.Tx, error) {
	// D1 does not support multi-statement transactions.
	// Return a no-op tx that executes statements immediately.
	return &noopTx{}, nil
}

// post sends a SQL query to the D1 proxy and returns the parsed response.
func (c *conn) post(endpoint string, query string, args []driver.Value) (*d1Result, error) {
	params := make([]interface{}, len(args))
	for i, a := range args {
		params[i] = a
	}

	body, err := json.Marshal(d1Request{SQL: query, Params: params})
	if err != nil {
		return nil, fmt.Errorf("d1: marshal request: %w", err)
	}

	resp, err := c.client.Post(c.baseURL+endpoint, "application/json", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("d1: http post to %s%s: %w", c.baseURL, endpoint, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		b, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("d1: proxy returned %d: %s", resp.StatusCode, string(b))
	}

	var result d1Result
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("d1: decode response: %w", err)
	}

	if !result.Success {
		return nil, fmt.Errorf("d1: query error: %s", result.Error)
	}

	return &result, nil
}

// ── Stmt ───────────────────────────────────────────────────────────────────

type stmt struct {
	conn  *conn
	query string
}

func (s *stmt) Close() error { return nil }

func (s *stmt) NumInput() int { return -1 } // variable number of args

func (s *stmt) Exec(args []driver.Value) (driver.Result, error) {
	resp, err := s.conn.post("/exec", s.query, args)
	if err != nil {
		return nil, err
	}
	return &execResult{
		lastInsertID: resp.Meta.LastRowID,
		rowsAffected: resp.Meta.Changes,
	}, nil
}

func (s *stmt) Query(args []driver.Value) (driver.Rows, error) {
	resp, err := s.conn.post("/query", s.query, args)
	if err != nil {
		return nil, err
	}

	// Extract column names from the first result row.
	// If results are empty, we get columns from meta (which the proxy provides).
	var columns []string
	if len(resp.Meta.Columns) > 0 {
		columns = resp.Meta.Columns
	} else if len(resp.Results) > 0 {
		// Derive columns from the first row's keys.
		for k := range resp.Results[0] {
			columns = append(columns, k)
		}
	}

	return &queryRows{
		columns: columns,
		data:    resp.Results,
		pos:     0,
	}, nil
}

// ── Result ─────────────────────────────────────────────────────────────────

type execResult struct {
	lastInsertID int64
	rowsAffected int64
}

func (r *execResult) LastInsertId() (int64, error) { return r.lastInsertID, nil }
func (r *execResult) RowsAffected() (int64, error) { return r.rowsAffected, nil }

// ── Rows ───────────────────────────────────────────────────────────────────

type queryRows struct {
	columns []string
	data    []map[string]interface{}
	pos     int
}

func (r *queryRows) Columns() []string { return r.columns }

func (r *queryRows) Close() error {
	r.pos = len(r.data)
	return nil
}

func (r *queryRows) Next(dest []driver.Value) error {
	if r.pos >= len(r.data) {
		return io.EOF
	}
	row := r.data[r.pos]
	for i, col := range r.columns {
		v, ok := row[col]
		if !ok {
			dest[i] = nil
		} else {
			dest[i] = v
		}
	}
	r.pos++
	return nil
}

// ── Tx (no-op) ─────────────────────────────────────────────────────────────
// D1 doesn't support multi-statement transactions. Each statement is auto-committed.

type noopTx struct{}

func (t *noopTx) Commit() error   { return nil }
func (t *noopTx) Rollback() error { return nil }

// ── Wire Protocol Types ────────────────────────────────────────────────────

// d1Request is the JSON body sent to the D1 proxy.
type d1Request struct {
	SQL    string        `json:"sql"`
	Params []interface{} `json:"params,omitempty"`
}

// d1Result mirrors the D1Result object from the Workers Binding API.
// See: https://developers.cloudflare.com/d1/worker-api/return-object/
type d1Result struct {
	Success bool                     `json:"success"`
	Error   string                   `json:"error,omitempty"`
	Results []map[string]interface{} `json:"results,omitempty"`
	Meta    d1Meta                   `json:"meta"`
}

// d1Meta mirrors the meta object from D1Result.
type d1Meta struct {
	// Columns is provided by our proxy (not part of the standard D1 meta)
	// to help the Go driver know column order for SELECT results.
	Columns []string `json:"columns,omitempty"`

	// Standard D1 meta fields.
	Changes   int64   `json:"changes"`
	LastRowID int64   `json:"last_row_id"`
	ChangedDB bool    `json:"changed_db"`
	RowsRead  int64   `json:"rows_read"`
	RowsWrite int64   `json:"rows_written"`
	Duration  float64 `json:"duration"`
}
