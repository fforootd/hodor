package d1driver

import (
	"bytes"
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"
	"time"
)

func TestQueryNormalizesJSONNumbersForBoolScan(t *testing.T) {
	t.Helper()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/query" {
			t.Fatalf("path = %s, want /query", r.URL.Path)
		}

		w.Header().Set("Content-Type", "application/json")
		if err := json.NewEncoder(w).Encode(d1Result{
			Success: true,
			Results: []map[string]interface{}{
				{
					"version_id": 1,
					"is_applied": 1,
				},
			},
			Meta: d1Meta{
				Columns: []string{"version_id", "is_applied"},
			},
		}); err != nil {
			t.Fatalf("encode response: %v", err)
		}
	}))
	defer server.Close()

	db, err := sql.Open("d1", server.URL)
	if err != nil {
		t.Fatalf("sql.Open: %v", err)
	}
	defer db.Close()

	row := db.QueryRow("SELECT version_id, is_applied FROM goose_db_version")

	var version int64
	var applied bool
	if err := row.Scan(&version, &applied); err != nil {
		t.Fatalf("row.Scan: %v", err)
	}

	if version != 1 {
		t.Fatalf("version = %d, want 1", version)
	}
	if !applied {
		t.Fatal("applied = false, want true")
	}
}

func TestExecSplitsMultiStatementDDL(t *testing.T) {
	t.Helper()

	var execCalls atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/exec" {
			t.Fatalf("path = %s, want /exec", r.URL.Path)
		}
		execCalls.Add(1)

		w.Header().Set("Content-Type", "application/json")
		if err := json.NewEncoder(w).Encode(d1Result{
			Success: true,
			Results: []map[string]interface{}{},
			Meta:    d1Meta{},
		}); err != nil {
			t.Fatalf("encode response: %v", err)
		}
	}))
	defer server.Close()

	db, err := sql.Open("d1", server.URL)
	if err != nil {
		t.Fatalf("sql.Open: %v", err)
	}
	defer db.Close()

	if _, err := db.Exec("CREATE TABLE foo (id INTEGER); CREATE TABLE bar (id INTEGER);"); err != nil {
		t.Fatalf("db.Exec: %v", err)
	}

	if got := execCalls.Load(); got != 2 {
		t.Fatalf("exec calls = %d, want 2", got)
	}
}

func TestQueryParsesSQLiteTimestampsForTimeScan(t *testing.T) {
	t.Helper()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/query" {
			t.Fatalf("path = %s, want /query", r.URL.Path)
		}

		w.Header().Set("Content-Type", "application/json")
		if err := json.NewEncoder(w).Encode(d1Result{
			Success: true,
			Results: []map[string]interface{}{
				{
					"id":         "01TEST",
					"created_at": "2026-03-30 23:48:14.973",
					"updated_at": "2026-03-30 23:48:14.973",
				},
			},
			Meta: d1Meta{
				Columns: []string{"id", "created_at", "updated_at"},
			},
		}); err != nil {
			t.Fatalf("encode response: %v", err)
		}
	}))
	defer server.Close()

	db, err := sql.Open("d1", server.URL)
	if err != nil {
		t.Fatalf("sql.Open: %v", err)
	}
	defer db.Close()

	row := db.QueryRow("SELECT id, created_at, updated_at FROM store WHERE id = ?", "01TEST")

	var id string
	var createdAt time.Time
	var updatedAt time.Time
	if err := row.Scan(&id, &createdAt, &updatedAt); err != nil {
		t.Fatalf("row.Scan: %v", err)
	}

	if id != "01TEST" {
		t.Fatalf("id = %q, want 01TEST", id)
	}

	want := time.Date(2026, time.March, 30, 23, 48, 14, 973000000, time.UTC)
	if !createdAt.Equal(want) {
		t.Fatalf("created_at = %s, want %s", createdAt.Format(time.RFC3339Nano), want.Format(time.RFC3339Nano))
	}
	if !updatedAt.Equal(want) {
		t.Fatalf("updated_at = %s, want %s", updatedAt.Format(time.RFC3339Nano), want.Format(time.RFC3339Nano))
	}
}

func TestExecEncodesByteParamsAsBlobPayload(t *testing.T) {
	t.Helper()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/exec" {
			t.Fatalf("path = %s, want /exec", r.URL.Path)
		}

		var payload map[string]interface{}
		if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
			t.Fatalf("decode request: %v", err)
		}

		params, ok := payload["params"].([]interface{})
		if !ok || len(params) != 1 {
			t.Fatalf("params = %#v, want single encoded blob param", payload["params"])
		}

		blob, ok := params[0].(map[string]interface{})
		if !ok {
			t.Fatalf("param = %#v, want object", params[0])
		}
		if blob["__d1_type"] != "blob" {
			t.Fatalf("__d1_type = %#v, want blob", blob["__d1_type"])
		}
		if blob["base64"] != base64.StdEncoding.EncodeToString([]byte{1, 2, 3}) {
			t.Fatalf("base64 = %#v, want AQID", blob["base64"])
		}

		w.Header().Set("Content-Type", "application/json")
		if err := json.NewEncoder(w).Encode(d1Result{
			Success: true,
			Results: []map[string]interface{}{},
			Meta:    d1Meta{},
		}); err != nil {
			t.Fatalf("encode response: %v", err)
		}
	}))
	defer server.Close()

	db, err := sql.Open("d1", server.URL)
	if err != nil {
		t.Fatalf("sql.Open: %v", err)
	}
	defer db.Close()

	if _, err := db.Exec("INSERT INTO authorization_model (serialized_protobuf) VALUES (?)", []byte{1, 2, 3}); err != nil {
		t.Fatalf("db.Exec: %v", err)
	}
}

func TestQueryNormalizesBlobArraysForByteScan(t *testing.T) {
	t.Helper()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/query" {
			t.Fatalf("path = %s, want /query", r.URL.Path)
		}

		w.Header().Set("Content-Type", "application/json")
		if err := json.NewEncoder(w).Encode(d1Result{
			Success: true,
			Results: []map[string]interface{}{
				{
					"serialized_protobuf": []int{1, 2, 3, 4},
				},
			},
			Meta: d1Meta{
				Columns: []string{"serialized_protobuf"},
			},
		}); err != nil {
			t.Fatalf("encode response: %v", err)
		}
	}))
	defer server.Close()

	db, err := sql.Open("d1", server.URL)
	if err != nil {
		t.Fatalf("sql.Open: %v", err)
	}
	defer db.Close()

	row := db.QueryRow("SELECT serialized_protobuf FROM authorization_model")

	var got []byte
	if err := row.Scan(&got); err != nil {
		t.Fatalf("row.Scan: %v", err)
	}

	if !bytes.Equal(got, []byte{1, 2, 3, 4}) {
		t.Fatalf("blob = %v, want [1 2 3 4]", got)
	}
}
