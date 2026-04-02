package api

import (
	"encoding/json"
	"errors"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/schema"
)

// RegisterProviderRoutes mounts provider CRUD endpoints.
func (a *API) RegisterProviderRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/providers/templates", a.listProviderTemplates)
	mux.HandleFunc("POST /v1/providers", a.requireAdmin(a.createProvider))
	mux.HandleFunc("GET /v1/providers", a.listProviders)
	mux.HandleFunc("GET /v1/providers/{id}", a.getProvider)
	mux.HandleFunc("PATCH /v1/providers/{id}", a.requireAdmin(a.updateProvider))
	mux.HandleFunc("DELETE /v1/providers/{id}", a.requireAdmin(a.deleteProvider))
}

func (a *API) listProviderTemplates(w http.ResponseWriter, r *http.Request) {
	if a.catalog == nil {
		httputil.WriteJSON(w, http.StatusOK, map[string]any{"templates": []any{}})
		return
	}

	templates := a.catalog.List("provider", "")
	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"templates":   templates,
		"deprecated":  true,
		"description": "Use /v1/catalog?type=provider for the primary marketplace surface.",
	})
}

func (a *API) createProvider(w http.ResponseWriter, r *http.Request) {
	req, err := decodeProviderBody(r)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	req.OrgID = firstNonEmptyString(r.Header.Get("X-Org-Id"), "1")
	req, err = a.prepareProviderWrite(r, req)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	repo := providers.NewRepository(a.db.SQL(), a.db.Dialect())
	providerID, err := repo.Create(r.Context(), id.New(), req)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "create provider failed: "+err.Error())
		return
	}

	created, err := repo.Get(r.Context(), providerID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "load created provider failed")
		return
	}

	scoped := a.db.Scoped(r.Context())
	stx, _ := scoped.BeginTx(r.Context(), nil)
	if stx != nil {
		emitEvent(r.Context(), stx, "provider.created", "", providerID, "provider", map[string]any{
			"display_name": created.DisplayName,
			"kind":         created.Kind,
			"protocol":     created.Protocol,
		})
		_ = stx.Commit()
	}
	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, created)
}

func (a *API) listProviders(w http.ResponseWriter, r *http.Request) {
	repo := providers.NewRepository(a.db.SQL(), a.db.Dialect())
	items, err := repo.List(r.Context())
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}

	providerList := make([]providers.Provider, 0, len(items))
	for _, prov := range items {
		providerList = append(providerList, providers.Redacted(prov))
	}
	httputil.WriteJSON(w, http.StatusOK, map[string]any{"providers": providerList, "count": len(providerList)})
}

func (a *API) getProvider(w http.ResponseWriter, r *http.Request) {
	repo := providers.NewRepository(a.db.SQL(), a.db.Dialect())
	prov, err := repo.Get(r.Context(), r.PathValue("id"))
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}
	httputil.WriteJSON(w, http.StatusOK, prov)
}

func (a *API) updateProvider(w http.ResponseWriter, r *http.Request) {
	repo := providers.NewRepository(a.db.SQL(), a.db.Dialect())
	current, err := repo.Get(r.Context(), r.PathValue("id"))
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}

	raw, err := decodeProviderRawBody(r)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	next, err := mergeProviderPatch(*current, raw)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	next.OrgID = current.OrgID
	next, err = a.prepareProviderWrite(r, next)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := repo.Save(r.Context(), next); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}

	updated, err := repo.Get(r.Context(), next.ID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "load updated provider failed")
		return
	}
	httputil.WriteJSON(w, http.StatusOK, updated)
}

func (a *API) deleteProvider(w http.ResponseWriter, r *http.Request) {
	repo := providers.NewRepository(a.db.SQL(), a.db.Dialect())
	rows, err := repo.Delete(r.Context(), r.PathValue("id"))
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	if rows == 0 {
		httputil.WriteError(w, http.StatusNotFound, "provider not found")
		return
	}

	scoped := a.db.Scoped(r.Context())
	stx, _ := scoped.BeginTx(r.Context(), nil)
	if stx != nil {
		emitEvent(r.Context(), stx, "provider.deleted", "", r.PathValue("id"), "provider", map[string]any{"provider_id": r.PathValue("id")})
		_ = stx.Commit()
	}
	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"status": "deleted"})
}

func decodeProviderBody(r *http.Request) (providers.Provider, error) {
	raw, err := decodeProviderRawBody(r)
	if err != nil {
		return providers.Provider{}, err
	}
	return providerFromMap(raw)
}

func decodeProviderRawBody(r *http.Request) (map[string]any, error) {
	var raw map[string]any
	if err := json.NewDecoder(r.Body).Decode(&raw); err != nil {
		return nil, errors.New("invalid JSON body")
	}
	return raw, nil
}

