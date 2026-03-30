package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"

	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/loginflow"
)

// RegisterLoginFlowRoutes mounts all login flow management routes.
func (a *API) RegisterLoginFlowRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/login-flows", a.listLoginFlows)
	mux.HandleFunc("POST /v1/login-flows", a.createLoginFlow)
	mux.HandleFunc("GET /v1/login-flows/{id}", a.getLoginFlow)
	mux.HandleFunc("PATCH /v1/login-flows/{id}", a.updateLoginFlow)
	mux.HandleFunc("DELETE /v1/login-flows/{id}", a.deleteLoginFlow)
	mux.HandleFunc("POST /v1/login-flows/{id}/promote", a.promoteLoginFlow)
	mux.HandleFunc("POST /v1/login-flows/{id}/archive", a.archiveLoginFlow)
	mux.HandleFunc("POST /v1/login-flows/{id}/test", a.testLoginFlowAudience)
	mux.HandleFunc("GET /v1/login-flows/{id}/export", a.exportLoginFlow)
	mux.HandleFunc("POST /v1/login-flows/resolve", a.resolveLoginFlow)
	logging.Printf("[api] registered /v1/login-flows (full CRUD + promote/archive/test/export/resolve)")
}

// --- Login Flow Types ---

type LoginFlowRequest struct {
	Name        string `json:"name"`
	Preset      string `json:"preset,omitempty"`
	IsDefault   bool   `json:"is_default,omitempty"`
	State       string `json:"state,omitempty"`
	Priority    int    `json:"priority,omitempty"`
	Audience    any    `json:"audience,omitempty"`
	AuthMethods any    `json:"auth_methods,omitempty"`
	Config      any    `json:"config,omitempty"`
	// Flat fields accepted from frontend.
	DisplayName string `json:"display_name,omitempty"`
	Profile     any    `json:"profile,omitempty"`
}

type LoginFlowResponse struct {
	ID          string `json:"id"`
	OrgID       string `json:"org_id"`
	Name        string `json:"name"`
	Preset      string `json:"preset"`
	IsDefault   bool   `json:"is_default"`
	Enabled     bool   `json:"enabled"`
	State       string `json:"state"`
	Priority    int    `json:"priority"`
	Audience    any    `json:"audience"`
	AuthMethods any    `json:"auth_methods"`
	Config      any    `json:"config"`
	Metadata    any    `json:"metadata,omitempty"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

// --- Handlers ---

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

	preset := req.Preset
	if preset == "" {
		preset = "identifier_first"
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
		`INSERT INTO login_flows (id, org_id, name, preset, config, is_default, enabled, state, priority, audience, auth_methods, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?)`,
		flowID, orgID, name, preset, configJSON,
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
		Preset:      preset,
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

	query := `SELECT id, COALESCE(org_id,''), name, preset, config, COALESCE(is_default,0), COALESCE(enabled,1), state, priority,
	                  COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'),
	                  COALESCE(metadata,'{}'), created_at, updated_at
	           FROM login_flows `

	if stateFilter != "" {
		query += ` AND state = ?`
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

	// Name fallback: accept either name or display_name from frontend.
	name := req.Name
	if name == "" {
		name = req.DisplayName
	}

	// Config fallback: accept either config or profile field.
	config := req.Config
	if config == nil {
		config = req.Profile
	}

	p := newPatch()
	p.Set("name", name)
	p.Set("preset", req.Preset)
	p.Set("state", req.State)
	if req.Priority != 0 {
		p.SetInt("priority", req.Priority)
	}
	p.SetJSON("config", config)
	p.SetJSON("audience", req.Audience)
	p.SetJSON("auth_methods", req.AuthMethods)
	// is_default: always set (zero-value bool is meaningful).
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

	// Prevent deletion of the default flow — it must always exist.
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

// promoteLoginFlow advances a flow's state: draft → testing → active.
func (a *API) promoteLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	var currentState string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT state FROM login_flows WHERE id = ?`, flowID,
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
		`UPDATE login_flows SET state = ?, updated_at = ? WHERE id = ?`,
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

// archiveLoginFlow moves a flow to archived state.
func (a *API) archiveLoginFlow(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "id required")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	result, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE login_flows SET state = 'archived', updated_at = ? WHERE id = ?`,
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

// testLoginFlowAudience runs audience rules against real users.
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

// exportLoginFlow exports a flow as a catalog-compatible template JSON.
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

	// Build a catalog-compatible export.
	export := map[string]any{
		"template": map[string]any{
			"name":        resp.Name,
			"type":        "login_flow",
			"version":     "1.0",
			"description": fmt.Sprintf("Exported from flow %s", flowID),
			"tags":        []string{"login", resp.Preset},
		},
		"variables": map[string]any{},
		"payload": map[string]any{
			"name":         resp.Name,
			"preset":       resp.Preset,
			"config":       resp.Config,
			"audience":     resp.Audience,
			"auth_methods": resp.AuthMethods,
			"priority":     resp.Priority,
		},
	}

	w.Header().Set("Content-Disposition", fmt.Sprintf(`attachment; filename="%s.json"`, flowID))
	httputil.WriteJSON(w, http.StatusOK, export)
}

// resolveLoginFlow resolves the best flow for a given user context.
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

// --- Helpers ---

func (a *API) loadLoginFlow(ctx interface{ Value(any) any }, flowID string) (LoginFlowResponse, error) {
	var resp LoginFlowResponse
	var configStr, audienceStr, authMethodsStr, metadataStr string
	var isDefault, enabled int

	err := a.db.SQL().QueryRow(
		`SELECT id, COALESCE(org_id,''), name, preset, config, COALESCE(is_default,0), COALESCE(enabled,1), state, priority,
		        COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'),
		        COALESCE(metadata,'{}'), created_at, updated_at
		 FROM login_flows WHERE id = ?`, flowID,
	).Scan(&resp.ID, &resp.OrgID, &resp.Name, &resp.Preset, &configStr,
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

type loginFlowScanner interface {
	Scan(dest ...any) error
}

func scanLoginFlowRow(s loginFlowScanner) (LoginFlowResponse, error) {
	var resp LoginFlowResponse
	var configStr, audienceStr, authMethodsStr, metadataStr string
	var isDefault, enabled int

	err := s.Scan(&resp.ID, &resp.OrgID, &resp.Name, &resp.Preset, &configStr,
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

func boolToInt(b bool) int {
	if b {
		return 1
	}
	return 0
}
