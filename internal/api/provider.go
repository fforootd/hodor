package api

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/id"
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

// providerSchemaID is the schema_id used for provider entities.
const providerSchemaID = "provider_v1"

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
	httputil.WriteJSON(w, http.StatusOK, map[string]any{"templates": providerTemplates})
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
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "name is required")
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
			httputil.WriteError(w, http.StatusBadRequest, "OIDC providers require issuer and client_id in config")
			return
		}
	}

	providerID := id.New()

	autoReg := true
	if req.AutoRegister != nil {
		autoReg = *req.AutoRegister
	}
	enabled := true
	if req.Enabled != nil {
		enabled = *req.Enabled
	}

	// Build entity data JSONB.
	overrides := map[string]string{}
	if req.ClaimOverrides != nil {
		overrides = req.ClaimOverrides
	}
	data := map[string]any{
		"protocol":        req.Protocol,
		"template":        req.Template,
		"config":          req.Config,
		"claim_overrides": overrides,
		"auto_register":   autoReg,
		"enabled":         enabled,
		"display_order":   req.DisplayOrder,
	}
	dataJSON, _ := json.Marshal(data)

	state := "active"
	if !enabled {
		state = "inactive"
	}

	now := time.Now().UTC().Format(time.RFC3339)

	_, err := a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, data, created_at, updated_at)
		 VALUES (?, '1', ?, ?, ?, ?, ?, ?, ?)`,
		providerID, req.Name, req.Name, state, providerSchemaID,
		string(dataJSON), now, now,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "create provider failed: "+err.Error())
		return
	}

	emitEventSimple(r.Context(), a.db.SQL(), "provider.created", "", providerID, "provider", map[string]any{
		"name": req.Name, "protocol": req.Protocol, "template": req.Template,
	})
	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, map[string]any{
		"id":       providerID,
		"name":     req.Name,
		"protocol": req.Protocol,
		"template": req.Template,
	})
}

// --- List ---

func (a *API) listProviders(w http.ResponseWriter, r *http.Request) {
	je := a.db.JSONExtract
	rows, err := a.db.SQL().QueryContext(r.Context(),
		fmt.Sprintf(`SELECT e.id, e.identifier, e.display_name, e.state, e.data, e.created_at, e.updated_at
		 FROM entities e
		 JOIN schemas s ON e.schema_id = s.id
		 WHERE s.type = 'provider'
		 ORDER BY CAST(%s AS INTEGER), e.display_name`, je("e.data", "display_order")))
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var providers []map[string]any
	for rows.Next() {
		var eid, identifier, state, dataStr, createdAt, updatedAt string
		var displayName string
		if err := rows.Scan(&eid, &identifier, &displayName, &state, &dataStr, &createdAt, &updatedAt); err != nil {
			continue
		}

		var data map[string]any
		json.Unmarshal([]byte(dataStr), &data)

		// Extract fields from data for backward-compatible response shape.
		configMap, _ := data["config"].(map[string]any)
		if configMap != nil {
			// Strip client_secret from list responses.
			delete(configMap, "client_secret")
		}
		overridesMap, _ := data["claim_overrides"].(map[string]any)
		autoReg, _ := data["auto_register"].(bool)
		enabled, _ := data["enabled"].(bool)
		displayOrder := 0
		if do, ok := data["display_order"].(float64); ok {
			displayOrder = int(do)
		}
		protocol, _ := data["protocol"].(string)
		template, _ := data["template"].(string)

		providers = append(providers, map[string]any{
			"id":              eid,
			"name":            identifier,
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
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}
	if providers == nil {
		providers = []map[string]any{}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"providers": providers, "count": len(providers)})
}

// --- Get ---

func (a *API) getProvider(w http.ResponseWriter, r *http.Request) {
	pid := r.PathValue("id")

	var identifier, displayName, state, dataStr, createdAt, updatedAt string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT e.identifier, e.display_name, e.state, e.data, e.created_at, e.updated_at
		 FROM entities e
		 JOIN schemas s ON e.schema_id = s.id
		 WHERE e.id = ? AND s.type = 'provider'`, pid,
	).Scan(&identifier, &displayName, &state, &dataStr, &createdAt, &updatedAt)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}

	var data map[string]any
	json.Unmarshal([]byte(dataStr), &data)

	configMap, _ := data["config"].(map[string]any)
	overridesMap, _ := data["claim_overrides"].(map[string]any)
	autoReg, _ := data["auto_register"].(bool)
	enabled, _ := data["enabled"].(bool)
	displayOrder := 0
	if do, ok := data["display_order"].(float64); ok {
		displayOrder = int(do)
	}
	protocol, _ := data["protocol"].(string)
	template, _ := data["template"].(string)

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"id":              pid,
		"name":            identifier,
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
	pid := r.PathValue("id")

	var req map[string]any
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	// Load current entity data.
	var currentDataStr string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT e.data FROM entities e
		 JOIN schemas s ON e.schema_id = s.id
		 WHERE e.id = ? AND s.type = 'provider'`, pid,
	).Scan(&currentDataStr)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}

	var data map[string]any
	json.Unmarshal([]byte(currentDataStr), &data)

	// Apply updates to the data map.
	sets := []string{"updated_at = datetime('now')"}
	if a.db.Dialect() == "postgres" {
		sets = []string{"updated_at = NOW()"}
	}
	args := []any{}

	if v, ok := req["name"].(string); ok {
		sets = append(sets, "identifier = ?", "display_name = ?")
		args = append(args, v, v)
	}
	if v, ok := req["enabled"].(bool); ok {
		data["enabled"] = v
		if v {
			sets = append(sets, "state = 'active'")
		} else {
			sets = append(sets, "state = 'inactive'")
		}
	}
	if v, ok := req["auto_register"].(bool); ok {
		data["auto_register"] = v
	}
	if v, ok := req["display_order"].(float64); ok {
		data["display_order"] = int(v)
	}
	if v, ok := req["config"].(map[string]any); ok {
		data["config"] = v
	}
	if v, ok := req["claim_overrides"].(map[string]any); ok {
		data["claim_overrides"] = v
	}

	// Serialize updated data.
	dataJSON, _ := json.Marshal(data)
	sets = append(sets, "data = ?")
	args = append(args, string(dataJSON))

	args = append(args, pid)
	query := fmt.Sprintf("UPDATE entities SET %s WHERE id = ?", strings.Join(sets, ", "))

	result, err := a.db.SQL().ExecContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	n, _ := result.RowsAffected()
	if n == 0 {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"status": "updated"})
}

// --- Delete ---

func (a *API) deleteProvider(w http.ResponseWriter, r *http.Request) {
	pid := r.PathValue("id")

	result, err := a.db.SQL().ExecContext(r.Context(),
		`DELETE FROM entities WHERE id = ? AND schema_id IN (SELECT id FROM schemas WHERE type = 'provider')`, pid)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	n, _ := result.RowsAffected()
	if n == 0 {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}

	// Also delete linked accounts.
	a.db.SQL().ExecContext(r.Context(), `DELETE FROM linked_accounts WHERE provider_id = ?`, pid)

	emitEventSimple(r.Context(), a.db.SQL(), "provider.deleted", "", pid, "provider", map[string]any{"provider_id": pid})
	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"status": "deleted"})
}

// --- Helpers ---

func generateShortID() string {
	return crypto.MustRandomHex(6)
}
