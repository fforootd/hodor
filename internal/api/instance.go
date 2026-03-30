package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"regexp"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/instance"
	"github.com/zitadel/zitadel/internal/logging"
)

// --- Instance types ---

// InstanceRequest is the JSON body for creating/updating instances.
type InstanceRequest struct {
	Name   string `json:"name"`
	Domain string `json:"domain,omitempty"`
	State  string `json:"state,omitempty"`
}

// InstanceResponse is the JSON response for an instance.
type InstanceResponse struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Domain    string `json:"domain"`
	IsRoot    bool   `json:"is_root"`
	State     string `json:"state"`
	CreatedAt string `json:"created_at"`
	UpdatedAt string `json:"updated_at"`
}

// instanceSlugPattern validates instance ID slugs.
var instanceSlugPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9_-]{1,62}[a-z0-9]$`)

// --- Instance routes ---

// RegisterInstanceRoutes mounts instance management endpoints.
func (a *API) RegisterInstanceRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/instances", a.listInstances)
	mux.HandleFunc("POST /v1/instances", a.createInstance)
	mux.HandleFunc("GET /v1/instances/{id}", a.getInstance)
	mux.HandleFunc("PATCH /v1/instances/{id}", a.updateInstance)
	mux.HandleFunc("DELETE /v1/instances/{id}", a.deleteInstance)
}

// RegisterInstanceProxyRoutes mounts the nested instance proxy.
// Requests to /v1/instances/{iid}/... are rewritten to /v1/... with
// instance context injected.
func (a *API) RegisterInstanceProxyRoutes(mux *http.ServeMux, inner http.Handler) {
	mux.HandleFunc("/v1/instances/{iid}/", func(w http.ResponseWriter, r *http.Request) {
		iid := r.PathValue("iid")
		if iid == "" {
			httputil.WriteError(w, http.StatusBadRequest, "instance ID required")
			return
		}

		// Verify the instance exists.
		var exists int
		if err := a.db.SQL().QueryRowContext(r.Context(),
			`SELECT COUNT(*) FROM instances WHERE id = ?`, iid,
		).Scan(&exists); err != nil || exists == 0 {
			httputil.WriteError(w, http.StatusNotFound, "instance not found")
			return
		}

		// Set instance context and rewrite path.
		ctx := instance.WithContext(r.Context(), iid)
		// Strip /v1/instances/{iid} prefix, keep the rest.
		prefix := fmt.Sprintf("/v1/instances/%s", iid)
		newPath := strings.TrimPrefix(r.URL.Path, prefix)
		if newPath == "" || newPath == "/" {
			newPath = "/"
		}
		newPath = "/v1" + newPath

		// Clone the request with new path and context.
		r2 := r.Clone(ctx)
		r2.URL.Path = newPath
		r2.RequestURI = newPath
		if r.URL.RawQuery != "" {
			r2.RequestURI = newPath + "?" + r.URL.RawQuery
		}

		inner.ServeHTTP(w, r2)
	})
}

// --- Instance handlers ---

func (a *API) listInstances(w http.ResponseWriter, r *http.Request) {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, name, COALESCE(domain,''), COALESCE(is_root,0), state, created_at, updated_at
		 FROM instances ORDER BY is_root DESC, name ASC`)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var items []InstanceResponse
	for rows.Next() {
		var inst InstanceResponse
		if err := rows.Scan(&inst.ID, &inst.Name, &inst.Domain, &inst.IsRoot, &inst.State, &inst.CreatedAt, &inst.UpdatedAt); err != nil {
			continue
		}
		items = append(items, inst)
	}
	if err := rows.Err(); err != nil {
		logging.Printf("[listInstances] rows error: %v", err)
	}
	if items == nil {
		items = []InstanceResponse{}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"items": items})
}

