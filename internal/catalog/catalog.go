// Package catalog implements the template catalog (ADR-015).
// It provides an embedded-first catalog with optional remote overlay.
// Resolution: embedded (always) → DB cache (offline restarts) → remote (fresh updates).
package catalog

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"embed"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/resourcedata"
	"github.com/zitadel/zitadel/internal/schema"
	"strings"
	"sync"
	"time"

	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/id"
)

//go:embed embedded/catalog.json
var embeddedCatalog string

//go:embed embedded/templates/*/*.json
var embeddedTemplates embed.FS

// Index is the catalog manifest listing all available templates.
type Index struct {
	Version   string     `json:"version"`
	Templates []Template `json:"templates"`
}

// Template is a single catalog entry (metadata only, not the payload).
type Template struct {
	ID           string   `json:"id"`
	Type         string   `json:"type"` // action | provider | authorization | schema
	Name         string   `json:"name"`
	Description  string   `json:"description"`
	Kind         string   `json:"kind,omitempty"`
	Protocol     string   `json:"protocol,omitempty"`
	Official     bool     `json:"official,omitempty"`
	Tags         []string `json:"tags"`
	Capabilities []string `json:"capabilities,omitempty"`
	LogoURL      string   `json:"logo_url,omitempty"`
	DocsURL      string   `json:"docs_url,omitempty"`
	Version      string   `json:"version"`
	Author       string   `json:"author,omitempty"`
	Path         string   `json:"path"`   // relative path to template JSON
	Source       string   `json:"source"` // "embedded" | "remote" | "cached"
}

// TemplatePayload is the full template content including variables and installable payload.
type TemplatePayload struct {
	Type        string         `json:"type"`
	Version     string         `json:"version"`
	Name        string         `json:"name"`
	Description string         `json:"description,omitempty"`
	Variables   map[string]Var `json:"variables,omitempty"`
	Payload     map[string]any `json:"payload"`
}

// Var describes a user-fillable variable in a template.
type Var struct {
	Type        string `json:"type"`
	Description string `json:"description"`
	Default     any    `json:"default,omitempty"`
	Sensitive   bool   `json:"sensitive,omitempty"`
}

// Service is the catalog manager. It loads embedded templates on creation
// and optionally merges remote templates in the background.
type Service struct {
	embedded *Index
	remote   *Index
	merged   *Index
	mu       sync.RWMutex
	db       *sql.DB
	dialect  string
	cfg      config.CatalogConfig
}

// New creates a new catalog service. The embedded catalog is loaded synchronously
// (always available). Remote refresh happens asynchronously via StartBackground().
func New(cfg config.CatalogConfig, db *sql.DB, dialect ...string) *Service {
	s := &Service{
		db:      db,
		dialect: catalogDialect(dialect),
		cfg:     cfg,
	}

	// Load embedded catalog (always succeeds — compiled into binary).
	idx, err := loadEmbeddedIndex()
	if err != nil {
		logging.Printf("[catalog] failed to load embedded catalog: %v", err)
		idx = &Index{Version: "0.0", Templates: nil}
	}
	s.embedded = idx
	s.merged = idx

	// Try to load DB-cached remote overlay (fast, no network).
	if cached := s.loadFromDBCache(); cached != nil {
		s.remote = cached
		s.merged = merge(s.embedded, cached)
		logging.Printf("[catalog] loaded %d cached remote templates", len(cached.Templates))
	}

	return s
}

// EmbeddedCount returns the number of embedded templates.
func (s *Service) EmbeddedCount() int {
	return len(s.embedded.Templates)
}

// CanRefresh reports whether a refresh source is configured.
func (s *Service) CanRefresh() bool {
	return s.cfg.URL != "" || s.cfg.LocalPath != ""
}

// List returns all templates, optionally filtered by type and/or tags.
func (s *Service) List(typeFilter string, tagFilter string) []Template {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var results []Template
	for _, t := range s.merged.Templates {
		if typeFilter != "" && t.Type != typeFilter {
			continue
		}
		if tagFilter != "" && !hasTag(t.Tags, tagFilter) {
			continue
		}
		results = append(results, t)
	}
	return results
}

