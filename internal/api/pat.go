package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"time"

	"github.com/zitadel/zitadel/internal/id"
)

// --- PAT types ---

type CreatePATRequest struct {
	EntityID int64    `json:"entity_id"`
	Name     string   `json:"name"`
	Scopes   []string `json:"scopes,omitempty"`
}

type CreatePATResponse struct {
	ID        int64    `json:"id,string"`
	Name      string   `json:"name"`
	EntityID  int64    `json:"entity_id,string"`
	Token     string   `json:"token"` // Only returned on creation — never again.
	Scopes    []string `json:"scopes"`
	CreatedAt string   `json:"created_at"`
}

type PATResponse struct {
	ID        int64    `json:"id,string"`
	Name      string   `json:"name"`
	EntityID  int64    `json:"entity_id,string"`
	Scopes    []string `json:"scopes"`
	LastUsed  *string  `json:"last_used,omitempty"`
	CreatedAt string   `json:"created_at"`
}

// RegisterPATRoutes mounts PAT management endpoints (all admin-only).
func (a *API) RegisterPATRoutes(mux *http.ServeMux) {
	mux.HandleFunc("POST /v1/pats", a.requireAdmin(a.createPAT))
	mux.HandleFunc("GET /v1/pats", a.requireAdmin(a.listPATs))
	mux.HandleFunc("DELETE /v1/pats/{id}", a.requireAdmin(a.revokePAT))
}

func (a *API) createPAT(w http.ResponseWriter, r *http.Request) {
	var req CreatePATRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.EntityID == 0 {
		writeError(w, http.StatusBadRequest, "entity_id is required")
		return
	}
	if req.Name == "" {
		writeError(w, http.StatusBadRequest, "name is required")
		return
	}

	// Default scopes to ["admin"] if not specified.
	if len(req.Scopes) == 0 {
		req.Scopes = []string{"admin"}
	}

	// Verify entity exists.
	var exists int
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT 1 FROM entities WHERE id = ?`, req.EntityID).Scan(&exists)
	if err != nil {
		writeError(w, http.StatusNotFound, "entity not found")
		return
	}

	rawToken, tokenHash, err := generatePrefixedToken(PrefixPAT)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to generate token")
		return
	}

	tokenID, err := id.New()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to generate id")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	scopesJSON, _ := json.Marshal(req.Scopes)

	_, err = a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO tokens (id, type, token_hash, entity_id, name, scopes, created_at)
		 VALUES (?, 'pat', ?, ?, ?, ?, ?)`,
		tokenID, tokenHash, req.EntityID, req.Name, string(scopesJSON), now,
	)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to create PAT: "+err.Error())
		return
	}

	a.EmitAuthEvent(r.Context(), "pat.created", req.EntityID, map[string]any{
		"token_id":  tokenID,
		"name":      req.Name,
		"entity_id": req.EntityID,
	})

	writeJSON(w, http.StatusCreated, CreatePATResponse{
		ID:        tokenID,
		Name:      req.Name,
		EntityID:  req.EntityID,
		Token:     rawToken,
		Scopes:    req.Scopes,
		CreatedAt: now,
	})
}

func (a *API) listPATs(w http.ResponseWriter, r *http.Request) {
	entityFilter, _ := strconv.ParseInt(r.URL.Query().Get("entity_id"), 10, 64)

	var query string
	var args []any
	if entityFilter > 0 {
		query = `SELECT id, name, entity_id, scopes, last_used, created_at
		         FROM tokens WHERE type = 'pat' AND revoked_at IS NULL AND entity_id = ?
		         ORDER BY created_at DESC`
		args = []any{entityFilter}
	} else {
		query = `SELECT id, name, entity_id, scopes, last_used, created_at
		         FROM tokens WHERE type = 'pat' AND revoked_at IS NULL
		         ORDER BY created_at DESC`
	}

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var pats []PATResponse
	for rows.Next() {
		var p PATResponse
		var scopesStr string
		var lastUsed *string
		if err := rows.Scan(&p.ID, &p.Name, &p.EntityID, &scopesStr, &lastUsed, &p.CreatedAt); err != nil {
			continue
		}
		json.Unmarshal([]byte(scopesStr), &p.Scopes)
		p.LastUsed = lastUsed
		pats = append(pats, p)
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, "row iteration failed")
		return
	}

	writeJSON(w, http.StatusOK, ListResponse{Items: pats})
}

func (a *API) revokePAT(w http.ResponseWriter, r *http.Request) {
	tokenID, err := parseID(r, "id")
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid id")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	result, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE tokens SET revoked_at = ? WHERE id = ? AND type = 'pat' AND revoked_at IS NULL`,
		now, tokenID)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "revoke failed")
		return
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		writeError(w, http.StatusNotFound, fmt.Sprintf("PAT %d not found or already revoked", tokenID))
		return
	}

	a.EmitAuthEvent(r.Context(), "pat.revoked", 0, map[string]any{
		"token_id": tokenID,
	})

	w.WriteHeader(http.StatusNoContent)
}