//nolint:gocyclo // Backward-compatible decoder for multiple legacy provider payload shapes.
func providerFromMap(raw map[string]any) (providers.Provider, error) {
	prov := providers.Provider{
		Connection: map[string]any{},
		Mapping: providers.Mapping{
			Claims: map[string]string{},
		},
		Session: map[string]any{},
		UI:      map[string]any{},
		Enabled: true,
	}

	if v, ok := raw["display_name"].(string); ok {
		prov.DisplayName = strings.TrimSpace(v)
	} else if v, ok := raw["name"].(string); ok {
		prov.DisplayName = strings.TrimSpace(v)
	}
	if v, ok := raw["kind"].(string); ok {
		prov.Kind = strings.TrimSpace(v)
	} else if v, ok := raw["template"].(string); ok {
		prov.Kind = strings.TrimSpace(v)
	}
	if v, ok := raw["protocol"].(string); ok {
		prov.Protocol = strings.TrimSpace(v)
	} else if v, ok := raw["type"].(string); ok {
		prov.Protocol = strings.TrimSpace(v)
	}
	if v, ok := raw["connection"].(map[string]any); ok {
		prov.Connection = v
	} else if v, ok := raw["config"].(map[string]any); ok {
		prov.Connection = v
	}
	if v, ok := raw["mapping"].(map[string]any); ok {
		if claims, ok := stringMapFromAny(v["claims"]); ok {
			prov.Mapping.Claims = claims
		}
	} else if v, ok := raw["claim_overrides"].(map[string]any); ok {
		if claims, ok := stringMapFromAny(v); ok {
			prov.Mapping.Claims = claims
		}
	}
	if v, ok := raw["target"].(map[string]any); ok {
		if schemaType, ok := v["schema_type"].(string); ok {
			prov.Target.SchemaType = strings.TrimSpace(schemaType)
		}
		if schemaID, ok := v["schema_id"].(string); ok {
			prov.Target.SchemaID = strings.TrimSpace(schemaID)
		}
	} else if v, ok := raw["schema_id"].(string); ok {
		prov.SchemaID = strings.TrimSpace(v)
	}
	if v, ok := raw["linking"].(map[string]any); ok {
		if mode, ok := v["mode"].(string); ok {
			prov.Linking.Mode = strings.TrimSpace(mode)
		}
		if matchBy, ok := v["match_by"].(string); ok {
			prov.Linking.MatchBy = strings.TrimSpace(matchBy)
		}
	} else {
		if v, ok := raw["auto_register"].(bool); ok && !v {
			prov.Linking.Mode = providers.LinkModeLinkOnly
		}
	}
	if v, ok := raw["session"].(map[string]any); ok {
		prov.Session = v
	}
	if v, ok := raw["ui"].(map[string]any); ok {
		prov.UI = v
	}
	if v, ok := raw["display_order"].(float64); ok {
		prov.UI["display_order"] = int(v)
	}
	if v, ok := raw["enabled"].(bool); ok {
		prov.Enabled = v
	}
	if v, ok := raw["catalog_ref"].(map[string]any); ok {
		if templateID, ok := v["template_id"].(string); ok {
			prov.CatalogRef.TemplateID = strings.TrimSpace(templateID)
		}
		if version, ok := v["template_version"].(string); ok {
			prov.CatalogRef.TemplateVersion = strings.TrimSpace(version)
		}
		if official, ok := v["official"].(bool); ok {
			prov.CatalogRef.Official = official
		}
		if capabilities, ok := providerStringSliceFromAny(v["capabilities"]); ok {
			prov.CatalogRef.Capabilities = capabilities
		}
		if logoURL, ok := v["logo_url"].(string); ok {
			prov.CatalogRef.LogoURL = strings.TrimSpace(logoURL)
		}
		if docsURL, ok := v["docs_url"].(string); ok {
			prov.CatalogRef.DocsURL = strings.TrimSpace(docsURL)
		}
	}

	if prov.DisplayName == "" {
		return providers.Provider{}, errors.New("display_name is required")
	}
	prov = providers.Normalize(prov)
	if err := validateProvider(prov); err != nil {
		return providers.Provider{}, err
	}
	return prov, nil
}

