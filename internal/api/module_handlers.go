package api

import (
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
)

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
