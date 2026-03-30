package api

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"

	"github.com/zitadel/zitadel/internal/logging"
)

// ──────────────────────────────────────────────────────────────────
// Group types
// ──────────────────────────────────────────────────────────────────

type GroupRequest struct {
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	State       string `json:"state,omitempty"`
	Metadata    any    `json:"metadata,omitempty"`
}

type GroupResponse struct {
	ID          string `json:"id"`
	OrgID       string `json:"org_id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	State       string `json:"state"`
	Metadata    any    `json:"metadata,omitempty"`
	MemberCount int    `json:"member_count"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

type MemberRequest struct {
	UserID string `json:"user_id"`
	Role   string `json:"role,omitempty"`
}

type MemberResponse struct {
	UserID      string `json:"user_id"`
	DisplayName string `json:"display_name,omitempty"`
	Role        string `json:"role"`
	AddedAt     string `json:"added_at"`
}

// ──────────────────────────────────────────────────────────────────
// Group handlers
// ──────────────────────────────────────────────────────────────────

func (a *API) RegisterGroupRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/groups", a.listGroups)
	mux.HandleFunc("POST /v1/groups", a.createGroup)
	mux.HandleFunc("GET /v1/groups/{id}", a.getGroup)
	mux.HandleFunc("PATCH /v1/groups/{id}", a.updateGroup)
	mux.HandleFunc("DELETE /v1/groups/{id}", a.deleteGroup)
	mux.HandleFunc("GET /v1/groups/{id}/members", a.listMembers("group"))
	mux.HandleFunc("POST /v1/groups/{id}/members", a.addMember("group"))
	mux.HandleFunc("DELETE /v1/groups/{id}/members/{userId}", a.removeMember("group"))
	logging.Printf("[api] registered /v1/groups (full CRUD + members)")
}

func (a *API) listGroups(w http.ResponseWriter, r *http.Request) {
	limit, cursor := parsePagination(r)
	orgID := r.URL.Query().Get("org_id")

	var where []string
	var args []any
	where = append(where, "g.id > ?")
	args = append(args, cursor)
	if orgID != "" {
		where = append(where, "g.org_id = ?")
		args = append(args, orgID)
	}

	query := fmt.Sprintf(`SELECT g.id, g.org_id, g.name, g.description, g.state,
		COALESCE(g.metadata,'{}'), g.created_at, g.updated_at,
		(SELECT COUNT(*) FROM memberships m WHERE m.resource_type='group' AND m.resource_id = g.id) as member_count
		FROM groups g WHERE %s ORDER BY g.id ASC LIMIT ?`,
		strings.Join(where, " AND "))
	args = append(args, limit+1)

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var groups []GroupResponse
	for rows.Next() {
		var g GroupResponse
		var metaStr string
		if err := rows.Scan(&g.ID, &g.OrgID, &g.Name, &g.Description, &g.State,
			&metaStr, &g.CreatedAt, &g.UpdatedAt, &g.MemberCount); err != nil {
			continue
		}
		_ = json.Unmarshal([]byte(metaStr), &g.Metadata)
		groups = append(groups, g)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "row iteration failed")
		return
	}

	var nextCursor string
	if len(groups) > limit {
		groups = groups[:limit]
		nextCursor = groups[len(groups)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: groups, NextCursor: nextCursor})
}

func (a *API) createGroup(w http.ResponseWriter, r *http.Request) {
	var req GroupRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "name is required")
		return
	}

	groupID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)
	orgID := r.Header.Get("X-Org-Id")
	if orgID == "" {
		orgID = "1"
	}

	metadataJSON := "{}"
	if req.Metadata != nil {
		if b, err := json.Marshal(req.Metadata); err == nil {
			metadataJSON = string(b)
		}
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO groups (id, org_id, name, description, state, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, 'active', ?, ?, ?)`,
		groupID, orgID, req.Name, req.Description, metadataJSON, now, now,
	)
	if err != nil {
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error": "database error", "code": 409, "details": err.Error(),
		})
		return
	}

	emitEvent(r.Context(), tx, "group.created", groupID, groupID, "group", map[string]any{
		"name": req.Name, "org_id": orgID,
	})

	// FGA: write hierarchy + ownership tuples — async, best-effort.
	if svc := FGAService; svc != nil {
		creatorID := creatorFromRequest(r)
		fgaAsync("group created", func() { //nolint:contextcheck
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			if err := svc.OnGroupCreated(ctx, groupID, creatorID, orgID); err != nil {
				logging.Printf("[fga] group created: %v", err)
			}
		})
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, GroupResponse{
		ID: groupID, OrgID: orgID, Name: req.Name,
		Description: req.Description, State: "active",
		CreatedAt: now, UpdatedAt: now,
	})
}

func (a *API) getGroup(w http.ResponseWriter, r *http.Request) {
	groupID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var g GroupResponse
	var metaStr string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT g.id, g.org_id, g.name, g.description, g.state,
		 COALESCE(g.metadata,'{}'), g.created_at, g.updated_at,
		 (SELECT COUNT(*) FROM memberships m WHERE m.resource_type='group' AND m.resource_id = g.id) as member_count
		 FROM groups g WHERE g.id = ?`, groupID,
	).Scan(&g.ID, &g.OrgID, &g.Name, &g.Description, &g.State,
		&metaStr, &g.CreatedAt, &g.UpdatedAt, &g.MemberCount)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "group not found")
		return
	}
	_ = json.Unmarshal([]byte(metaStr), &g.Metadata)

	httputil.WriteJSON(w, http.StatusOK, g)
}