// Get returns the full template payload (with variables) for the given ID.
func (s *Service) Get(templateID string) (*TemplatePayload, *Template, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Find template metadata.
	var tpl *Template
	for i, t := range s.merged.Templates {
		if t.ID == templateID {
			tpl = &s.merged.Templates[i]
			break
		}
	}
	if tpl == nil {
		return nil, nil, fmt.Errorf("template %q not found", templateID)
	}

	// Load the template payload.
	payload, err := s.loadTemplatePayload(tpl)
	if err != nil {
		return nil, tpl, fmt.Errorf("load template %q: %w", templateID, err)
	}

	return payload, tpl, nil
}

// Install creates an entity from a template with variable substitution.
// The entity carries a `_catalog` metadata block for origin tracking (ADR-015 §8).
func (s *Service) Install(ctx context.Context, templateID string, variables map[string]any) (string, error) {
	payload, tpl, err := s.Get(templateID)
	if err != nil {
		return "", err
	}

	// Substitute variables into the payload.
	resolved := substituteVars(payload.Payload, payload.Variables, variables)

	catalogMeta := map[string]any{
		"template_id":      templateID,
		"template_version": tpl.Version,
		"installed_at":     "now", // replaced by DB
		"auto_upgrade":     false,
	}

	resourceID := id.New()
	displayName := resourcedata.StringFromAny(resolved["display_name"])
	if displayName == "" {
		displayName = payload.Name
	}

	orgID := "1"
	if ctxOrg, ok := ctx.Value("org_id").(string); ok && ctxOrg != "" {
		orgID = ctxOrg
	}

	switch payload.Type {
	case "provider":
		schemaRec, schemaErr := s.resolveSchema(ctx, "provider", "")
		if schemaErr != nil {
			return "", schemaErr
		}
		prov, convErr := providerFromResolvedPayload(resolved, tpl)
		if convErr != nil {
			return "", convErr
		}
		prov.OrgID = orgID
		prov.SchemaID = schemaRec.ID
		targetSchemaID, targetSchemaType, targetErr := providers.ResolveTargetSchema(ctx, s.db, prov.Target, s.dialect)
		if targetErr != nil {
			return "", targetErr
		}
		prov.Target.SchemaID = targetSchemaID
		prov.Target.SchemaType = targetSchemaType
		data, dataErr := providers.SchemaData(prov)
		if dataErr != nil {
			return "", dataErr
		}
		if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
			return "", err
		}
		storedProvider, payloadErr := schema.ObjectMap(prov)
		if payloadErr != nil {
			return "", payloadErr
		}
		prov.CatalogMeta = buildCatalogMeta(storedProvider, catalogMeta)
		repo := providers.NewRepository(s.db, s.dialect)
		resourceID, err = repo.Create(ctx, resourceID, prov)
	case "action":
		schemaRec, schemaErr := s.resolveSchema(ctx, "action", "")
		if schemaErr != nil {
			return "", schemaErr
		}
		hook := resourcedata.FirstNonEmptyString(resourcedata.StringFromAny(resolved["hook"]), "on_event")
		actionType := resourcedata.FirstNonEmptyString(resourcedata.StringFromAny(resolved["action_type"]), "expr")
		triggerExpr := resourcedata.FirstNonEmptyString(resourcedata.StringFromAny(resolved["trigger"]), "true")
		priority := intFromAny(resolved["priority"], 0)
		enabled := boolFromAny(resolved["enabled"], true)
		failOpen := boolFromAny(resolved["fail_open"], false)
		timeoutMS := intFromAny(resolved["timeout_ms"], 5000)
		actionMetadata := resourcedata.StripKeys(cloneMap(resolved),
			"display_name", "hook", "action_type", "trigger", "config", "priority", "enabled", "fail_open", "timeout_ms",
		)
		actionData, configMap, buildErr := resourcedata.BuildActionSchemaData(displayName, hook, actionType, triggerExpr, priority, enabled, resolved["config"], actionMetadata)
		if buildErr != nil {
			return "", buildErr
		}
		actionData["fail_open"] = failOpen
		actionData["timeout_ms"] = timeoutMS
		if err := schema.ValidateData(schemaRec.Schema, actionData); err != nil {
			return "", err
		}
		configJSON := resourcedata.EncodeObjectString(configMap)
		storedAction := withCatalogMeta(actionData, catalogMeta)
		metadataJSON := resourcedata.EncodeObjectString(storedAction)
		_, err = s.db.ExecContext(ctx,
			`INSERT INTO actions (id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, timeout_ms, schema_id, metadata, created_at, updated_at)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			resourceID, orgID, displayName, hook, actionType, triggerExpr,
			configJSON, priority, enabled, failOpen, timeoutMS, schemaRec.ID, metadataJSON, timeNow(), timeNow(),
		)
	case "login_flow":
		schemaRec, schemaErr := s.resolveSchema(ctx, "login_flow", "")
		if schemaErr != nil {
			return "", schemaErr
		}
		strategy := resourcedata.FirstNonEmptyString(resourcedata.StringFromAny(resolved["strategy"]), "identifier_first")
		state := resourcedata.FirstNonEmptyString(resourcedata.StringFromAny(resolved["state"]), "active")
		priority := intFromAny(resolved["priority"], 10)
		isDefault := boolFromAny(resolved["is_default"], false)
		audience := mapValueOrEmpty(resolved["audience"])
		authMethods := mapValueOrEmpty(resolved["auth_methods"])
		flowMetadata := resourcedata.StripKeys(cloneMap(resolved),
			"display_name", "strategy", "is_default", "state", "priority", "audience", "auth_methods", "config",
		)
		flowData, configMap, buildErr := resourcedata.BuildLoginFlowSchemaData(displayName, strategy, isDefault, state, priority, audience, authMethods, resolved["config"], flowMetadata)
		if buildErr != nil {
			return "", buildErr
		}
		if err := schema.ValidateData(schemaRec.Schema, flowData); err != nil {
			return "", err
		}
		storedFlow := withCatalogMeta(flowData, catalogMeta)
		_, err = s.db.ExecContext(ctx,
			`INSERT INTO login_flows (id, org_id, name, strategy, auth_methods, config, enabled, state, priority, audience, schema_id, metadata, created_at, updated_at)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			resourceID, orgID, displayName, strategy,
			resourcedata.EncodeObjectString(authMethods), resourcedata.EncodeObjectString(configMap), true, state, priority,
			resourcedata.EncodeObjectString(audience), schemaRec.ID, resourcedata.EncodeObjectString(storedFlow), timeNow(), timeNow(),
		)
	default:
		schemaType := tpl.Type
		if schemaType == "authorization" {
			schemaType = "fga_model"
		}
		schemaRec, schemaErr := s.resolveSchema(ctx, schemaType, "")
		if schemaErr != nil && schema.IsUserSchemaType(schemaType) {
			return "", schemaErr
		}
		schemaID := ""
		if schemaRec != nil {
			schemaID = schemaRec.ID
		}
		metadataJSON := resourcedata.EncodeObjectString(withCatalogMeta(resourcedata.CloneObjectMap(resolved), catalogMeta))
		_, err = s.db.ExecContext(ctx,
			`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
			 VALUES (?, ?, ?, ?, 'human', 'active', ?, ?, ?, ?)`,
			resourceID, orgID, templateID, displayName, schemaID, metadataJSON, timeNow(), timeNow(),
		)
	}
	if err != nil {
		return "", fmt.Errorf("insert resource: %w", err)
	}

	return resourceID, nil
}

