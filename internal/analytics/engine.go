// Package analytics provides the OLAP query layer for Zitadel.
//
// Architecture (ADR-009):
//
//	Default: queries OLTP database directly (SQLite or Postgres)
//	No embedded DuckDB, no CGO. Pure Go.
//
//	Mutations (entity.*, schema.*) → always OLTP
//	Observations (auth.*, api.*, token.*) → configurable backend (default: OLTP)
//
//	Console UI sends SQL to POST /v1/analytics/query → routed to backend.
package analytics

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
)

// Backend is the pluggable analytics query interface.
// The default implementation (OLTPBackend) queries the same database
// as the OLTP tables. Customers can swap to ClickHouse, etc.
type Backend interface {
	Query(ctx context.Context, sql string, limit int) (*QueryResult, error)
	Tables(ctx context.Context) ([]TableInfo, error)
	Close() error
}

// QueryResult is the response from an analytics query.
type QueryResult struct {
	Columns     []string        `json:"columns"`
	ColumnTypes []string        `json:"column_types"`
	Rows        [][]interface{} `json:"rows"`
	RowCount    int             `json:"row_count"`
	ExecutionMs int64           `json:"execution_ms"`
	Error       string          `json:"error,omitempty"`
}

// TableInfo describes a queryable table.
type TableInfo struct {
	Name     string   `json:"name"`
	Columns  []Column `json:"columns"`
	RowCount int64    `json:"row_count"`
}

// Column describes a column.
type Column struct {
	Name string   `json:"name"`
	Type string   `json:"type"`
	Ref  *RefInfo `json:"ref,omitempty"`
}

// RefInfo describes a foreign key relationship to another entity or table.
// Mirrors x-ref annotations from the JSON schemas.
type RefInfo struct {
	Resource string `json:"resource"`
	Display  string `json:"display,omitempty"`
	Path     string `json:"path,omitempty"`
	Inverse  string `json:"inverse,omitempty"`
}

// OLTPBackend queries the OLTP database directly (SQLite or Postgres).
// This is the default backend — zero config, no extra dependencies.
type OLTPBackend struct {
	db      *sql.DB
	dialect string // "sqlite" or "postgres"
}

// NewOLTPBackend creates an analytics backend that queries the OLTP database.
func NewOLTPBackend(db *sql.DB, dialect string) *OLTPBackend {
	logging.Printf("[analytics] OLTP backend ready (dialect=%s)", dialect)
	return &OLTPBackend{db: db, dialect: dialect}
}

func (b *OLTPBackend) Close() error { return nil }

// Query executes a read-only SQL query against the OLTP database.
func (b *OLTPBackend) Query(ctx context.Context, rawSQL string, limit int) (*QueryResult, error) {
	start := time.Now()

	// Sanitize: only allow SELECT/WITH queries.
	normalized := strings.TrimSpace(strings.ToUpper(rawSQL))
	if !strings.HasPrefix(normalized, "SELECT") && !strings.HasPrefix(normalized, "WITH") {
		return nil, fmt.Errorf("only SELECT and WITH queries are allowed")
	}
	for _, kw := range []string{"DROP ", "DELETE ", "INSERT ", "UPDATE ", "CREATE ", "ALTER ", "TRUNCATE "} {
		if strings.Contains(normalized, kw) {
			return nil, fmt.Errorf("DDL/DML statements are not allowed")
		}
	}

	// Inject LIMIT if missing.
	if limit > 0 && !strings.Contains(normalized, "LIMIT ") {
		rawSQL = fmt.Sprintf("%s LIMIT %d", strings.TrimRight(rawSQL, ";"), limit)
	}

	rows, err := b.db.QueryContext(ctx, rawSQL)
	if err != nil {
		return nil, fmt.Errorf("query: %w", err)
	}
	defer rows.Close()

	colTypes, err := rows.ColumnTypes()
	if err != nil {
		return nil, fmt.Errorf("column types: %w", err)
	}

	columns := make([]string, len(colTypes))
	typeNames := make([]string, len(colTypes))
	for i, ct := range colTypes {
		columns[i] = ct.Name()
		typeNames[i] = ct.DatabaseTypeName()
	}

	var resultRows [][]interface{}
	for rows.Next() {
		vals := make([]interface{}, len(columns))
		ptrs := make([]interface{}, len(columns))
		for i := range vals {
			ptrs[i] = &vals[i]
		}
		if err := rows.Scan(ptrs...); err != nil {
			return nil, fmt.Errorf("scan: %w", err)
		}
		for i, v := range vals {
			if b, ok := v.([]byte); ok {
				vals[i] = string(b)
			}
		}
		resultRows = append(resultRows, vals)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("rows: %w", err)
	}
	if resultRows == nil {
		resultRows = [][]interface{}{}
	}

	return &QueryResult{
		Columns:     columns,
		ColumnTypes: typeNames,
		Rows:        resultRows,
		RowCount:    len(resultRows),
		ExecutionMs: time.Since(start).Milliseconds(),
	}, nil
}