func (a *API) updateGroup(w http.ResponseWriter, r *http.Request) {
	groupID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var req GroupRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	p := newPatch()
	p.Set("name", req.Name)
	p.Set("description", req.Description)
	p.Set("state", req.State)
	p.SetJSON("metadata", req.Metadata)

	query, args := p.Build("groups", groupID)
	result, err := a.db.SQL().ExecContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "group not found")
		return
	}

	a.bus.Signal()

	// Re-read.
	a.getGroup(w, r)
}

func (a *API) deleteGroup(w http.ResponseWriter, r *http.Request) {
	groupID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	result, err := a.db.SQL().ExecContext(r.Context(), `DELETE FROM groups WHERE id = ?`, groupID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "group not found")
		return
	}

	a.bus.Signal()
	w.WriteHeader(http.StatusNoContent)
}

// ──────────────────────────────────────────────────────────────────
// Project types
// ──────────────────────────────────────────────────────────────────

type ProjectRequest struct {
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	State       string `json:"state,omitempty"`
	Metadata    any    `json:"metadata,omitempty"`
}

type ProjectResponse struct {
	ID          string `json:"id"`
	OrgID       string `json:"org_id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	State       string `json:"state"`
	Metadata    any    `json:"metadata,omitempty"`
	MemberCount int    `json:"member_count"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

// ──────────────────────────────────────────────────────────────────
// Project handlers
// ──────────────────────────────────────────────────────────────────

func (a *API) RegisterProjectRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/projects", a.listProjects)
	mux.HandleFunc("POST /v1/projects", a.createProject)
	mux.HandleFunc("GET /v1/projects/{id}", a.getProject)
	mux.HandleFunc("PATCH /v1/projects/{id}", a.updateProject)
	mux.HandleFunc("DELETE /v1/projects/{id}", a.deleteProject)
	mux.HandleFunc("GET /v1/projects/{id}/members", a.listMembers("project"))
	mux.HandleFunc("POST /v1/projects/{id}/members", a.addMember("project"))
	mux.HandleFunc("DELETE /v1/projects/{id}/members/{userId}", a.removeMember("project"))
	logging.Printf("[api] registered /v1/projects (full CRUD + members)")
}

func (a *API) listProjects(w http.ResponseWriter, r *http.Request) {
	limit, cursor := parsePagination(r)
	orgID := r.URL.Query().Get("org_id")

	var where []string
	var args []any
	where = append(where, "p.id > ?")
	args = append(args, cursor)
	if orgID != "" {
		where = append(where, "p.org_id = ?")
		args = append(args, orgID)
	}

	query := fmt.Sprintf(`SELECT p.id, p.org_id, p.name, p.description, p.state,
		COALESCE(p.metadata,'{}'), p.created_at, p.updated_at,
		(SELECT COUNT(*) FROM memberships m WHERE m.resource_type='project' AND m.resource_id = p.id) as member_count
		FROM projects p WHERE %s ORDER BY p.id ASC LIMIT ?`,
		strings.Join(where, " AND "))
	args = append(args, limit+1)

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var projects []ProjectResponse
	for rows.Next() {
		var p ProjectResponse
		var metaStr string
		if err := rows.Scan(&p.ID, &p.OrgID, &p.Name, &p.Description, &p.State,
			&metaStr, &p.CreatedAt, &p.UpdatedAt, &p.MemberCount); err != nil {
			continue
		}
		_ = json.Unmarshal([]byte(metaStr), &p.Metadata)
		projects = append(projects, p)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "row iteration failed")
		return
	}

	var nextCursor string
	if len(projects) > limit {
		projects = projects[:limit]
		nextCursor = projects[len(projects)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: projects, NextCursor: nextCursor})
}

