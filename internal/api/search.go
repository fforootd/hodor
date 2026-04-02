package api

import (
	"database/sql"
	"encoding/json"
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
// Queries use "?" for instance_id as the FIRST arg, then pattern args + limit.
// The schema search is global (no instance_id scoping).
func searchDefs() []searchDef {
	return []searchDef{
		{
			resourceType: "user",
			query: `SELECT id, identifier, display_name, state FROM users
				WHERE instance_id = ? AND (identifier LIKE ? OR display_name LIKE ?)
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
			query:        `SELECT id, name FROM orgs WHERE instance_id = ? AND name LIKE ? ORDER BY name LIMIT ?`,
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
			query:        `SELECT id, event_type, created_at FROM events WHERE instance_id = ? AND event_type LIKE ? ORDER BY id DESC LIMIT ?`,
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
			query: `SELECT id, name, protocol, COALESCE(metadata,'{}') FROM providers
				WHERE instance_id = ? AND name LIKE ?
				ORDER BY name LIMIT ?`,
			scan: func(rows *sql.Rows) (SearchResult, error) {
				var id, name, protocol, metadataJSON string
				if err := rows.Scan(&id, &name, &protocol, &metadataJSON); err != nil {
					return SearchResult{}, err
				}
				kind := ""
				var metadata map[string]any
				if err := json.Unmarshal([]byte(metadataJSON), &metadata); err == nil {
					if v, ok := metadata["kind"].(string); ok {
						kind = v
					}
				}
				subtitle := protocol
				if kind != "" {
					subtitle += " · " + kind
				}
				return SearchResult{ResourceType: "provider", ID: id, Title: name, Subtitle: subtitle}, nil
			},
		},
	}
}

// searchResource runs a single search definition against the database.
func (a *API) searchResource(r *http.Request, def searchDef, pattern string, limit int) []SearchResult {
	scoped := a.db.Scoped(r.Context())

	// Determine arg count. Schema queries have no instance_id scoping.
	var args []any
	argCount := strings.Count(def.query, "?")
	if def.resourceType == "schema" {
		// Schema is global — no instance_id arg.
		switch argCount {
		case 3:
			args = []any{pattern, pattern, limit}
		default:
			args = []any{pattern, limit}
		}
	} else {
		// Instance-scoped: first arg is always instance_id.
		switch argCount {
		case 4:
			args = []any{scoped.InstanceID(), pattern, pattern, limit}
		case 3:
			args = []any{scoped.InstanceID(), pattern, limit}
		default:
			args = []any{scoped.InstanceID(), pattern, limit}
		}
	}

	rows, err := scoped.QueryContext(r.Context(), scoped.Rebind(def.query), args...)
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
