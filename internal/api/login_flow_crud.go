package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
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

	flowID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)

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

	configJSON := "{}"
	if req.Config != nil {
		if b, err := json.Marshal(req.Config); err == nil {
			configJSON = string(b)
		}
	} else if req.Profile != nil {
		if b, err := json.Marshal(req.Profile); err == nil {
			configJSON = string(b)
		}
	}

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

	_, err := a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO login_flows (id, org_id, name, strategy, config, is_default, enabled, state, priority, audience, auth_methods, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?)`,
		flowID, orgID, name, strategy, configJSON,
		boolToInt(req.IsDefault), state, req.Priority,
		audienceJSON, authMethodsJSON, now, now,
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

	var args []any

	query := `SELECT id, COALESCE(org_id,''), name, strategy, config, COALESCE(is_default,0), COALESCE(enabled,1), state, priority,
	                  COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'),
	                  COALESCE(metadata,'{}'), created_at, updated_at
	           FROM login_flows `

	if stateFilter != "" {
		query += ` WHERE state = ?`
		args = append(args, stateFilter)
	}
	query += ` ORDER BY COALESCE(is_default,0) DESC, priority DESC, created_at DESC`

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
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

	config := req.Config
	if config == nil {
		config = req.Profile
	}

	p := newPatch()
	p.Set("name", name)
	p.Set("strategy", req.Strategy)
	p.Set("state", req.State)
	if req.Priority != 0 {
		p.SetInt("priority", req.Priority)
	}
	p.SetJSON("config", config)
	p.SetJSON("audience", req.Audience)
	p.SetJSON("auth_methods", req.AuthMethods)
	p.SetInt("is_default", boolToInt(req.IsDefault))

	query, args := p.Build("login_flows", flowID)
	result, err := a.db.SQL().ExecContext(r.Context(), query, args...)
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

func (a *API) deleteLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	var isDefault int
	_ = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(is_default,0) FROM login_flows WHERE id = ?`, flowID,
	).Scan(&isDefault)
	if isDefault != 0 {
		httputil.WriteError(w, http.StatusBadRequest, "cannot delete the default login flow — edit it instead")
		return
	}

	result, err := a.db.SQL().ExecContext(r.Context(), `DELETE FROM login_flows WHERE id = ?`, flowID)
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
