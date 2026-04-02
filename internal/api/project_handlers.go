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
	"github.com/zitadel/zitadel/internal/schema"
)

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
	scoped := a.db.Scoped(r.Context())

	var where []string
	var args []any
	where = append(where, "p.instance_id = ?")
	args = append(args, scoped.InstanceID())
	where = append(where, "p.id > ?")
	args = append(args, cursor)
	if orgID != "" {
		where = append(where, "p.org_id = ?")
		args = append(args, orgID)
	}

	query := fmt.Sprintf(`SELECT p.id, p.org_id, p.name, p.description, p.state,
		COALESCE(p.schema_id,''), COALESCE(p.metadata,'{}'), p.created_at, p.updated_at,
		(SELECT COUNT(*) FROM memberships m WHERE m.instance_id = p.instance_id AND m.resource_type='project' AND m.resource_id = p.id) as member_count
		FROM projects p WHERE %s ORDER BY p.id ASC LIMIT ?`,
		strings.Join(where, " AND "))
	args = append(args, limit+1)

	rows, err := scoped.QueryContext(r.Context(), scoped.Rebind(query), args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var projects []ProjectResponse
	for rows.Next() {
		var p ProjectResponse
		var metaStr string
		if err := rows.Scan(&p.ID, &p.OrgID, &p.Name, &p.Description, &p.State, &p.SchemaID,
			&metaStr, &p.CreatedAt, &p.UpdatedAt, &p.MemberCount); err != nil {
			continue
		}
		projects = append(projects, a.buildProjectResponse(r.Context(), p, metaStr))
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

	schemaRec, err := a.resolveResourceSchema(r.Context(), "project", req.SchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(data) == 0 {
		data = projectCanonicalData(req.Name, req.Description, map[string]any{"metadata": req.Metadata})
	}
	name := stringFromAny(data["name"])
	if name == "" {
		name = strings.TrimSpace(req.Name)
		data["name"] = name
	}
	if name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "name is required")
		return
	}
	description := stringFromAny(data["description"])
	if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	projectID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)
	orgID := r.Header.Get("X-Org-Id")
	if orgID == "" {
		orgID = "1"
	}

	metadataJSON := encodeObjectString(stripKeys(data, "name", "description"))

	scoped := a.db.Scoped(r.Context())
	stx, err := scoped.BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer stx.Rollback()

	_, err = stx.ExecContext(r.Context(),
		stx.Rebind(`INSERT INTO projects (instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?)`),
		stx.InstanceID(), projectID, orgID, name, description, schemaRec.ID, metadataJSON, now, now,
	)
	if err != nil {
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error": "database error", "code": 409, "details": err.Error(),
		})
		return
	}

	emitEvent(r.Context(), stx, "project.created", projectID, projectID, "project", map[string]any{
		"name": name, "org_id": orgID,
	})

	if svc := FGAService; svc != nil {
		creatorID := creatorFromRequest(r)
		fgaAsync("project created", func() {
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			if err := svc.OnProjectCreated(ctx, projectID, creatorID, orgID); err != nil {
				logging.Printf("[fga] project created: %v", err)
			}
		})
	}

	if err := stx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, ProjectResponse{
		ID:          projectID,
		OrgID:       orgID,
		Name:        name,
		Description: description,
		State:       "active",
		SchemaID:    schemaRec.ID,
		SchemaType:  schemaRec.Type,
		Metadata:    data["metadata"],
		Data:        data,
		CreatedAt:   now,
		UpdatedAt:   now,
	})
}

func (a *API) getProject(w http.ResponseWriter, r *http.Request) {
	projectID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	scoped := a.db.Scoped(r.Context())
	var p ProjectResponse
	var metaStr string
	err = scoped.QueryRowContext(r.Context(),
		scoped.Rebind(`SELECT p.id, p.org_id, p.name, p.description, p.state,
		 COALESCE(p.schema_id,''), COALESCE(p.metadata,'{}'), p.created_at, p.updated_at,
		 (SELECT COUNT(*) FROM memberships m WHERE m.instance_id = p.instance_id AND m.resource_type='project' AND m.resource_id = p.id) as member_count
		 FROM projects p WHERE p.instance_id = ? AND p.id = ?`), scoped.InstanceID(), projectID,
	).Scan(&p.ID, &p.OrgID, &p.Name, &p.Description, &p.State, &p.SchemaID,
		&metaStr, &p.CreatedAt, &p.UpdatedAt, &p.MemberCount)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "project not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, a.buildProjectResponse(r.Context(), p, metaStr))
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

	scoped := a.db.Scoped(r.Context())
	var current ProjectResponse
	var currentSchemaID string
	var metadataStr string
	err = scoped.QueryRowContext(r.Context(),
		scoped.Rebind(`SELECT id, org_id, name, description, state, COALESCE(schema_id,''), COALESCE(metadata,'{}'), created_at, updated_at,
		        (SELECT COUNT(*) FROM memberships m WHERE m.instance_id = ? AND m.resource_type='project' AND m.resource_id = projects.id) as member_count
		 FROM projects
		 WHERE instance_id = ? AND id = ?`),
		scoped.InstanceID(), scoped.InstanceID(), projectID,
	).Scan(&current.ID, &current.OrgID, &current.Name, &current.Description, &current.State, &currentSchemaID, &metadataStr, &current.CreatedAt, &current.UpdatedAt, &current.MemberCount)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "project not found")
		return
	}
	current.SchemaID = currentSchemaID

	schemaRec, err := a.resolveResourceSchema(r.Context(), "project", firstNonEmptyString(strings.TrimSpace(req.SchemaID), currentSchemaID))
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(data) == 0 {
		data = projectCanonicalData(current.Name, current.Description, decodeObjectString(metadataStr))
		if strings.TrimSpace(req.Name) != "" {
			data["name"] = strings.TrimSpace(req.Name)
		}
		if strings.TrimSpace(req.Description) != "" {
			data["description"] = strings.TrimSpace(req.Description)
		}
		if req.Metadata != nil {
			data["metadata"] = req.Metadata
		}
	}
	if stringFromAny(data["name"]) == "" {
		data["name"] = current.Name
	}
	if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	nextState := current.State
	if strings.TrimSpace(req.State) != "" {
		nextState = strings.TrimSpace(req.State)
	}

	result, err := scoped.ExecContext(r.Context(),
		scoped.Rebind(`UPDATE projects
		 SET name = ?, description = ?, state = ?, schema_id = ?, metadata = ?, updated_at = ?
		 WHERE instance_id = ? AND id = ?`),
		stringFromAny(data["name"]),
		stringFromAny(data["description"]),
		nextState,
		schemaRec.ID,
		encodeObjectString(stripKeys(data, "name", "description")),
		timeNow(),
		scoped.InstanceID(),
		projectID,
	)
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

	scoped := a.db.Scoped(r.Context())
	result, err := scoped.ExecContext(r.Context(), scoped.Rebind(`DELETE FROM projects WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), projectID)
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
