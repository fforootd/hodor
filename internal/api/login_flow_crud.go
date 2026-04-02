package api

import (
	"database/sql"
	"encoding/json"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	loginflowsvc "github.com/zitadel/zitadel/internal/loginflow"
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
	orgID := r.Header.Get("X-Org-Id")
	if orgID == "" {
		orgID = "1"
	}

	record, err := a.loginFlowStore.Create(r.Context(), loginflowsvc.WriteParams{
		ID:              flowID,
		OrgID:           orgID,
		SchemaID:        schemaRec.ID,
		Name:            name,
		Strategy:        strategy,
		IsDefault:       req.IsDefault,
		Enabled:         true,
		State:           state,
		Priority:        req.Priority,
		AudienceJSON:    marshalJSON(req.Audience),
		AuthMethodsJSON: marshalJSON(req.AuthMethods),
		ConfigJSON:      configJSON,
		MetadataJSON:    "{}",
		CreatedAt:       now,
		UpdatedAt:       now,
	})
	if err != nil {
		logging.Printf("[createLoginFlow] DB insert failed: %v", err)
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create login flow")
		return
	}

	a.bus.Signal()
	httputil.WriteJSON(w, http.StatusCreated, loginFlowResponseFromRecord(record))
}

func (a *API) getLoginFlow(w http.ResponseWriter, r *http.Request) {
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
	httputil.WriteJSON(w, http.StatusOK, loginFlowResponseFromRecord(record))
}

func (a *API) listLoginFlows(w http.ResponseWriter, r *http.Request) {
	records, err := a.loginFlowStore.List(r.Context(), r.URL.Query().Get("state"))
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}

	items := make([]LoginFlowResponse, 0, len(records))
	for _, record := range records {
		items = append(items, loginFlowResponseFromRecord(record))
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
	if req.State != "" && req.State != "draft" && req.State != "testing" && req.State != "active" && req.State != "archived" {
		httputil.WriteError(w, http.StatusBadRequest, "invalid state")
		return
	}

	current, err := a.loginFlowStore.Get(r.Context(), flowID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		return
	}

	name := current.Name
	if req.Name != "" {
		name = req.Name
	} else if req.DisplayName != "" {
		name = req.DisplayName
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
	isDefault := current.IsDefault || req.IsDefault
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

	record, err := a.loginFlowStore.Update(r.Context(), loginflowsvc.WriteParams{
		ID:              flowID,
		OrgID:           current.OrgID,
		SchemaID:        schemaRec.ID,
		Name:            name,
		Strategy:        strategy,
		IsDefault:       isDefault,
		Enabled:         current.Enabled,
		State:           state,
		Priority:        priority,
		AudienceJSON:    marshalJSON(audienceValue),
		AuthMethodsJSON: marshalJSON(authMethodsValue),
		ConfigJSON:      configJSON,
		MetadataJSON:    marshalJSON(current.Metadata),
		UpdatedAt:       timeNow(),
	})
	if err != nil {
		if err == sql.ErrNoRows {
			httputil.WriteError(w, http.StatusNotFound, "login flow not found")
			return
		}
		httputil.WriteError(w, http.StatusInternalServerError, "update failed: "+err.Error())
		return
	}

	a.bus.Signal()
	httputil.WriteJSON(w, http.StatusOK, loginFlowResponseFromRecord(record))
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

	if err := a.loginFlowStore.Delete(r.Context(), flowID); err != nil {
		switch {
		case err == sql.ErrNoRows:
			httputil.WriteError(w, http.StatusNotFound, "login flow not found")
		case err.Error() == "cannot delete the default login flow":
			httputil.WriteError(w, http.StatusBadRequest, "cannot delete the default login flow — edit it instead")
		default:
			httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		}
		return
	}

	a.bus.Signal()
	w.WriteHeader(http.StatusNoContent)
}