func (s *Service) resolveSchema(ctx context.Context, schemaType, schemaID string) (*schema.SchemaRecord, error) {
	return schema.ResolveSchemaForType(ctx, s.db, schemaType, schemaID, s.dialect)
}

func (s *Service) upsertCache(ctx context.Context, key, data string) error {
	_, err := s.db.ExecContext(ctx,
		`INSERT INTO cache (namespace, key, data, fetched_at)
		 VALUES ('catalog', ?, ?, ?)
		 ON CONFLICT(namespace, key) DO UPDATE SET data = excluded.data, fetched_at = excluded.fetched_at`,
		key, data, timeNow(),
	)
	return err
}

func catalogDialect(dialect []string) string {
	if len(dialect) == 0 {
		return "sqlite"
	}
	switch strings.TrimSpace(dialect[0]) {
	case "postgres":
		return "postgres"
	default:
		return "sqlite"
	}
}

func timeNow() string {
	return time.Now().UTC().Format(time.RFC3339)
}

func boolToInt(value bool) int {
	if value {
		return 1
	}
	return 0
}

func boolFromAny(value any, fallback bool) bool {
	switch typed := value.(type) {
	case bool:
		return typed
	case int:
		return typed != 0
	case int64:
		return typed != 0
	case float64:
		return typed != 0
	default:
		return fallback
	}
}