//nolint:gocyclo // Backward-compatible patch merger for multiple legacy provider payload shapes.
func mergeProviderPatch(current providers.Provider, raw map[string]any) (providers.Provider, error) {
	next := current
	if value, ok := raw["display_name"].(string); ok {
		next.DisplayName = strings.TrimSpace(value)
	} else if value, ok := raw["name"].(string); ok {
		next.DisplayName = strings.TrimSpace(value)
	}
	if value, ok := raw["kind"].(string); ok {
		next.Kind = strings.TrimSpace(value)
	} else if value, ok := raw["template"].(string); ok {
		next.Kind = strings.TrimSpace(value)
	}
	if value, ok := raw["protocol"].(string); ok {
		next.Protocol = strings.TrimSpace(value)
	} else if value, ok := raw["type"].(string); ok {
		next.Protocol = strings.TrimSpace(value)
	}
	if value, ok := raw["connection"].(map[string]any); ok {
		next.Connection = value
	} else if value, ok := raw["config"].(map[string]any); ok {
		next.Connection = value
	}
	if value, ok := raw["mapping"].(map[string]any); ok {
		if claims, ok := stringMapFromAny(value["claims"]); ok {
			next.Mapping.Claims = claims
		}
	} else if value, ok := raw["claim_overrides"].(map[string]any); ok {
		if claims, ok := stringMapFromAny(value); ok {
			next.Mapping.Claims = claims
		}
	}
	if value, ok := raw["target"].(map[string]any); ok {
		if schemaType, ok := value["schema_type"].(string); ok {
			next.Target.SchemaType = strings.TrimSpace(schemaType)
		}
		if schemaID, ok := value["schema_id"].(string); ok {
			next.Target.SchemaID = strings.TrimSpace(schemaID)
		}
	} else if value, ok := raw["schema_id"].(string); ok {
		next.SchemaID = strings.TrimSpace(value)
	}
	if value, ok := raw["linking"].(map[string]any); ok {
		if mode, ok := value["mode"].(string); ok {
			next.Linking.Mode = strings.TrimSpace(mode)
		}
		if matchBy, ok := value["match_by"].(string); ok {
			next.Linking.MatchBy = strings.TrimSpace(matchBy)
		}
	} else if value, ok := raw["auto_register"].(bool); ok && !value {
		next.Linking.Mode = providers.LinkModeLinkOnly
	}
	if value, ok := raw["session"].(map[string]any); ok {
		next.Session = value
	}
	if value, ok := raw["ui"].(map[string]any); ok {
		next.UI = value
	}
	if value, ok := raw["display_order"].(float64); ok {
		if next.UI == nil {
			next.UI = map[string]any{}
		}
		next.UI["display_order"] = int(value)
	}
	if value, ok := raw["enabled"].(bool); ok {
		next.Enabled = value
	}
	if value, ok := raw["catalog_ref"].(map[string]any); ok {
		if templateID, ok := value["template_id"].(string); ok {
			next.CatalogRef.TemplateID = strings.TrimSpace(templateID)
		}
		if version, ok := value["template_version"].(string); ok {
			next.CatalogRef.TemplateVersion = strings.TrimSpace(version)
		}
		if official, ok := value["official"].(bool); ok {
			next.CatalogRef.Official = official
		}
		if capabilities, ok := providerStringSliceFromAny(value["capabilities"]); ok {
			next.CatalogRef.Capabilities = capabilities
		}
		if logoURL, ok := value["logo_url"].(string); ok {
			next.CatalogRef.LogoURL = strings.TrimSpace(logoURL)
		}
		if docsURL, ok := value["docs_url"].(string); ok {
			next.CatalogRef.DocsURL = strings.TrimSpace(docsURL)
		}
	}
	next = providers.Normalize(next)
	if err := validateProvider(next); err != nil {
		return providers.Provider{}, err
	}
	return next, nil
}

func validateProvider(prov providers.Provider) error {
	if prov.Protocol == "oidc" {
		issuer, _ := prov.Connection["issuer"].(string)
		clientID, _ := prov.Connection["client_id"].(string)
		if issuer == "" || clientID == "" {
			return errors.New("OIDC providers require connection.issuer and connection.client_id")
		}
	}
	if prov.Protocol == "oauth2" {
		clientID, _ := prov.Connection["client_id"].(string)
		if clientID == "" {
			return errors.New("OAuth2 providers require connection.client_id")
		}
	}
	return nil
}

func stringMapFromAny(value any) (map[string]string, bool) {
	raw, ok := value.(map[string]any)
	if !ok {
		return nil, false
	}
	out := make(map[string]string, len(raw))
	for key, item := range raw {
		str, ok := item.(string)
		if !ok {
			continue
		}
		out[key] = str
	}
	return out, true
}

func providerStringSliceFromAny(value any) ([]string, bool) {
	items, ok := value.([]any)
	if !ok {
		return nil, false
	}
	out := make([]string, 0, len(items))
	for _, item := range items {
		str, ok := item.(string)
		if !ok {
			continue
		}
		out = append(out, str)
	}
	return out, true
}

func generateShortID() string {
	return id.New()[:12]
}

func (a *API) prepareProviderWrite(r *http.Request, prov providers.Provider) (providers.Provider, error) {
	schemaRec, err := a.resolveResourceSchema(r.Context(), "provider", prov.SchemaID)
	if err != nil {
		return providers.Provider{}, err
	}
	targetSchemaID, targetSchemaType, err := providers.ResolveTargetSchema(r.Context(), a.db.SQL(), prov.Target, a.db.Dialect())
	if err != nil {
		return providers.Provider{}, err
	}
	prov.Target.SchemaID = targetSchemaID
	prov.Target.SchemaType = targetSchemaType

	data, err := providerSchemaData(prov)
	if err != nil {
		return providers.Provider{}, err
	}
	if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
		return providers.Provider{}, err
	}

	prov.SchemaID = schemaRec.ID
	return providers.Normalize(prov), nil
}

func providerSchemaData(prov providers.Provider) (map[string]any, error) {
	return providers.SchemaData(prov)
}