func (a *API) createProject(w http.ResponseWriter, r *http.Request) {
	var req ProjectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "name is required")
		return
	}

	projectID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)
	orgID := r.Header.Get("X-Org-Id")
	if orgID == "" {
		orgID = "1"
	}

	metadataJSON := "{}"
	if req.Metadata != nil {
		if b, err := json.Marshal(req.Metadata); err == nil {
			metadataJSON = string(b)
		}
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO projects (id, org_id, name, description, state, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, 'active', ?, ?, ?)`,
		projectID, orgID, req.Name, req.Description, metadataJSON, now, now,
	)
	if err != nil {
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error": "database error", "code": 409, "details": err.Error(),
		})
		return
	}

	emitEvent(r.Context(), tx, "project.created", projectID, projectID, "project", map[string]any{
		"name": req.Name, "org_id": orgID,
	})

	// FGA: write hierarchy + ownership tuples — async, best-effort.
	if svc := FGAService; svc != nil {
		creatorID := creatorFromRequest(r)
		fgaAsync("project created", func() { //nolint:contextcheck
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			if err := svc.OnProjectCreated(ctx, projectID, creatorID, orgID); err != nil {
				logging.Printf("[fga] project created: %v", err)
			}
		})
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, ProjectResponse{
		ID: projectID, OrgID: orgID, Name: req.Name,
		Description: req.Description, State: "active",
		CreatedAt: now, UpdatedAt: now,
	})
}

func (a *API) getProject(w http.ResponseWriter, r *http.Request) {
	projectID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var p ProjectResponse
	var metaStr string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT p.id, p.org_id, p.name, p.description, p.state,
		 COALESCE(p.metadata,'{}'), p.created_at, p.updated_at,
		 (SELECT COUNT(*) FROM memberships m WHERE m.resource_type='project' AND m.resource_id = p.id) as member_count
		 FROM projects p WHERE p.id = ?`, projectID,
	).Scan(&p.ID, &p.OrgID, &p.Name, &p.Description, &p.State,
		&metaStr, &p.CreatedAt, &p.UpdatedAt, &p.MemberCount)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "project not found")
		return
	}
	_ = json.Unmarshal([]byte(metaStr), &p.Metadata)

	httputil.WriteJSON(w, http.StatusOK, p)
}

func (a *API) updateProject(w http.ResponseWriter, r *http.Request) {
	projectID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var req ProjectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	p := newPatch()
	p.Set("name", req.Name)
	p.Set("description", req.Description)
	p.Set("state", req.State)
	p.SetJSON("metadata", req.Metadata)

	query, args := p.Build("projects", projectID)
	result, err := a.db.SQL().ExecContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "project not found")
		return
	}

	a.bus.Signal()
	a.getProject(w, r)
}

func (a *API) deleteProject(w http.ResponseWriter, r *http.Request) {
	projectID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	result, err := a.db.SQL().ExecContext(r.Context(), `DELETE FROM projects WHERE id = ?`, projectID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "project not found")
		return
	}

	a.bus.Signal()
	w.WriteHeader(http.StatusNoContent)
}

// ── Module management API ──

func (a *API) RegisterModuleRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/modules", a.listModules)
	mux.HandleFunc("POST /v1/modules/{name}/enable", a.enableModule)
	mux.HandleFunc("POST /v1/modules/{name}/disable", a.disableModule)
	logging.Printf("[api] registered /v1/modules (enable/disable)")
}

func (a *API) listModules(w http.ResponseWriter, r *http.Request) {
	svc := FGAService
	if svc == nil {
		httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: []any{}})
		return
	}

	type moduleInfo struct {
		Name        string `json:"name"`
		Description string `json:"description"`
		Enabled     bool   `json:"enabled"`
	}

	enabled := make(map[string]bool)
	for _, name := range svc.EnabledModules() {
		enabled[name] = true
	}

	// Import module registry info.
	modules := []moduleInfo{
		{Name: "rbac", Description: "Role-Based Access Control", Enabled: enabled["rbac"]},
		{Name: "abac", Description: "Attribute-Based Access Control", Enabled: enabled["abac"]},
		{Name: "teams", Description: "Hierarchical Teams", Enabled: enabled["teams"]},
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: modules})
}

func (a *API) enableModule(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not available")
		return
	}

	if err := svc.EnableModule(r.Context(), name); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"module": name, "enabled": true})
}

func (a *API) disableModule(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")
	svc := FGAService
	if svc == nil {
		httputil.WriteError(w, http.StatusServiceUnavailable, "FGA not available")
		return
	}

	if err := svc.DisableModule(r.Context(), name); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"module": name, "enabled": false})
}

// ── helpers ──