func intFromAny(value any, fallback int) int {
	switch typed := value.(type) {
	case int:
		return typed
	case int32:
		return int(typed)
	case int64:
		return int(typed)
	case float64:
		return int(typed)
	default:
		return fallback
	}
}

func mapValueOrEmpty(value any) map[string]any {
	mapped, err := resourcedata.ObjectMapOrEmpty(value)
	if err != nil {
		return map[string]any{}
	}
	return mapped
}

func cloneMap(input map[string]any) map[string]any {
	return resourcedata.CloneObjectMap(input)
}

func withCatalogMeta(data map[string]any, base map[string]any) map[string]any {
	out := cloneMap(data)
	out["_catalog"] = buildCatalogMeta(out, base)
	return out
}

func buildCatalogMeta(data map[string]any, base map[string]any) map[string]any {
	meta := cloneMap(base)
	meta["installed_hash"] = catalogHash(data)
	return meta
}

func catalogHash(data map[string]any) string {
	out := cloneMap(data)
	delete(out, "_catalog")
	raw, err := json.Marshal(out)
	if err != nil {
		return ""
	}
	return computeHash(raw)
}

// CatalogState returns the lifecycle state of an entity based on its _catalog metadata.
// Returns "custom" if no _catalog block exists, "linked" if content hash matches,
// or "forked" if it has diverged.
func CatalogState(data map[string]any) string {
	catalogMeta, ok := data["_catalog"].(map[string]any)
	if !ok {
		return "custom"
	}

	installedHash, _ := catalogMeta["installed_hash"].(string)
	if installedHash == "" {
		return "custom"
	}

	// Compute current hash (excluding _catalog block itself).
	dataCopy := make(map[string]any)
	for k, v := range data {
		if k != "_catalog" {
			dataCopy[k] = v
		}
	}
	currentJSON, _ := json.Marshal(dataCopy)
	currentHash := computeHash(currentJSON)

	if currentHash == installedHash {
		return "linked"
	}
	return "forked"
}

// SetRemote updates the remote overlay (called by the remote fetcher).
func (s *Service) SetRemote(idx *Index) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.remote = idx
	s.merged = merge(s.embedded, idx)
}

// loadTemplatePayload reads the template JSON content.
func (s *Service) loadTemplatePayload(tpl *Template) (*TemplatePayload, error) {
	var data []byte
	var err error

	switch tpl.Source {
	case "remote", "cached":
		// Try DB cache first.
		var cached string
		err = s.db.QueryRow(
			`SELECT data FROM cache WHERE namespace = 'catalog' AND key = ?`, "template:"+tpl.ID,
		).Scan(&cached)
		if err == nil {
			data = []byte(cached)
		} else {
			// Fall back to embedded if remote template can't be loaded.
			data, err = embeddedTemplates.ReadFile("embedded/" + tpl.Path)
			if err != nil {
				return nil, fmt.Errorf("template %q not in cache or embedded: %w", tpl.ID, err)
			}
		}
	default: // "embedded" or empty
		data, err = embeddedTemplates.ReadFile("embedded/" + tpl.Path)
		if err != nil {
			return nil, fmt.Errorf("read embedded %s: %w", tpl.Path, err)
		}
	}

	var payload TemplatePayload
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil, fmt.Errorf("parse template: %w", err)
	}
	return &payload, nil
}

