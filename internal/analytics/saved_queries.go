package analytics

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
)

// SavedQuery represents a persisted analytics query.
type SavedQuery struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	SQL         string `json:"sql"`
	CreatedBy   string `json:"created_by,omitempty"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

// RegisterSavedQueryRoutes mounts CRUD endpoints for saved queries.
func (e *Engine) RegisterSavedQueryRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/analytics/queries", e.handleListQueries)
	mux.HandleFunc("POST /v1/analytics/queries", e.handleCreateQuery)
	mux.HandleFunc("PUT /v1/analytics/queries/{id}", e.handleUpdateQuery)
	mux.HandleFunc("DELETE /v1/analytics/queries/{id}", e.handleDeleteQuery)
}

func (e *Engine) db() *sql.DB {
	if b, ok := e.backend.(*OLTPBackend); ok {
		return b.db
	}
	return nil
}

func (e *Engine) handleListQueries(w http.ResponseWriter, r *http.Request) {
	db := e.db()
	if db == nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": "saved queries not supported"})
		return
	}

	rows, err := db.QueryContext(r.Context(),
		`SELECT id, name, description, sql_text, created_by, created_at, updated_at
		 FROM saved_queries ORDER BY updated_at DESC`)
	if err != nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	defer rows.Close()

	var queries []SavedQuery
	for rows.Next() {
		var q SavedQuery
		if err := rows.Scan(&q.ID, &q.Name, &q.Description, &q.SQL, &q.CreatedBy, &q.CreatedAt, &q.UpdatedAt); err != nil {
			continue
		}
		queries = append(queries, q)
	}
	if queries == nil {
		queries = []SavedQuery{}
	}
	httputil.WriteJSON(w, http.StatusOK, map[string]any{"items": queries})
}

func (e *Engine) handleCreateQuery(w http.ResponseWriter, r *http.Request) {
	db := e.db()
	if db == nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": "saved queries not supported"})
		return
	}

	var req struct {
		Name        string `json:"name"`
		Description string `json:"description"`
		SQL         string `json:"sql"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid JSON"})
		return
	}
	if strings.TrimSpace(req.Name) == "" || strings.TrimSpace(req.SQL) == "" {
		httputil.WriteJSON(w, http.StatusBadRequest, map[string]string{"error": "name and sql are required"})
		return
	}

	id := fmt.Sprintf("sq_%d", time.Now().UnixNano())
	now := time.Now().UTC().Format("2006-01-02 15:04:05")

	_, err := db.ExecContext(r.Context(),
		`INSERT INTO saved_queries (id, name, description, sql_text, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		id, req.Name, req.Description, req.SQL, now, now)
	if err != nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}

	httputil.WriteJSON(w, http.StatusCreated, SavedQuery{
		ID:          id,
		Name:        req.Name,
		Description: req.Description,
		SQL:         req.SQL,
		CreatedAt:   now,
		UpdatedAt:   now,
	})
}

func (e *Engine) handleUpdateQuery(w http.ResponseWriter, r *http.Request) {
	db := e.db()
	if db == nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": "saved queries not supported"})
		return
	}

	id := r.PathValue("id")
	if id == "" {
		httputil.WriteJSON(w, http.StatusBadRequest, map[string]string{"error": "id is required"})
		return
	}

	var req struct {
		Name        string `json:"name"`
		Description string `json:"description"`
		SQL         string `json:"sql"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid JSON"})
		return
	}

	now := time.Now().UTC().Format("2006-01-02 15:04:05")
	result, err := db.ExecContext(r.Context(),
		`UPDATE saved_queries SET name = ?, description = ?, sql_text = ?, updated_at = ? WHERE id = ?`,
		req.Name, req.Description, req.SQL, now, id)
	if err != nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	affected, _ := result.RowsAffected()
	if affected == 0 {
		httputil.WriteJSON(w, http.StatusNotFound, map[string]string{"error": "query not found"})
		return
	}
	httputil.WriteJSON(w, http.StatusOK, SavedQuery{
		ID:          id,
		Name:        req.Name,
		Description: req.Description,
		SQL:         req.SQL,
		UpdatedAt:   now,
	})
}

func (e *Engine) handleDeleteQuery(w http.ResponseWriter, r *http.Request) {
	db := e.db()
	if db == nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": "saved queries not supported"})
		return
	}

	id := r.PathValue("id")
	result, err := db.ExecContext(r.Context(), `DELETE FROM saved_queries WHERE id = ?`, id)
	if err != nil {
		httputil.WriteJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	affected, _ := result.RowsAffected()
	if affected == 0 {
		httputil.WriteJSON(w, http.StatusNotFound, map[string]string{"error": "query not found"})
		return
	}
	w.WriteHeader(http.StatusNoContent)
}
