package api

import (
	"encoding/json"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/resourcedata"
	"github.com/zitadel/zitadel/internal/schema"
)

func (a *API) createLoginFlow(w http.ResponseWriter, r *http.Request) {
	var req LoginFlowRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	name := req.Name
	if name == "" {
		name = req.DisplayName
	}
	if name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "name is required")
		return
	}

	schemaRec, err := a.resolveResourceSchema(r.Context(), "login_flow", req.SchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	strategy := req.Strategy
	if strategy == "" {
		strategy = "identifier_first"
	}

	state := req.State
	if state == "" {
		state = "draft"
	}
	if state != "draft" && state != "testing" && state != "active" && state != "archived" {
		httputil.WriteError(w, http.StatusBadRequest, "state must be one of: draft, testing, active, archived")
		return
	}

	flowData, configJSON, err := buildLoginFlowWrite(req, name, strategy, state)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := schema.ValidateData(schemaRec.Schema, flowData); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	flowID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)

	audienceJSON := "{}"
	if req.Audience != nil {
		if b, err := json.Marshal(req.Audience); err == nil {
			audienceJSON = string(b)
		}
	}

	authMethodsJSON := "{}"
	if req.AuthMethods != nil {
		if b, err := json.Marshal(req.AuthMethods); err == nil {
			authMethodsJSON = string(b)
		}
	}

	orgID := r.Header.Get("X-Org-Id")
	if orgID == "" {
		orgID = "1"
	}

	scoped := a.db.Scoped(r.Context())
	_, err = scoped.ExecContext(r.Context(),
		scoped.Rebind(`INSERT INTO login_flows (instance_id, id, org_id, name, strategy, config, is_default, enabled, state, priority, audience, auth_methods, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '{}', ?, ?)`),
		scoped.InstanceID(), flowID, orgID, name, strategy, configJSON,
		req.IsDefault, true, state, req.Priority,
		audienceJSON, authMethodsJSON, schemaRec.ID, now, now,
	)
	if err != nil {
		logging.Printf("[createLoginFlow] DB insert failed: %v", err)
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create login flow")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, LoginFlowResponse{
		ID:          flowID,
		OrgID:       orgID,
		SchemaID:    schemaRec.ID,
		Name:        name,
		Strategy:    strategy,
		IsDefault:   req.IsDefault,
		Enabled:     true,
		State:       state,
		Priority:    req.Priority,
		Audience:    req.Audience,
		AuthMethods: req.AuthMethods,
		Config:      req.Config,
		CreatedAt:   now,
		UpdatedAt:   now,
	})
}

func (a *API) getLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	resp, err := a.loadLoginFlow(r.Context(), flowID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}
	httputil.WriteJSON(w, http.StatusOK, resp)
}

func (a *API) listLoginFlows(w http.ResponseWriter, r *http.Request) {
	stateFilter := r.URL.Query().Get("state")
	scoped := a.db.Scoped(r.Context())

	var args []any
	args = append(args, scoped.InstanceID())

	query := `SELECT id, COALESCE(org_id,''), COALESCE(schema_id,''), name, strategy, config,
	                  CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END,
	                  CASE WHEN COALESCE(enabled, true) THEN 1 ELSE 0 END, state, priority,
	                  COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'),
	                  COALESCE(metadata,'{}'), created_at, updated_at
	           FROM login_flows WHERE instance_id = ?`

	if stateFilter != "" {
		query += ` AND state = ?`
		args = append(args, stateFilter)
	}
	query += ` ORDER BY CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END DESC, priority DESC, created_at DESC`

	rows, err := scoped.QueryContext(r.Context(), scoped.Rebind(query), args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var items []LoginFlowResponse
	for rows.Next() {
		resp, err := scanLoginFlowRow(rows)
		if err != nil {
			continue
		}
		items = append(items, resp)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: items})
}