// loadFromDBCache loads the cached remote index from the database.
func (s *Service) loadFromDBCache() *Index {
	if s.db == nil {
		return nil
	}

	var data string
	err := s.db.QueryRow(
		`SELECT data FROM cache WHERE namespace = 'catalog' AND key = 'remote_index'`,
	).Scan(&data)
	if err != nil {
		return nil
	}

	var idx Index
	if err := json.Unmarshal([]byte(data), &idx); err != nil {
		return nil
	}
	// Mark all as cached.
	for i := range idx.Templates {
		idx.Templates[i].Source = "cached"
	}
	return &idx
}

// CacheToDB stores the remote index in the database for offline restarts.
func (s *Service) CacheToDB(idx *Index) {
	if s.db == nil {
		return
	}

	data, err := json.Marshal(idx)
	if err != nil {
		logging.Printf("[catalog] failed to marshal index for cache: %v", err)
		return
	}
	if err := s.upsertCache(context.Background(), "remote_index", string(data)); err != nil {
		logging.Printf("[catalog] failed to cache index: %v", err)
	}
}

// loadEmbeddedIndex parses the compiled-in catalog.json.
func loadEmbeddedIndex() (*Index, error) {
	var idx Index
	if err := json.Unmarshal([]byte(embeddedCatalog), &idx); err != nil {
		return nil, fmt.Errorf("parse embedded catalog: %w", err)
	}
	for i := range idx.Templates {
		idx.Templates[i].Source = "embedded"
	}
	return &idx, nil
}

// merge overlays remote templates on top of embedded ones.
// Embedded templates are never removed. Remote can add new or upgrade existing.
func merge(embedded, remote *Index) *Index {
	if remote == nil {
		return embedded
	}

	result := &Index{
		Version:   embedded.Version,
		Templates: make([]Template, len(embedded.Templates)),
	}
	copy(result.Templates, embedded.Templates)

	// Build lookup by ID.
	idxByID := make(map[string]int)
	for i, t := range result.Templates {
		idxByID[t.ID] = i
	}

	for _, rt := range remote.Templates {
		if i, exists := idxByID[rt.ID]; exists {
			// Remote has newer version → upgrade.
			if rt.Version > result.Templates[i].Version {
				result.Templates[i] = rt
			}
		} else {
			// New template from remote → add.
			result.Templates = append(result.Templates, rt)
		}
	}

	return result
}

// substituteVars replaces {{var}} placeholders in the payload with values.
func substituteVars(payload map[string]any, defs map[string]Var, values map[string]any) map[string]any {
	// Build effective values: defaults + user overrides.
	effective := make(map[string]any)
	for k, v := range defs {
		if v.Default != nil {
			effective[k] = v.Default
		}
	}
	for k, v := range values {
		effective[k] = v
	}

	// Deep-substitute.
	result := make(map[string]any)
	for k, v := range payload {
		result[k] = substituteValue(v, effective)
	}
	return result
}

// substituteValue recursively substitutes {{var}} in strings, maps, and slices.
func substituteValue(v any, vars map[string]any) any {
	switch val := v.(type) {
	case string:
		return substituteString(val, vars)
	case map[string]any:
		result := make(map[string]any)
		for k, sv := range val {
			result[k] = substituteValue(sv, vars)
		}
		return result
	case []any:
		result := make([]any, len(val))
		for i, sv := range val {
			result[i] = substituteValue(sv, vars)
		}
		return result
	default:
		return v
	}
}

