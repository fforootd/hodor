package api

import (
	"crypto/rand"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"
)

// ProviderTemplate defines a preconfigured IDP preset.
type ProviderTemplate struct {
	ID             string            `json:"id"`
	Name           string            `json:"name"`
	Protocol       string            `json:"protocol"`
	DefaultConfig  map[string]any    `json:"default_config"`
	DefaultScopes  string            `json:"default_scopes"`
	ClaimOverrides map[string]string `json:"claim_overrides,omitempty"`
	Description    string            `json:"description"`
}

var providerTemplates = []ProviderTemplate{
	{
		ID: "google", Name: "Google", Protocol: "oidc",
		Description: "Google Workspace & consumer accounts",
		DefaultConfig: map[string]any{
			"issuer": "https://accounts.google.com",
			"scopes": "openid email profile",
		},
	},
	{
		ID: "entraid", Name: "Microsoft Entra ID", Protocol: "oidc",
		Description: "Microsoft Entra ID (Azure AD)",
		DefaultConfig: map[string]any{
			"issuer": "https://login.microsoftonline.com/{tenant_id}/v2.0",
			"scopes": "openid email profile",
		},
		ClaimOverrides: map[string]string{
			"email": "claims.preferred_username ?? claims.email",
		},
	},
	{
		ID: "gitlab", Name: "GitLab", Protocol: "oidc",
		Description: "GitLab.com or self-hosted",
		DefaultConfig: map[string]any{
			"issuer": "https://gitlab.com",
			"scopes": "openid email profile",
		},
	},
	{
		ID: "apple", Name: "Apple", Protocol: "oidc",
		Description: "Sign in with Apple",
		DefaultConfig: map[string]any{
			"issuer": "https://appleid.apple.com",
			"scopes": "openid email name",
		},
		ClaimOverrides: map[string]string{
			"display_name": "claims.name.firstName + ' ' + claims.name.lastName",
		},
	},
	{
		ID: "custom", Name: "Custom OIDC", Protocol: "oidc",
		Description: "Manual OIDC configuration",
		DefaultConfig: map[string]any{
			"issuer": "",
			"scopes": "openid email profile",
		},
	},
}

// RegisterProviderRoutes mounts provider CRUD endpoints.
func (a *API) RegisterProviderRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/providers/templates", a.listProviderTemplates)
	mux.HandleFunc("POST /v1/providers", a.requireAdmin(a.createProvider))
	mux.HandleFunc("GET /v1/providers", a.listProviders)
	mux.HandleFunc("GET /v1/providers/{id}", a.getProvider)
	mux.HandleFunc("PATCH /v1/providers/{id}", a.requireAdmin(a.updateProvider))
	mux.HandleFunc("DELETE /v1/providers/{id}", a.requireAdmin(a.deleteProvider))
}

// --- Templates ---

func (a *API) listProviderTemplates(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{"templates": providerTemplates})
}

// --- Create ---

func (a *API) createProvider(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Name           string            `json:"name"`
		Protocol       string            `json:"protocol"`
		Template       string            `json:"template"`
		Config         map[string]any    `json:"config"`
		ClaimOverrides map[string]string `json:"claim_overrides"`
		AutoRegister   *bool             `json:"auto_register"`
		Enabled        *bool             `json:"enabled"`
		DisplayOrder   int               `json:"display_order"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Name == "" {
		writeError(w, http.StatusBadRequest, "name is required")
		return
	}
	if req.Protocol == "" {
		req.Protocol = "oidc"
	}
	if req.Template == "" {
		req.Template = "custom"
	}

	// Apply template defaults if a known template is selected.
	if req.Config == nil {
		req.Config = map[string]any{}
	}
	for _, t := range providerTemplates {
		if t.ID == req.Template {
			for k, v := range t.DefaultConfig {
				if _, exists := req.Config[k]; !exists {
					req.Config[k] = v
				}
			}
			if req.ClaimOverrides == nil && len(t.ClaimOverrides) > 0 {
				req.ClaimOverrides = t.ClaimOverrides
			}
			break
		}
	}

	// Validate OIDC config.
	if req.Protocol == "oidc" {
		issuer, _ := req.Config["issuer"].(string)
		clientID, _ := req.Config["client_id"].(string)
		if issuer == "" || clientID == "" {
			writeError(w, http.StatusBadRequest, "OIDC providers require issuer and client_id in config")
			return
		}
	}

	providerID := fmt.Sprintf("prov_%s", generateShortID())

	configJSON, _ := json.Marshal(req.Config)
	overridesJSON := "{}"
	if req.ClaimOverrides != nil {
		b, _ := json.Marshal(req.ClaimOverrides)
		overridesJSON = string(b)
	}

	autoReg := true
	if req.AutoRegister != nil {
		autoReg = *req.AutoRegister
	}
	enabled := true
	if req.Enabled != nil {
		enabled = *req.Enabled
	}

	now := time.Now().UTC().Format(time.RFC3339)

	_, err := a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, created_at, updated_at)
		 VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		providerID, req.Name, req.Protocol, req.Template,
		string(configJSON), overridesJSON,
		autoReg, enabled, req.DisplayOrder,
		now, now,
	)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "create provider failed: "+err.Error())
		return
	}

	emitEventSimple(r.Context(), a.db.SQL(), "provider.created", 0, providerID, "provider", map[string]any{
		"name": req.Name, "protocol": req.Protocol, "template": req.Template,
	})
	a.bus.Signal()

	writeJSON(w, http.StatusCreated, map[string]any{
		"id":       providerID,
		"name":     req.Name,
		"protocol": req.Protocol,
		"template": req.Template,
	})
}

// --- List ---

func (a *API) listProviders(w http.ResponseWriter, r *http.Request) {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, created_at, updated_at
		 FROM providers ORDER BY display_order, name`)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var providers []map[string]any
	for rows.Next() {
		var id, name, protocol, template, config, overrides, createdAt, updatedAt string
		var autoReg, enabled bool
		var displayOrder int
		if err := rows.Scan(&id, &name, &protocol, &template, &config, &overrides, &autoReg, &enabled, &displayOrder, &createdAt, &updatedAt); err != nil {
			continue
		}

		var configMap map[string]any
		json.Unmarshal([]byte(config), &configMap)
		// Strip client_secret from list responses.
		delete(configMap, "client_secret")

		var overridesMap map[string]string
		json.Unmarshal([]byte(overrides), &overridesMap)

		providers = append(providers, map[string]any{
			"id":              id,
			"name":            name,
			"protocol":        protocol,
			"template":        template,
			"config":          configMap,
			"claim_overrides": overridesMap,
			"auto_register":   autoReg,
			"enabled":         enabled,
			"display_order":   displayOrder,
			"created_at":      createdAt,
			"updated_at":      updatedAt,
		})
	}
	if providers == nil {
		providers = []map[string]any{}
	}

	writeJSON(w, http.StatusOK, map[string]any{"providers": providers, "count": len(providers)})
}

