package api

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/loginflow"
)

func (a *API) promoteLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	currentState, nextState, err := a.loginFlowStore.Promote(r.Context(), flowID)
	if err != nil {
		switch {
		case err == sql.ErrNoRows:
			httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		case err != nil:
			httputil.WriteError(w, http.StatusBadRequest, err.Error())
		}
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

	if err := a.loginFlowStore.Archive(r.Context(), flowID); err != nil {
		if err == sql.ErrNoRows {
			httputil.WriteError(w, http.StatusNotFound, "login flow not found")
			return
		}
		httputil.WriteError(w, http.StatusInternalServerError, "archive failed")
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

	record, err := a.loginFlowStore.Get(r.Context(), flowID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}
	resp := loginFlowResponseFromRecord(record)

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
