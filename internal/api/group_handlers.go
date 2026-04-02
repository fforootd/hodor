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
	scoped := a.db.Scoped(r.Context())

	var where []string
	var args []any
	where = append(where, "g.instance_id = ?")
	args = append(args, scoped.InstanceID())
	where = append(where, "g.id > ?")
	args = append(args, cursor)
	if orgID != "" {
		where = append(where, "g.org_id = ?")
		args = append(args, orgID)
	}

	query := fmt.Sprintf(`SELECT g.id, g.org_id, g.name, g.description, g.state,
		COALESCE(g.schema_id,''), COALESCE(g.metadata,'{}'), g.created_at, g.updated_at,
		(SELECT COUNT(*) FROM memberships m WHERE m.instance_id = g.instance_id AND m.resource_type='group' AND m.resource_id = g.id) as member_count
		FROM groups g WHERE %s ORDER BY g.id ASC LIMIT ?`,
		strings.Join(where, " AND "))
	args = append(args, limit+1)

	rows, err := scoped.QueryContext(r.Context(), scoped.Rebind(query), args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var groups []GroupResponse
	for rows.Next() {
		var g GroupResponse
		var metaStr string
		if err := rows.Scan(&g.ID, &g.OrgID, &g.Name, &g.Description, &g.State, &g.SchemaID,
			&metaStr, &g.CreatedAt, &g.UpdatedAt, &g.MemberCount); err != nil {
			continue
		}
		groups = append(groups, a.buildGroupResponse(r.Context(), g, metaStr))
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

	schemaRec, err := a.resolveResourceSchema(r.Context(), "group", req.SchemaID)
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
		data = groupCanonicalData(req.Name, req.Description, map[string]any{"metadata": req.Metadata})
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

	groupID := id.New()
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
		stx.Rebind(`INSERT INTO groups (instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?)`),
		stx.InstanceID(), groupID, orgID, name, description, schemaRec.ID, metadataJSON, now, now,
	)
	if err != nil {
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error": "database error", "code": 409, "details": err.Error(),
		})
		return
	}

	emitEvent(r.Context(), stx, "group.created", groupID, groupID, "group", map[string]any{
		"name": name, "org_id": orgID,
	})

	if svc := FGAService; svc != nil {
		creatorID := creatorFromRequest(r)
		fgaAsync("group created", func() {
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			if err := svc.OnGroupCreated(ctx, groupID, creatorID, orgID); err != nil {
				logging.Printf("[fga] group created: %v", err)
			}
		})
	}

	if err := stx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, GroupResponse{
		ID:          groupID,
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

func (a *API) getGroup(w http.ResponseWriter, r *http.Request) {
	groupID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	scoped := a.db.Scoped(r.Context())
	var g GroupResponse
	var metaStr string
	err = scoped.QueryRowContext(r.Context(),
		scoped.Rebind(`SELECT g.id, g.org_id, g.name, g.description, g.state,
		 COALESCE(g.schema_id,''), COALESCE(g.metadata,'{}'), g.created_at, g.updated_at,
		 (SELECT COUNT(*) FROM memberships m WHERE m.instance_id = g.instance_id AND m.resource_type='group' AND m.resource_id = g.id) as member_count
		 FROM groups g WHERE g.instance_id = ? AND g.id = ?`), scoped.InstanceID(), groupID,
	).Scan(&g.ID, &g.OrgID, &g.Name, &g.Description, &g.State, &g.SchemaID,
		&metaStr, &g.CreatedAt, &g.UpdatedAt, &g.MemberCount)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "group not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, a.buildGroupResponse(r.Context(), g, metaStr))
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

	scoped := a.db.Scoped(r.Context())
	var current GroupResponse
	var currentSchemaID string
	var metadataStr string
	err = scoped.QueryRowContext(r.Context(),
		scoped.Rebind(`SELECT id, org_id, name, description, state, COALESCE(schema_id,''), COALESCE(metadata,'{}'), created_at, updated_at,
		        (SELECT COUNT(*) FROM memberships m WHERE m.instance_id = ? AND m.resource_type='group' AND m.resource_id = groups.id) as member_count
		 FROM groups
		 WHERE instance_id = ? AND id = ?`),
		scoped.InstanceID(), scoped.InstanceID(), groupID,
	).Scan(&current.ID, &current.OrgID, &current.Name, &current.Description, &current.State, &currentSchemaID, &metadataStr, &current.CreatedAt, &current.UpdatedAt, &current.MemberCount)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "group not found")
		return
	}
	current.SchemaID = currentSchemaID

	schemaRec, err := a.resolveResourceSchema(r.Context(), "group", firstNonEmptyString(strings.TrimSpace(req.SchemaID), currentSchemaID))
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
		data = groupCanonicalData(current.Name, current.Description, decodeObjectString(metadataStr))
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
		scoped.Rebind(`UPDATE groups
		 SET name = ?, description = ?, state = ?, schema_id = ?, metadata = ?, updated_at = ?
		 WHERE instance_id = ? AND id = ?`),
		stringFromAny(data["name"]),
		stringFromAny(data["description"]),
		nextState,
		schemaRec.ID,
		encodeObjectString(stripKeys(data, "name", "description")),
		timeNow(),
		scoped.InstanceID(),
		groupID,
	)
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
	a.getGroup(w, r)
}

func (a *API) deleteGroup(w http.ResponseWriter, r *http.Request) {
	groupID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	scoped := a.db.Scoped(r.Context())
	result, err := scoped.ExecContext(r.Context(), scoped.Rebind(`DELETE FROM groups WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), groupID)
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