// Tables returns metadata about queryable tables.
func (b *OLTPBackend) Tables(ctx context.Context) ([]TableInfo, error) {
	// The tables we expose for analytics queries.
	analyticsTable := []string{"events", "entities", "sessions"}
	tables := make([]TableInfo, 0, len(analyticsTable))

	for _, name := range analyticsTable {
		info := TableInfo{Name: name}

		// Row count.
		var count int64
		if err := b.db.QueryRowContext(ctx, fmt.Sprintf("SELECT COUNT(*) FROM %s", name)).Scan(&count); err == nil {
			info.RowCount = count
		}

		// Column info.
		info.Columns = b.getColumns(ctx, name)
		tables = append(tables, info)
	}

	return tables, nil
}

func (b *OLTPBackend) getColumns(ctx context.Context, table string) []Column {
	var query string
	if b.dialect == "postgres" {
		query = fmt.Sprintf(`SELECT column_name, data_type FROM information_schema.columns WHERE table_name = '%s' ORDER BY ordinal_position`, table)
	} else {
		query = fmt.Sprintf("PRAGMA table_info('%s')", table)
	}

	rows, err := b.db.QueryContext(ctx, query)
	if err != nil {
		return nil
	}
	defer rows.Close()

	var cols []Column
	if b.dialect == "postgres" {
		for rows.Next() {
			var c Column
			if err := rows.Scan(&c.Name, &c.Type); err == nil {
				cols = append(cols, c)
			}
		}
	} else {
		// SQLite PRAGMA table_info returns: cid, name, type, notnull, dflt_value, pk
		for rows.Next() {
			var cid int
			var c Column
			var notnull int
			var dflt sql.NullString
			var pk int
			if err := rows.Scan(&cid, &c.Name, &c.Type, &notnull, &dflt, &pk); err == nil {
				cols = append(cols, c)
			}
		}
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return cols
}

// Engine wraps a Backend and exposes HTTP handlers.
type Engine struct {
	backend Backend
}

// New creates a new analytics Engine.
func New(backend Backend) *Engine {
	return &Engine{backend: backend}
}

// RegisterRoutes mounts analytics API endpoints.
func (e *Engine) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("POST /v1/analytics/query", e.handleQuery)
	mux.HandleFunc("GET /v1/analytics/tables", e.handleTables)
	mux.HandleFunc("GET /v1/analytics/schema", e.handleSchema)
}

func (e *Engine) handleQuery(w http.ResponseWriter, r *http.Request) {
	var req struct {
		SQL   string `json:"sql"`
		Limit int    `json:"limit,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteJSON(w, http.StatusBadRequest, QueryResult{Error: "invalid JSON body"})
		return
	}
	if req.SQL == "" {
		httputil.WriteJSON(w, http.StatusBadRequest, QueryResult{Error: "sql field is required"})
		return
	}
	if req.Limit == 0 {
		req.Limit = 1000
	}

	result, err := e.backend.Query(r.Context(), req.SQL, req.Limit)
	if err != nil {
		httputil.WriteJSON(w, http.StatusBadRequest, QueryResult{Error: err.Error()})
		return
	}
	httputil.WriteJSON(w, http.StatusOK, result)
}

func (e *Engine) handleTables(w http.ResponseWriter, r *http.Request) {
	tables, err := e.backend.Tables(r.Context())
	if err != nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	httputil.WriteJSON(w, http.StatusOK, map[string]interface{}{"tables": tables})
}

// schemaRefs defines foreign key relationship metadata per table+column.
// This mirrors the x-ref annotations declared in the JSON schemas under
// internal/schema/schemas/*.json. Kept here as a static map to avoid
// parsing JSON at runtime.
var schemaRefs = map[string]map[string]*RefInfo{
	"events": {
		"actor_id":     {Resource: "entities", Display: "display_name", Path: "/console/s/{type}/{id}", Inverse: "events?actor_id={id}"},
		"aggregate_id": {Resource: "entities", Display: "identifier", Path: "/console/s/{type}/{id}"},
		"session_id":   {Resource: "sessions", Display: "id", Path: "/console/sessions"},
	},
	"sessions": {
		"entity_id": {Resource: "entities", Display: "display_name", Path: "/console/s/{type}/{id}", Inverse: "sessions?entity_id={id}"},
	},
}

func (e *Engine) handleSchema(w http.ResponseWriter, r *http.Request) {
	tables, err := e.backend.Tables(r.Context())
	if err != nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	schema := make(map[string]interface{})
	for _, t := range tables {
		tableRefs := schemaRefs[t.Name]
		cols := make([]map[string]interface{}, len(t.Columns))
		for i, c := range t.Columns {
			col := map[string]interface{}{"name": c.Name, "type": c.Type}
			if tableRefs != nil {
				if ref, ok := tableRefs[c.Name]; ok {
					col["ref"] = ref
				}
			}
			cols[i] = col
		}
		schema[t.Name] = map[string]interface{}{
			"columns":   cols,
			"row_count": t.RowCount,
		}
	}
	httputil.WriteJSON(w, http.StatusOK, schema)
}