// substituteString replaces all {{key}} occurrences in a string.
// If the entire string is a single {{key}} and the value is non-string, return the raw value
// (preserves integers, booleans, etc.).
func substituteString(s string, vars map[string]any) any {
	trimmed := strings.TrimSpace(s)
	if strings.HasPrefix(trimmed, "{{") && strings.HasSuffix(trimmed, "}}") {
		key := strings.TrimSuffix(strings.TrimPrefix(trimmed, "{{"), "}}")
		key = strings.TrimSpace(key)
		if val, ok := vars[key]; ok {
			return val
		}
	}

	result := s
	for k, v := range vars {
		placeholder := "{{" + k + "}}"
		result = strings.ReplaceAll(result, placeholder, fmt.Sprintf("%v", v))
	}
	return result
}

// hasTag checks if a template has the given tag.
func hasTag(tags []string, tag string) bool {
	for _, t := range tags {
		if t == tag {
			return true
		}
	}
	return false
}

func providerFromResolvedPayload(resolved map[string]any, tpl *Template) (providers.Provider, error) {
	prov := providers.Provider{
		DisplayName: fmt.Sprintf("%v", resolved["display_name"]),
		Kind:        tpl.Kind,
		Protocol:    firstNonEmptyString(fmt.Sprintf("%v", resolved["protocol"]), tpl.Protocol),
		Enabled:     true,
	}

	if connection, ok := resolved["connection"].(map[string]any); ok {
		prov.Connection = connection
	} else {
		prov.Connection = map[string]any{}
		for _, key := range []string{"issuer", "authorization_url", "token_url", "userinfo_url", "client_id", "client_secret", "scopes"} {
			if value, ok := resolved[key]; ok {
				prov.Connection[key] = value
			}
		}
	}

	if mapping, ok := resolved["mapping"].(map[string]any); ok {
		if claims, ok := mapping["claims"].(map[string]any); ok {
			prov.Mapping.Claims = make(map[string]string, len(claims))
			for key, value := range claims {
				if str, ok := value.(string); ok {
					prov.Mapping.Claims[key] = str
				}
			}
		}
	} else if legacyClaims, ok := resolved["claim_mappings"].(map[string]any); ok {
		prov.Mapping.Claims = make(map[string]string, len(legacyClaims))
		for key, value := range legacyClaims {
			if str, ok := value.(string); ok {
				prov.Mapping.Claims[key] = str
			}
		}
	}

	if target, ok := resolved["target"].(map[string]any); ok {
		if schemaType, ok := target["schema_type"].(string); ok {
			prov.Target.SchemaType = schemaType
		}
		if schemaID, ok := target["schema_id"].(string); ok {
			prov.Target.SchemaID = schemaID
		}
	}
	if linking, ok := resolved["linking"].(map[string]any); ok {
		if mode, ok := linking["mode"].(string); ok {
			prov.Linking.Mode = mode
		}
		if matchBy, ok := linking["match_by"].(string); ok {
			prov.Linking.MatchBy = matchBy
		}
	}
	if ui, ok := resolved["ui"].(map[string]any); ok {
		prov.UI = ui
	}
	if session, ok := resolved["session"].(map[string]any); ok {
		prov.Session = session
	}
	if enabled, ok := resolved["enabled"].(bool); ok {
		prov.Enabled = enabled
	}
	if catalogMeta, ok := resolved["_catalog"].(map[string]any); ok {
		prov.CatalogMeta = catalogMeta
	}

	prov.CatalogRef = providers.CatalogRef{
		TemplateID:      tpl.ID,
		TemplateVersion: tpl.Version,
		Official:        tpl.Official,
		Capabilities:    tpl.Capabilities,
		LogoURL:         tpl.LogoURL,
		DocsURL:         tpl.DocsURL,
	}
	return providers.Normalize(prov), nil
}

func firstNonEmptyString(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" && value != "<nil>" {
			return value
		}
	}
	return ""
}

// computeHash returns a SHA-256 hex digest of the given data.
// Used for content-based lifecycle state detection (linked vs forked).
func computeHash(data []byte) string {
	h := sha256.Sum256(data)
	return "sha256:" + hex.EncodeToString(h[:])
}