// --- Get ---

func (a *API) getProvider(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")

	var name, protocol, template, config, overrides, createdAt, updatedAt string
	var autoReg, enabled bool
	var displayOrder int
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, created_at, updated_at
		 FROM providers WHERE id = ?`, id,
	).Scan(&name, &protocol, &template, &config, &overrides, &autoReg, &enabled, &displayOrder, &createdAt, &updatedAt)
	if err != nil {
		writeError(w, http.StatusNotFound, "provider not found")
		return
	}

	var configMap map[string]any
	json.Unmarshal([]byte(config), &configMap)

	var overridesMap map[string]string
	json.Unmarshal([]byte(overrides), &overridesMap)

	writeJSON(w, http.StatusOK, map[string]any{
		"id":              id,
		"name":            name,
		"protocol":        protocol,
		"template":        template,
		"config":          configMap,
		"claim_overrides": overridesMap,
		"auto_register":   autoReg,
		"enabled":         enabled,
		"display_order":   displayOrder,
		"created_at":      createdAt,
		"updated_at":      updatedAt,
	})
}

// --- Update ---

func (a *API) updateProvider(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")

	var req map[string]any
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	sets := []string{"updated_at = datetime('now')"}
	args := []any{}

	if v, ok := req["name"].(string); ok {
		sets = append(sets, "name = ?")
		args = append(args, v)
	}
	if v, ok := req["enabled"].(bool); ok {
		sets = append(sets, "enabled = ?")
		args = append(args, v)
	}
	if v, ok := req["auto_register"].(bool); ok {
		sets = append(sets, "auto_register = ?")
		args = append(args, v)
	}
	if v, ok := req["display_order"].(float64); ok {
		sets = append(sets, "display_order = ?")
		args = append(args, int(v))
	}
	if v, ok := req["config"].(map[string]any); ok {
		b, _ := json.Marshal(v)
		sets = append(sets, "config = ?")
		args = append(args, string(b))
	}
	if v, ok := req["claim_overrides"].(map[string]any); ok {
		b, _ := json.Marshal(v)
		sets = append(sets, "claim_overrides = ?")
		args = append(args, string(b))
	}

	args = append(args, id)
	query := fmt.Sprintf("UPDATE providers SET %s WHERE id = ?", strings.Join(sets, ", "))

	result, err := a.db.SQL().ExecContext(r.Context(), query, args...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "update failed")
		return
	}
	n, _ := result.RowsAffected()
	if n == 0 {
		writeError(w, http.StatusNotFound, "provider not found")
		return
	}

	writeJSON(w, http.StatusOK, map[string]any{"status": "updated"})
}

// --- Delete ---

func (a *API) deleteProvider(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")

	result, err := a.db.SQL().ExecContext(r.Context(), `DELETE FROM providers WHERE id = ?`, id)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	n, _ := result.RowsAffected()
	if n == 0 {
		writeError(w, http.StatusNotFound, "provider not found")
		return
	}

	// Also delete linked accounts.
	a.db.SQL().ExecContext(r.Context(), `DELETE FROM linked_accounts WHERE provider_id = ?`, id)

	emitEventSimple(r.Context(), a.db.SQL(), "provider.deleted", 0, id, "provider", map[string]any{"provider_id": id})
	a.bus.Signal()

	writeJSON(w, http.StatusOK, map[string]any{"status": "deleted"})
}

// --- Helpers ---

func generateShortID() string {
	b := make([]byte, 6)
	if _, err := rand.Read(b); err != nil {
		return fmt.Sprintf("%x", time.Now().UnixNano())[:12]
	}
	return fmt.Sprintf("%x", b)
}
