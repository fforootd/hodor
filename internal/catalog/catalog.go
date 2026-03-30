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
	"strings"
	"sync"

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
	ID          string   `json:"id"`
	Type        string   `json:"type"` // action | provider | authorization | schema
	Name        string   `json:"name"`
	Description string   `json:"description"`
	Tags        []string `json:"tags"`
	Version     string   `json:"version"`
	Author      string   `json:"author,omitempty"`
	Path        string   `json:"path"`   // relative path to template JSON
	Source      string   `json:"source"` // "embedded" | "remote" | "cached"
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
	cfg      config.CatalogConfig
}

// New creates a new catalog service. The embedded catalog is loaded synchronously
// (always available). Remote refresh happens asynchronously via StartBackground().
func New(cfg config.CatalogConfig, db *sql.DB) *Service {
	s := &Service{
		db:  db,
		cfg: cfg,
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

	// Compute content hash of the original resolved payload (before adding _catalog).
	resolvedJSON, err := json.Marshal(resolved)
	if err != nil {
		return "", fmt.Errorf("marshal payload: %w", err)
	}
	contentHash := computeHash(resolvedJSON)

	// Add _catalog origin tracking metadata.
	resolved["_catalog"] = map[string]any{
		"template_id":      templateID,
		"template_version": tpl.Version,
		"installed_at":     "now", // replaced by DB
		"installed_hash":   contentHash,
		"auto_upgrade":     false,
	}

	// Determine schema type from template type.
	schemaType := tpl.Type
	if schemaType == "authorization" {
		schemaType = "fga_model"
	}

	// Look up the schema_id for this type (latest default version).
	var schemaID string
	err = s.db.QueryRowContext(ctx,
		`SELECT id FROM schemas WHERE type = ? ORDER BY is_default DESC, version DESC LIMIT 1`,
		schemaType,
	).Scan(&schemaID)
	if err != nil {
		// If no schema exists for this type, use a synthetic ID.
		schemaID = schemaType + "_v1"
	}

	// Serialize the full payload (including _catalog).
	dataJSON, err := json.Marshal(resolved)
	if err != nil {
		return "", fmt.Errorf("marshal payload: %w", err)
	}

	resourceID := id.New()
	displayName, _ := resolved["display_name"].(string)
	if displayName == "" {
		displayName = payload.Name
	}

	// Dispatch insert to the correct dedicated table based on template type.
	// Use org from context if available, otherwise default to '1'.
	orgID := "1"
	if ctxOrg, ok := ctx.Value("org_id").(string); ok && ctxOrg != "" {
		orgID = ctxOrg
	}

	switch payload.Type {
	case "provider":
		protocol, _ := resolved["protocol"].(string)
		if protocol == "" {
			protocol = "oidc"
		}
		templateName, _ := resolved["template"].(string)
		if templateName == "" {
			templateName = templateID
		}
		configJSON, _ := json.Marshal(resolved["config"])
		overridesJSON, _ := json.Marshal(resolved["claim_overrides"])
		_, err = s.db.ExecContext(ctx,
			`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, schema_id, metadata, created_at, updated_at)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))`,
			resourceID, orgID, displayName, protocol, templateName,
			string(configJSON), string(overridesJSON), schemaID, string(dataJSON),
		)
	case "action":
		hook, _ := resolved["hook"].(string)
		actionType, _ := resolved["action_type"].(string)
		triggerExpr, _ := resolved["trigger"].(string)
		configJSON, _ := json.Marshal(resolved["config"])
		_, err = s.db.ExecContext(ctx,
			`INSERT INTO actions (id, org_id, name, hook, action_type, trigger_expr, config, schema_id, metadata, created_at, updated_at)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))`,
			resourceID, orgID, displayName, hook, actionType, triggerExpr,
			string(configJSON), schemaID, string(dataJSON),
		)
	case "login_flow":
		// Extract top-level fields that map to dedicated columns.
		preset, _ := resolved["preset"].(string)
		if preset == "" {
			preset = "identifier_first"
		}
		authMethodsJSON, _ := json.Marshal(resolved["auth_methods"])
		configJSON, _ := json.Marshal(resolved["config"])
		audienceJSON, _ := json.Marshal(resolved["audience"])
		if string(audienceJSON) == "null" {
			audienceJSON = []byte("{}")
		}

		_, err = s.db.ExecContext(ctx,
			`INSERT INTO login_flows (id, org_id, name, preset, auth_methods, config, enabled, state, priority, audience, schema_id, metadata, created_at, updated_at)
			 VALUES (?, ?, ?, ?, ?, ?, 1, 'active', 10, ?, ?, ?, datetime('now'), datetime('now'))`,
			resourceID, orgID, displayName, preset,
			string(authMethodsJSON), string(configJSON), string(audienceJSON),
			schemaID, string(dataJSON),
		)
	default:
		// Schema or unknown type — insert as user.
		_, err = s.db.ExecContext(ctx,
			`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
			 VALUES (?, ?, ?, ?, 'human', 'active', ?, ?, datetime('now'), datetime('now'))`,
			resourceID, orgID, templateID, displayName, schemaID, string(dataJSON),
		)
	}
	if err != nil {
		return "", fmt.Errorf("insert resource: %w", err)
	}

	return resourceID, nil
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
	data, err := json.Marshal(idx)
	if err != nil {
		logging.Printf("[catalog] failed to marshal index for cache: %v", err)
		return
	}
	_, err = s.db.Exec(
		`INSERT OR REPLACE INTO cache (namespace, key, data, fetched_at) VALUES ('catalog', 'remote_index', ?, datetime('now'))`,
		string(data),
	)
	if err != nil {
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

// computeHash returns a SHA-256 hex digest of the given data.
// Used for content-based lifecycle state detection (linked vs forked).
func computeHash(data []byte) string {
	h := sha256.Sum256(data)
	return "sha256:" + hex.EncodeToString(h[:])
}