func (a *API) updateLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	var req LoginFlowRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	if req.State != "" {
		if req.State != "draft" && req.State != "testing" && req.State != "active" && req.State != "archived" {
			httputil.WriteError(w, http.StatusBadRequest, "invalid state")
			return
		}
	}

	name := req.Name
	if name == "" {
		name = req.DisplayName
	}

	current, err := a.loadLoginFlow(r.Context(), flowID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}

	strategy := current.Strategy
	if req.Strategy != "" {
		strategy = req.Strategy
	}
	state := current.State
	if req.State != "" {
		state = req.State
	}
	priority := current.Priority
	if req.Priority != 0 {
		priority = req.Priority
	}
	isDefault := current.IsDefault
	if req.IsDefault {
		isDefault = true
	}
	configValue := current.Config
	if req.Config != nil {
		configValue = req.Config
	} else if req.Profile != nil {
		configValue = req.Profile
	}
	audienceValue := current.Audience
	if req.Audience != nil {
		audienceValue = req.Audience
	}
	authMethodsValue := current.AuthMethods
	if req.AuthMethods != nil {
		authMethodsValue = req.AuthMethods
	}
	if name == "" {
		name = current.Name
	}

	schemaRec, err := a.resolveResourceSchema(r.Context(), "login_flow", firstNonEmptyString(strings.TrimSpace(req.SchemaID), current.SchemaID))
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	flowData, configJSON, err := buildLoginFlowCanonicalData(name, strategy, isDefault, state, priority, audienceValue, authMethodsValue, configValue)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := schema.ValidateData(schemaRec.Schema, flowData); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	scoped := a.db.Scoped(r.Context())
	audienceJSON := marshalJSON(audienceValue)
	authMethodsJSON := marshalJSON(authMethodsValue)
	query := `UPDATE login_flows
		SET name = ?, strategy = ?, config = ?, is_default = ?, state = ?, priority = ?, audience = ?, auth_methods = ?, schema_id = ?, updated_at = ?
		WHERE instance_id = ? AND id = ?`
	result, err := scoped.ExecContext(r.Context(), scoped.Rebind(query),
		name, strategy, configJSON, isDefault, state, priority, audienceJSON, authMethodsJSON, schemaRec.ID, timeNow(), scoped.InstanceID(), flowID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed: "+err.Error())
		return
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}

	a.bus.Signal()

	resp, _ := a.loadLoginFlow(r.Context(), flowID)
	httputil.WriteJSON(w, http.StatusOK, resp)
}

func buildLoginFlowWrite(req LoginFlowRequest, name, strategy, state string) (map[string]any, string, error) {
	configValue := req.Config
	if configValue == nil {
		configValue = req.Profile
	}
	return buildLoginFlowCanonicalData(name, strategy, req.IsDefault, state, req.Priority, req.Audience, req.AuthMethods, configValue)
}

func buildLoginFlowCanonicalData(name, strategy string, isDefault bool, state string, priority int, audience, authMethods, configValue any) (map[string]any, string, error) {
	data, configMap, err := resourcedata.BuildLoginFlowSchemaData(name, strategy, isDefault, state, priority, audience, authMethods, configValue, nil)
	if err != nil {
		return nil, "", err
	}

	configJSON := "{}"
	if len(configMap) > 0 {
		raw, marshalErr := json.Marshal(configMap)
		if marshalErr != nil {
			return nil, "", marshalErr
		}
		configJSON = string(raw)
	}
	return data, configJSON, nil
}

func (a *API) deleteLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	scoped := a.db.Scoped(r.Context())
	var isDefault int
	_ = scoped.QueryRowContext(r.Context(),
		scoped.Rebind(`SELECT CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END FROM login_flows WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), flowID,
	).Scan(&isDefault)
	if isDefault != 0 {
		httputil.WriteError(w, http.StatusBadRequest, "cannot delete the default login flow — edit it instead")
		return
	}

	result, err := scoped.ExecContext(r.Context(), scoped.Rebind(`DELETE FROM login_flows WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), flowID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}

	a.bus.Signal()
	w.WriteHeader(http.StatusNoContent)
}
