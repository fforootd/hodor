package api

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/loginflow"
)

func (a *API) promoteLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	var currentState string
	err := a.db.SQL().QueryRowContext(r.Context(),
		a.bindQuery(`SELECT state FROM login_flows WHERE id = ?`), flowID,
	).Scan(&currentState)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}

	var nextState string
	switch currentState {
	case "draft":
		nextState = "testing"
	case "testing":
		nextState = "active"
	case "active":
		httputil.WriteError(w, http.StatusBadRequest, "flow is already active")
		return
	case "archived":
		httputil.WriteError(w, http.StatusBadRequest, "cannot promote archived flow; create a new version")
		return
	default:
		httputil.WriteError(w, http.StatusBadRequest, "unknown state: "+currentState)
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	_, err = a.db.SQL().ExecContext(r.Context(),
		a.bindQuery(`UPDATE login_flows SET state = ?, updated_at = ? WHERE id = ?`),
		nextState, now, flowID,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "promote failed")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"id":             flowID,
		"previous_state": currentState,
		"state":          nextState,
	})
}

func (a *API) archiveLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	result, err := a.db.SQL().ExecContext(r.Context(),
		a.bindQuery(`UPDATE login_flows SET state = 'archived', updated_at = ? WHERE id = ?`),
		now, flowID,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "archive failed")
		return
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}

	a.bus.Signal()
	httputil.WriteJSON(w, http.StatusOK, map[string]any{"id": flowID, "state": "archived"})
}

func (a *API) testLoginFlowAudience(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	resolver := loginflow.NewResolver(a.db)
	result, err := resolver.TestAudience(r.Context(), flowID, 20)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, err.Error())
		return
	}

	httputil.WriteJSON(w, http.StatusOK, result)
}

func (a *API) exportLoginFlow(w http.ResponseWriter, r *http.Request) {
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

	export := map[string]any{
		"template": map[string]any{
			"name":        resp.Name,
			"type":        "login_flow",
			"version":     "1.0",
			"description": fmt.Sprintf("Exported from flow %s", flowID),
			"tags":        []string{"login", resp.Strategy},
		},
		"variables": map[string]any{},
		"payload": map[string]any{
			"name":         resp.Name,
			"strategy":     resp.Strategy,
			"config":       resp.Config,
			"audience":     resp.Audience,
			"auth_methods": resp.AuthMethods,
			"priority":     resp.Priority,
		},
	}

	w.Header().Set("Content-Disposition", fmt.Sprintf(`attachment; filename="%s.json"`, flowID))
	httputil.WriteJSON(w, http.StatusOK, export)
}

func (a *API) resolveLoginFlow(w http.ResponseWriter, r *http.Request) {
	var req struct {
		UserID   string            `json:"user_id"`
		OrgID    string            `json:"org_id"`
		SchemaID string            `json:"schema_id"`
		Metadata map[string]string `json:"metadata"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	resolver := loginflow.NewResolver(a.db)
	flow, err := resolver.Resolve(r.Context(), loginflow.UserContext{
		UserID:   req.UserID,
		OrgID:    req.OrgID,
		SchemaID: req.SchemaID,
		Metadata: req.Metadata,
	})
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "resolution failed: "+err.Error())
		return
	}

	if flow == nil {
		httputil.WriteError(w, http.StatusNotFound, "no matching flow found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, flow)
}

func (a *API) loadLoginFlow(ctx context.Context, flowID string) (LoginFlowResponse, error) {
	var resp LoginFlowResponse
	var configStr, audienceStr, authMethodsStr, metadataStr string
	var isDefault, enabled int

	err := a.db.SQL().QueryRowContext(ctx,
		a.bindQuery(`SELECT id, COALESCE(org_id,''), COALESCE(schema_id,''), name, strategy, config,
		        CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END,
		        CASE WHEN COALESCE(enabled, true) THEN 1 ELSE 0 END, state, priority,
		        COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'),
		        COALESCE(metadata,'{}'), created_at, updated_at
		 FROM login_flows WHERE id = ?`), flowID,
	).Scan(&resp.ID, &resp.OrgID, &resp.SchemaID, &resp.Name, &resp.Strategy, &configStr,
		&isDefault, &enabled, &resp.State, &resp.Priority,
		&audienceStr, &authMethodsStr, &metadataStr,
		&resp.CreatedAt, &resp.UpdatedAt)
	if err != nil {
		return resp, err
	}

	resp.IsDefault = isDefault == 1 || isDefault != 0
	resp.Enabled = enabled == 1 || enabled != 0
	json.Unmarshal([]byte(configStr), &resp.Config)
	json.Unmarshal([]byte(audienceStr), &resp.Audience)
	json.Unmarshal([]byte(authMethodsStr), &resp.AuthMethods)
	json.Unmarshal([]byte(metadataStr), &resp.Metadata)

	return resp, nil
}
