package api

import (
	"encoding/json"
	"errors"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"

	"github.com/zitadel/zitadel/internal/settings"
)

// RegisterSettingsRoutes mounts the hierarchical settings CRUD endpoints.
// Settings cascade: instance → org → app (ADR-009).
func (a *API) RegisterSettingsRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/settings/{type}", a.getSettings)
	mux.HandleFunc("PUT /v1/settings/{type}", a.putSettings)
	mux.HandleFunc("DELETE /v1/settings/{type}", a.deleteSettings)
}

// getSettings returns the effective (merged) settings for a type.
// Query params: ?scope=org&scope_id=X  (defaults to instance-level merged view).
// If ?raw=true, returns the raw override at the specified scope without merging.
func (a *API) getSettings(w http.ResponseWriter, r *http.Request) {
	settingsType := r.PathValue("type")
	if settingsType == "" {
		httputil.WriteError(w, http.StatusBadRequest, "settings type is required")
		return
	}

	scope := r.URL.Query().Get("scope")
	scopeID := r.URL.Query().Get("scope_id")
	raw := r.URL.Query().Get("raw") == "true"

	if raw {
		// Return unmerged override at the specified scope.
		if scope == "" {
			scope = "instance"
		}
		data, err := settings.Get(r.Context(), a.db.SQL(), settingsType, scope, scopeID)
		if errors.Is(err, settings.ErrNotFound) {
			httputil.WriteJSON(w, http.StatusOK, map[string]any{
				"type":      settingsType,
				"scope":     scope,
				"scope_id":  scopeID,
				"data":      map[string]any{},
				"inherited": true,
			})
			return
		}
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "failed to read settings")
			return
		}
		httputil.WriteJSON(w, http.StatusOK, map[string]any{
			"type":     settingsType,
			"scope":    scope,
			"scope_id": scopeID,
			"data":     data,
		})
		return
	}

	// Resolve effective settings by merging the cascade.
	orgID := scopeID
	appID := ""
	switch scope {
	case "app":
		appID = scopeID
		orgID = r.URL.Query().Get("org_id")
	case "org":
		orgID = scopeID
	}

	data, err := settings.Resolve(r.Context(), a.db.SQL(), settingsType, orgID, appID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to resolve settings")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"type":      settingsType,
		"effective": data,
		"scope":     scope,
		"scope_id":  scopeID,
	})
}

// putSettings creates or updates a settings override at a specific scope.
// Body: JSON object with settings fields to override.
// Query params: ?scope=org&scope_id=X  (defaults to instance-level).
func (a *API) putSettings(w http.ResponseWriter, r *http.Request) {
	settingsType := r.PathValue("type")
	if settingsType == "" {
		httputil.WriteError(w, http.StatusBadRequest, "settings type is required")
		return
	}

	scope := r.URL.Query().Get("scope")
	if scope == "" {
		scope = "instance"
	}
	scopeID := r.URL.Query().Get("scope_id")

	var data map[string]any
	if err := json.NewDecoder(r.Body).Decode(&data); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if len(data) == 0 {
		httputil.WriteError(w, http.StatusBadRequest, "at least one setting field is required")
		return
	}

	if err := settings.Put(r.Context(), a.db.SQL(), settingsType, scope, scopeID, data); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to save settings")
		return
	}

	// Emit event.
	emitEventTo(r.Context(), a.db.Scoped(r.Context()), "settings.updated",
		r.Header.Get("X-Identity-Id"), "", "settings",
		map[string]any{
			"type":     settingsType,
			"scope":    scope,
			"scope_id": scopeID,
		})

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"status":   "updated",
		"type":     settingsType,
		"scope":    scope,
		"scope_id": scopeID,
		"data":     data,
	})
}

// deleteSettings removes a settings override at a specific scope.
// The scope will then inherit from its parent.
func (a *API) deleteSettings(w http.ResponseWriter, r *http.Request) {
	settingsType := r.PathValue("type")
	scope := r.URL.Query().Get("scope")
	if scope == "" {
		scope = "instance"
	}
	scopeID := r.URL.Query().Get("scope_id")

	if err := settings.Delete(r.Context(), a.db.SQL(), settingsType, scope, scopeID); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to delete settings")
		return
	}

	emitEventTo(r.Context(), a.db.Scoped(r.Context()), "settings.deleted",
		r.Header.Get("X-Identity-Id"), "", "settings",
		map[string]any{
			"type":     settingsType,
			"scope":    scope,
			"scope_id": scopeID,
		})

	a.bus.Signal()

	w.WriteHeader(http.StatusNoContent)
}
