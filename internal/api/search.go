package api

import (
	"database/sql"
	"net/http"
	"strconv"
	"strings"

	"github.com/zitadel/zitadel/internal/httputil"
)

// SearchResult is a universal search result across all resource types.
type SearchResult struct {
	ResourceType string `json:"resource_type"`
	ID           string `json:"id"`
	Title        string `json:"title"`
	Subtitle     string `json:"subtitle,omitempty"`
}

// searchDef describes how to search a single resource type.
type searchDef struct {
	resourceType string
	query        string
	scan         func(*sql.Rows) (SearchResult, error)
}

// searchDefs returns all configured search definitions.
func searchDefs() []searchDef {
	return []searchDef{
		{
			resourceType: "user",
			query: `SELECT id, identifier, display_name, state FROM users
				WHERE identifier LIKE ? OR display_name LIKE ?
				ORDER BY id DESC LIMIT ?`,
			scan: func(rows *sql.Rows) (SearchResult, error) {
				var id, ident, state string
				var dn sql.NullString
				if err := rows.Scan(&id, &ident, &dn, &state); err != nil {
					return SearchResult{}, err
				}
				subtitle := state
				if dn.Valid && dn.String != "" {
					subtitle = dn.String + " · " + state
				}
				return SearchResult{ResourceType: "user", ID: id, Title: ident, Subtitle: subtitle}, nil
			},
		},
		{
			resourceType: "org",
			query:        `SELECT id, name FROM orgs WHERE name LIKE ? ORDER BY name LIMIT ?`,
			scan: func(rows *sql.Rows) (SearchResult, error) {
				var id, name string
				if err := rows.Scan(&id, &name); err != nil {
					return SearchResult{}, err
				}
				return SearchResult{ResourceType: "org", ID: id, Title: name, Subtitle: id}, nil
			},
		},
		{
			resourceType: "schema",
			query:        `SELECT id, type FROM schemas WHERE id LIKE ? OR type LIKE ? LIMIT ?`,
			scan: func(rows *sql.Rows) (SearchResult, error) {
				var id, t string
				if err := rows.Scan(&id, &t); err != nil {
					return SearchResult{}, err
				}
				return SearchResult{ResourceType: "schema", ID: id, Title: t, Subtitle: id}, nil
			},
		},
		{
			resourceType: "event",
			query:        `SELECT id, event_type, created_at FROM events WHERE event_type LIKE ? ORDER BY id DESC LIMIT ?`,
			scan: func(rows *sql.Rows) (SearchResult, error) {
				var id, evtType, createdAt string
				if err := rows.Scan(&id, &evtType, &createdAt); err != nil {
					return SearchResult{}, err
				}
				return SearchResult{ResourceType: "event", ID: id, Title: evtType, Subtitle: createdAt}, nil
			},
		},
		{
			resourceType: "provider",
			query: `SELECT id, name, protocol, template FROM providers
				WHERE name LIKE ?
				ORDER BY name LIMIT ?`,
			scan: func(rows *sql.Rows) (SearchResult, error) {
				var id, name, protocol, tmpl string
				if err := rows.Scan(&id, &name, &protocol, &tmpl); err != nil {
					return SearchResult{}, err
				}
				return SearchResult{ResourceType: "provider", ID: id, Title: name, Subtitle: protocol + " · " + tmpl}, nil
			},
		},
	}
}

// searchResource runs a single search definition against the database.
func (a *API) searchResource(r *http.Request, def searchDef, pattern string, limit int) []SearchResult {
	// Determine arg count: queries with 2 LIKE columns need pattern twice + limit.
	var args []any
	argCount := strings.Count(def.query, "?")
	switch argCount {
	case 3:
		args = []any{pattern, pattern, limit}
	case 2:
		args = []any{pattern, limit}
	default:
		args = []any{pattern, limit}
	}

	rows, err := a.db.SQL().QueryContext(r.Context(), def.query, args...)
	if err != nil {
		return nil
	}
	defer rows.Close()

	var results []SearchResult
	for rows.Next() {
		res, err := def.scan(rows)
		if err != nil {
			continue
		}
		results = append(results, res)
	}
	if rows.Err() != nil {
		return nil
	}
	return results
}

func (a *API) search(w http.ResponseWriter, r *http.Request) {
	q := strings.TrimSpace(r.URL.Query().Get("q"))
	if q == "" {
		httputil.WriteJSON(w, http.StatusOK, map[string]any{"results": []any{}, "query": ""})
		return
	}

	limit := 10
	if l := r.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 50 {
			limit = n
		}
	}

	pattern := "%" + q + "%"
	results := make([]SearchResult, 0, limit*5)
	for _, def := range searchDefs() {
		results = append(results, a.searchResource(r, def, pattern, limit)...)
	}

	// Deduplicate
	seen := map[string]bool{}
	var deduped []SearchResult
	for _, res := range results {
		key := res.ResourceType + ":" + res.ID
		if !seen[key] {
			seen[key] = true
			deduped = append(deduped, res)
		}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"results": deduped,
		"query":   q,
		"count":   len(deduped),
	})
}
