package api

import (
	"encoding/json"
	"net/http"

	"github.com/zitadel/zitadel/internal/logging"
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
	mux.HandleFunc("POST /v1/login-flows/{id}/assets", a.uploadLoginFlowAsset)
	mux.HandleFunc("POST /v1/login-flows/{id}/assets/import", a.importLoginFlowAsset)
	mux.HandleFunc("DELETE /v1/login-flows/{id}/assets/{assetId}", a.deleteLoginFlowAsset)
	mux.HandleFunc("GET /assets/login/{id}", a.serveLoginFlowAsset)
	logging.Printf("[api] registered /v1/login-flows (full CRUD + promote/archive/test/export/resolve)")
}

type LoginFlowRequest struct {
	Name        string `json:"name"`
	Strategy    string `json:"strategy,omitempty"`
	IsDefault   bool   `json:"is_default,omitempty"`
	State       string `json:"state,omitempty"`
	Priority    int    `json:"priority,omitempty"`
	Audience    any    `json:"audience,omitempty"`
	AuthMethods any    `json:"auth_methods,omitempty"`
	Config      any    `json:"config,omitempty"`
	DisplayName string `json:"display_name,omitempty"`
	Profile     any    `json:"profile,omitempty"`
}

type LoginFlowResponse struct {
	ID          string `json:"id"`
	OrgID       string `json:"org_id"`
	Name        string `json:"name"`
	Strategy    string `json:"strategy"`
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

type loginFlowScanner interface {
	Scan(dest ...any) error
}

func scanLoginFlowRow(s loginFlowScanner) (LoginFlowResponse, error) {
	var resp LoginFlowResponse
	var configStr, audienceStr, authMethodsStr, metadataStr string
	var isDefault, enabled int

	err := s.Scan(&resp.ID, &resp.OrgID, &resp.Name, &resp.Strategy, &configStr,
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
