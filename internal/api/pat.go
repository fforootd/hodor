package api

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"time"

	"github.com/zitadel/zitadel/internal/id"
)

// --- PAT types ---

type CreatePATRequest struct {
	UserID string   `json:"user_id"`
	Name   string   `json:"name"`
	Scopes []string `json:"scopes,omitempty"`
}

type CreatePATResponse struct {
	ID        string   `json:"id"`
	Name      string   `json:"name"`
	UserID    string   `json:"user_id"`
	Token     string   `json:"token"` // Only returned on creation — never again.
	Scopes    []string `json:"scopes"`
	CreatedAt string   `json:"created_at"`
}

type PATResponse struct {
	ID        string   `json:"id"`
	Name      string   `json:"name"`
	UserID    string   `json:"user_id"`
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
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.UserID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "user_id is required")
		return
	}
	if req.Name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "name is required")
		return
	}

	// Default scopes to ["admin"] if not specified.
	if len(req.Scopes) == 0 {
		req.Scopes = []string{"admin"}
	}

	scoped := a.db.Scoped(r.Context())

	// Verify entity exists.
	var exists int
	err := scoped.QueryRowContext(r.Context(),
		scoped.Rebind(`SELECT 1 FROM users WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), req.UserID).Scan(&exists)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "entity not found")
		return
	}

	rawToken, tokenHash, err := generatePrefixedToken(PrefixPAT)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to generate token")
		return
	}

	tokenID := id.New()

	now := time.Now().UTC().Format(time.RFC3339)
	scopesJSON, _ := json.Marshal(req.Scopes)

	_, err = scoped.ExecContext(r.Context(),
		scoped.Rebind(`INSERT INTO tokens (instance_id, id, type, token_hash, user_id, name, scopes, created_at)
		 VALUES (?, ?, 'pat', ?, ?, ?, ?, ?)`),
		scoped.InstanceID(), tokenID, tokenHash, req.UserID, req.Name, string(scopesJSON), now,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create PAT: "+err.Error())
		return
	}

	a.EmitAuthEvent(r.Context(), "pat.created", req.UserID, map[string]any{
		"token_id": tokenID,
		"name":     req.Name,
		"user_id":  req.UserID,
	})

	httputil.WriteJSON(w, http.StatusCreated, CreatePATResponse{
		ID:        tokenID,
		Name:      req.Name,
		UserID:    req.UserID,
		Token:     rawToken,
		Scopes:    req.Scopes,
		CreatedAt: now,
	})
}

func (a *API) listPATs(w http.ResponseWriter, r *http.Request) {
	entityFilter := r.URL.Query().Get("user_id")
	scoped := a.db.Scoped(r.Context())

	var query string
	var args []any
	if entityFilter != "" {
		query = scoped.Rebind(`SELECT id, name, user_id, scopes, last_used, created_at
		         FROM tokens WHERE instance_id = ? AND type = 'pat' AND revoked_at IS NULL AND user_id = ?
		         ORDER BY created_at DESC`)
		args = []any{scoped.InstanceID(), entityFilter}
	} else {
		query = scoped.Rebind(`SELECT id, name, user_id, scopes, last_used, created_at
		         FROM tokens WHERE instance_id = ? AND type = 'pat' AND revoked_at IS NULL
		         ORDER BY created_at DESC`)
		args = []any{scoped.InstanceID()}
	}

	rows, err := scoped.QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var pats []PATResponse
	for rows.Next() {
		var p PATResponse
		var scopesStr string
		var lastUsed *string
		if err := rows.Scan(&p.ID, &p.Name, &p.UserID, &scopesStr, &lastUsed, &p.CreatedAt); err != nil {
			continue
		}
		json.Unmarshal([]byte(scopesStr), &p.Scopes)
		p.LastUsed = lastUsed
		pats = append(pats, p)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "row iteration failed")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: pats})
}

func (a *API) revokePAT(w http.ResponseWriter, r *http.Request) {
	tokenID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	scoped := a.db.Scoped(r.Context())
	now := time.Now().UTC().Format(time.RFC3339)
	result, err := scoped.ExecContext(r.Context(),
		scoped.Rebind(`UPDATE tokens SET revoked_at = ? WHERE instance_id = ? AND id = ? AND type = 'pat' AND revoked_at IS NULL`),
		now, scoped.InstanceID(), tokenID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "revoke failed")
		return
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		httputil.WriteError(w, http.StatusNotFound, fmt.Sprintf("PAT %s not found or already revoked", tokenID))
		return
	}

	a.EmitAuthEvent(r.Context(), "pat.revoked", "", map[string]any{
		"token_id": tokenID,
	})

	w.WriteHeader(http.StatusNoContent)
}