func (a *API) createInstance(w http.ResponseWriter, r *http.Request) {
	var req InstanceRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "name is required")
		return
	}

	// Generate slug ID from name.
	slug := strings.ToLower(strings.ReplaceAll(req.Name, " ", "-"))
	slug = regexp.MustCompile(`[^a-z0-9-]`).ReplaceAllString(slug, "")
	if len(slug) < 3 {
		slug = "inst-" + id.New()[:8]
	}
	if !instanceSlugPattern.MatchString(slug) {
		slug = "inst-" + id.New()[:8]
	}

	now := time.Now().UTC().Format(time.RFC3339)
	domain := req.Domain

	_, err := a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO instances (id, name, domain, is_root, state, created_at, updated_at)
		 VALUES (?, ?, ?, 0, 'active', ?, ?)`,
		slug, req.Name, domain, now, now,
	)
	if err != nil {
		logging.Printf("[createInstance] insert failed: %v", err)
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error":   "instance already exists or slug conflict",
			"details": err.Error(),
		})
		return
	}

	// Write FGA tuples for the new instance.
	if FGAService != nil {
		userID := r.Header.Get("X-Identity-Id")
		if err := FGAService.OnInstanceCreated(r.Context(), slug, userID); err != nil {
			logging.Printf("[createInstance] FGA tuple write failed: %v", err)
		}
	}

	// Emit event.
	tx, _ := a.db.SQL().BeginTx(r.Context(), nil)
	if tx != nil {
		emitEvent(r.Context(), tx, "instance.created", slug, slug, "instance", map[string]any{
			"name":   req.Name,
			"domain": domain,
		})
		_ = tx.Commit()
	}

	httputil.WriteJSON(w, http.StatusCreated, InstanceResponse{
		ID:        slug,
		Name:      req.Name,
		Domain:    domain,
		IsRoot:    false,
		State:     "active",
		CreatedAt: now,
		UpdatedAt: now,
	})
}

func (a *API) getInstance(w http.ResponseWriter, r *http.Request) {
	instID := r.PathValue("id")

	var inst InstanceResponse
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, name, COALESCE(domain,''), COALESCE(is_root,0), state, created_at, updated_at
		 FROM instances WHERE id = ?`, instID,
	).Scan(&inst.ID, &inst.Name, &inst.Domain, &inst.IsRoot, &inst.State, &inst.CreatedAt, &inst.UpdatedAt)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "instance not found")
		return
	}

	// Get counts for this instance.
	var userCount, orgCount int
	_ = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM users WHERE instance_id = ?`, instID).Scan(&userCount)
	_ = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM orgs WHERE instance_id = ?`, instID).Scan(&orgCount)

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"instance":   inst,
		"user_count": userCount,
		"org_count":  orgCount,
	})
}

func (a *API) updateInstance(w http.ResponseWriter, r *http.Request) {
	instID := r.PathValue("id")

	// Don't allow modifying the root instance.
	var isRoot bool
	if err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(is_root,0) FROM instances WHERE id = ?`, instID,
	).Scan(&isRoot); err != nil {
		httputil.WriteError(w, http.StatusNotFound, "instance not found")
		return
	}

	var req InstanceRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)

	setClauses := []string{"updated_at = ?"}
	args := []any{now}

	if req.Name != "" {
		setClauses = append(setClauses, "name = ?")
		args = append(args, req.Name)
	}
	if req.Domain != "" {
		setClauses = append(setClauses, "domain = ?")
		args = append(args, req.Domain)
	}
	if req.State != "" && !isRoot {
		setClauses = append(setClauses, "state = ?")
		args = append(args, req.State)
	}

	args = append(args, instID)
	query := fmt.Sprintf("UPDATE instances SET %s WHERE id = ?", strings.Join(setClauses, ", "))

	if _, err := a.db.SQL().ExecContext(r.Context(), query, args...); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}

	w.WriteHeader(http.StatusNoContent)
}

func (a *API) deleteInstance(w http.ResponseWriter, r *http.Request) {
	instID := r.PathValue("id")

	// Don't allow deleting the root instance.
	var isRoot bool
	if err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(is_root,0) FROM instances WHERE id = ?`, instID,
	).Scan(&isRoot); err != nil {
		httputil.WriteError(w, http.StatusNotFound, "instance not found")
		return
	}
	if isRoot {
		httputil.WriteError(w, http.StatusForbidden, "cannot delete root instance")
		return
	}

	// Soft-delete: set state to 'deleted'.
	now := time.Now().UTC().Format(time.RFC3339)
	if _, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE instances SET state = 'deleted', updated_at = ? WHERE id = ?`, now, instID,
	); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}

	w.WriteHeader(http.StatusNoContent)
}
