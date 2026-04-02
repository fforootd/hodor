package api

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/fga"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
)

// timeNow returns the current time formatted as RFC3339 (consistent across handlers).
func timeNow() string {
	return time.Now().UTC().Format(time.RFC3339)
}

// ── Request helpers ─────────────────────────────────────────────────────────

// decodeBody reads and decodes the JSON request body into T.
// Returns the decoded value and true on success; writes a 400 error and
// returns false on failure.
func decodeBody[T any](w http.ResponseWriter, r *http.Request) (T, bool) {
	var v T
	dec := json.NewDecoder(http.MaxBytesReader(w, r.Body, 1<<20)) // 1 MiB
	dec.DisallowUnknownFields()
	if err := dec.Decode(&v); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return v, false
	}
	if err := dec.Decode(&struct{}{}); err != io.EOF {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return v, false
	}
	return v, true
}

// requireID extracts and validates a path parameter by name.
// Returns the value and true on success; writes a 400 error on failure.
func requireID(w http.ResponseWriter, r *http.Request, name string) (string, bool) {
	v := r.PathValue(name)
	if v == "" {
		httputil.WriteError(w, http.StatusBadRequest, "missing "+name)
		return "", false
	}
	return v, true
}

// ── Transaction helpers ─────────────────────────────────────────────────────

// beginTx opens a scoped database transaction with standard error handling.
// Returns the ScopedTx and true on success; writes a 500 error on failure.
func (a *API) beginTx(w http.ResponseWriter, r *http.Request) (*database.ScopedTx, bool) {
	scoped := a.db.Scoped(r.Context())
	stx, err := scoped.BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return nil, false
	}
	return stx, true
}

// commitTx commits a scoped transaction with standard error handling.
// Returns true on success; writes a 500 error on failure.
func commitTx(w http.ResponseWriter, stx *database.ScopedTx) bool {
	if err := stx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return false
	}
	return true
}

// ── FGA helpers ─────────────────────────────────────────────────────────────

// fgaSync runs an FGA operation synchronously if the service is available.
// On failure it logs the error, writes a 500 response, and returns false.
// Use this only for operations where FGA consistency is required before responding
// (e.g. membership removals). For creates, prefer fgaAsync.
func fgaSync(w http.ResponseWriter, label string, fn func(context.Context, *fga.Service) error, ctx context.Context) bool {
	svc := FGAService
	if svc == nil {
		return true
	}
	if err := fn(ctx, svc); err != nil {
		logging.Printf("[fga] %s: %v", label, err)
		httputil.WriteError(w, http.StatusInternalServerError, "authorization sync failed")
		return false
	}
	return true
}

// fgaAsync runs fn in a background goroutine with a 2-second timeout.
// Used for FGA writes that are best-effort — the DB is the source of truth.
// Callers should capture svc := FGAService and guard nil themselves.
func fgaAsync(label string, fn func()) {
	go func() {
		done := make(chan struct{})
		go func() {
			defer close(done)
			fn()
		}()
		select {
		case <-done:
		case <-time.After(2 * time.Second):
			logging.Printf("[fga] async %s timed out after 2s (non-blocking)", label)
		}
	}()
}

// ── Pagination helpers ──────────────────────────────────────────────────────

// parsePagination extracts limit and cursor from query parameters.
// Moved here from group_project.go to be shared across all list handlers.
func parsePagination(r *http.Request) (limit int, cursor string) {
	limit = 50
	if l := r.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 200 {
			limit = n
		}
	}
	cursor = r.URL.Query().Get("cursor")
	return
}

// ── Identity helpers ────────────────────────────────────────────────────────

// creatorFromRequest extracts the actor ID from the request headers,
// falling back to "admin" for bootstrap/wizard contexts.
func creatorFromRequest(r *http.Request) string {
	if id := r.Header.Get("X-Identity-Id"); id != "" {
		return id
	}
	return "admin"
}

// ── JSON helpers ────────────────────────────────────────────────────────────

// marshalJSON marshals v to a JSON string, returning "{}" on nil or error.
func marshalJSON(v any) string {
	if v == nil {
		return "{}"
	}
	b, err := json.Marshal(v)
	if err != nil {
		return "{}"
	}
	return string(b)
}

func (a *API) bindQuery(query string) string {
	return a.db.Rebind(query)
}

// ── Patch builder (dynamic UPDATE) ──────────────────────────────────────────

// patchBuilder accumulates SET clauses for a partial UPDATE statement.
// Usage:
//
//	p := newPatch()
//	p.Set("name", req.Name)            // only appends if non-empty
//	p.SetJSON("metadata", req.Metadata) // marshals to JSON string
//	query, args := p.Build("orgs", orgID, instanceID)
//	result, err := db.ExecContext(ctx, query, args...)
type patchBuilder struct {
	clauses []string
	args    []any
	now     string
	dialect string
}

func newPatch(dialect ...string) *patchBuilder {
	now := timeNow()
	patchDialect := "sqlite"
	if len(dialect) > 0 && strings.TrimSpace(dialect[0]) != "" {
		patchDialect = strings.TrimSpace(dialect[0])
	}
	return &patchBuilder{
		clauses: []string{"updated_at = ?"},
		args:    []any{now},
		now:     now,
		dialect: patchDialect,
	}
}

// Set adds a SET clause if val is non-empty (non-zero for strings).
func (p *patchBuilder) Set(col, val string) {
	if val != "" {
		p.clauses = append(p.clauses, col+" = ?")
		p.args = append(p.args, val)
	}
}

// SetAny adds a SET clause if val is non-nil.
func (p *patchBuilder) SetAny(col string, val any) {
	if val != nil {
		p.clauses = append(p.clauses, col+" = ?")
		p.args = append(p.args, val)
	}
}

// SetJSON marshals val to JSON and adds a SET clause if val is non-nil.
func (p *patchBuilder) SetJSON(col string, val any) {
	if val != nil {
		p.clauses = append(p.clauses, col+" = ?")
		p.args = append(p.args, marshalJSON(val))
	}
}

// SetInt adds a SET clause for an int value (always applied).
func (p *patchBuilder) SetInt(col string, val int) {
	p.clauses = append(p.clauses, col+" = ?")
	p.args = append(p.args, val)
}

// Build returns the full UPDATE query and args (including the trailing WHERE id = ? AND instance_id = ?).
func (p *patchBuilder) Build(table, id, instanceID string) (string, []any) {
	p.args = append(p.args, id, instanceID)
	query := "UPDATE " + table + " SET " + strings.Join(p.clauses, ", ") + " WHERE id = ? AND instance_id = ?"
	return bindQueryForDialect(query, p.dialect), p.args
}

func bindQueryForDialect(query, dialect string) string {
	return database.RebindPlaceholders(query, dialect)
}
