// Package seed loads declarative YAML seed files and applies them to the database.
//
// Seed files describe a desired state: schemas → providers → entities → linked accounts.
// They support environment variable substitution (${VAR}) and are idempotent (on_conflict: skip).
package seed

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"os"
	"regexp"
	"slices"
	"strings"

	"gopkg.in/yaml.v3"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/id"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// SeedFile represents the top-level structure of a seed YAML file.
type SeedFile struct {
	Providers  []SeedProvider `yaml:"providers"`
	Apps       []SeedApp      `yaml:"apps"`
	Identities []SeedIdentity `yaml:"users"`
}

// Summary returns a compact count summary for CLI and logs.
func (s SeedFile) Summary() SeedSummary {
	return SeedSummary{
		Providers: len(s.Providers),
		Apps:      len(s.Apps),
		Users:     len(s.Identities),
	}
}

// SeedSummary is a compact summary of a seed file's contents.
type SeedSummary struct {
	Providers int
	Apps      int
	Users     int
}

// SeedProvider defines a provider to seed.
type SeedProvider struct {
	ID             string            `yaml:"id"`
	Name           string            `yaml:"name"`
	Protocol       string            `yaml:"protocol"`
	Template       string            `yaml:"template"`
	Config         map[string]any    `yaml:"config"`
	ClaimOverrides map[string]string `yaml:"claim_overrides"`
	AutoRegister   *bool             `yaml:"auto_register"`
}

// SeedApp defines an app/OIDC client to seed.
type SeedApp struct {
	Name                   string         `yaml:"name"`
	ClientID               string         `yaml:"client_id"`
	ClientSecret           string         `yaml:"client_secret"`
	AppType                string         `yaml:"app_type"`
	RedirectURIs           []string       `yaml:"redirect_uris"`
	PostLogoutRedirectURIs []string       `yaml:"post_logout_redirect_uris"`
	GrantTypes             []string       `yaml:"grant_types"`
	ResponseTypes          []string       `yaml:"response_types"`
	SchemaID               string         `yaml:"schema_id"`
	State                  string         `yaml:"state"`
	Metadata               map[string]any `yaml:"metadata"`
	OnConflict             string         `yaml:"on_conflict"`
}

// SeedIdentity defines an identity to seed.
type SeedIdentity struct {
	Identifier     string              `yaml:"identifier"`
	DisplayName    string              `yaml:"display_name"`
	SchemaID       string              `yaml:"schema_id"`
	State          string              `yaml:"state"`
	Password       string              `yaml:"password"`
	Profile        map[string]any      `yaml:"profile"`
	Capabilities   []string            `yaml:"capabilities"` // NEW: ["admin", "password"]
	PATs           []SeedPAT           `yaml:"pats"`         // NEW: personal access tokens
	OnConflict     string              `yaml:"on_conflict"`  // NEW: "skip" (default), "warn", "update"
	LinkedAccounts []SeedLinkedAccount `yaml:"linked_identities"`
}

// SeedPAT defines a personal access token to seed for an identity.
type SeedPAT struct {
	Name   string   `yaml:"name"`
	Token  string   `yaml:"token"` // raw token (will be prefixed + hashed)
	Scopes []string `yaml:"scopes"`
}

// SeedLinkedAccount links an identity to a provider in the seed.
type SeedLinkedAccount struct {
	Provider      string `yaml:"provider"` // provider name or ID
	ExternalSub   string `yaml:"external_sub"`
	ExternalEmail string `yaml:"external_email"`
}

// envVarPattern matches ${VAR} or ${VAR:-default} patterns.
var envVarPattern = regexp.MustCompile(`\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}`)

// substituteEnvVars replaces ${VAR} patterns with environment variable values.
func substituteEnvVars(input []byte) []byte {
	return envVarPattern.ReplaceAllFunc(input, func(match []byte) []byte {
		parts := envVarPattern.FindSubmatch(match)
		varName := string(parts[1])
		defaultVal := string(parts[2])
		if val := os.Getenv(varName); val != "" {
			return []byte(val)
		}
		return []byte(defaultVal)
	})
}

// LoadFile reads, expands, parses, and validates a seed file.
func LoadFile(path string) (*SeedFile, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read seed file %s: %w", path, err)
	}

	data = substituteEnvVars(data)

	var seed SeedFile
	decoder := yaml.NewDecoder(bytes.NewReader(data))
	if err := decoder.Decode(&seed); err != nil {
		return nil, fmt.Errorf("parse seed file: %w", err)
	}
	if err := validate(seed); err != nil {
		return nil, err
	}

	return &seed, nil
}

// LoadAndApply reads a seed YAML file and applies it to the database.
// It is idempotent — existing records are handled per on_conflict setting.
func LoadAndApply(ctx context.Context, db *sql.DB, path string) error {
	seed, err := LoadFile(path)
	if err != nil {
		return err
	}

	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin transaction: %w", err)
	}
	defer tx.Rollback()

	// Look up any org ID to assign seeded entities to (may be empty if no orgs exist yet).
	var defaultOrgID string
	_ = tx.QueryRowContext(ctx, `SELECT id FROM orgs LIMIT 1`).Scan(&defaultOrgID)

	// Phase 1: Providers.
	for _, p := range seed.Providers {
		if err := seedProvider(ctx, tx, p, defaultOrgID); err != nil {
			return fmt.Errorf("seed provider %q: %w", p.Name, err)
		}
	}

	// Phase 2: Apps.
	for _, app := range seed.Apps {
		if err := seedApp(ctx, tx, app, defaultOrgID); err != nil {
			return fmt.Errorf("seed app %q: %w", app.ClientID, err)
		}
	}

	// Phase 3: Identities (with inline linked accounts, capabilities, PATs).
	for _, ident := range seed.Identities {
		if err := seedIdentity(ctx, tx, ident, defaultOrgID); err != nil {
			return fmt.Errorf("seed identity %q: %w", ident.Identifier, err)
		}
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	totalItems := len(seed.Providers) + len(seed.Apps) + len(seed.Identities)
	logging.Printf("[seed] applied %d items from %s", totalItems, path)
	return nil
}

func validate(seed SeedFile) error {
	providerIDs := map[string]struct{}{}
	providerNames := map[string]struct{}{}
	appClientIDs := map[string]struct{}{}
	userIdentifiers := map[string]struct{}{}
	validConflictModes := []string{"", "skip", "warn", "update"}

	for _, provider := range seed.Providers {
		if id := strings.TrimSpace(provider.ID); id != "" {
			if _, exists := providerIDs[id]; exists {
				return fmt.Errorf("validate seed file: duplicate provider id %q", id)
			}
			providerIDs[id] = struct{}{}
		}
		if name := strings.TrimSpace(provider.Name); name != "" {
			if _, exists := providerNames[name]; exists {
				return fmt.Errorf("validate seed file: duplicate provider name %q", name)
			}
			providerNames[name] = struct{}{}
		}
	}

	for _, identity := range seed.Identities {
		identifier := strings.TrimSpace(identity.Identifier)
		if identifier == "" {
			return fmt.Errorf("validate seed file: user identifier is required")
		}
		if _, exists := userIdentifiers[identifier]; exists {
			return fmt.Errorf("validate seed file: duplicate user identifier %q", identifier)
		}
		userIdentifiers[identifier] = struct{}{}

		if !slices.Contains(validConflictModes, identity.OnConflict) {
			return fmt.Errorf("validate seed file: unsupported on_conflict %q for user %q", identity.OnConflict, identifier)
		}
	}

	for _, app := range seed.Apps {
		clientID := strings.TrimSpace(app.ClientID)
		if clientID == "" {
			return fmt.Errorf("validate seed file: app client_id is required")
		}
		if _, exists := appClientIDs[clientID]; exists {
			return fmt.Errorf("validate seed file: duplicate app client_id %q", clientID)
		}
		appClientIDs[clientID] = struct{}{}

		if !slices.Contains(validConflictModes, app.OnConflict) {
			return fmt.Errorf("validate seed file: unsupported on_conflict %q for app %q", app.OnConflict, clientID)
		}
	}

	return nil
}

func seedProvider(ctx context.Context, tx *sql.Tx, p SeedProvider, orgID string) error {
	// Skip if exists by name in providers table.
	var existing string
	err := tx.QueryRowContext(ctx,
		`SELECT id FROM providers WHERE name = ?`, p.Name).Scan(&existing)
	if err == nil {
		logging.Printf("[seed] provider %q already exists, skipping", p.Name)
		return nil
	}

	provID := p.ID
	if provID == "" {
		provID = fmt.Sprintf("prov_%s", generateShortID())
	}
	if p.Protocol == "" {
		p.Protocol = "oidc"
	}
	if p.Template == "" {
		p.Template = "custom"
	}
	autoReg := true
	if p.AutoRegister != nil {
		autoReg = *p.AutoRegister
	}

	prov := providers.Provider{
		ID:          provID,
		OrgID:       orgID,
		DisplayName: p.Name,
		Protocol:    p.Protocol,
		Connection:  map[string]any{},
		Mapping: providers.Mapping{
			Claims: map[string]string{},
		},
		Enabled: true,
		CatalogRef: providers.CatalogRef{
			TemplateID: p.Template,
		},
	}
	if p.Config != nil {
		prov.Connection = p.Config
	}
	if p.ClaimOverrides != nil {
		prov.Mapping.Claims = p.ClaimOverrides
	}
	if !autoReg {
		prov.Linking.Mode = providers.LinkModeLinkOnly
	}
	prov = providers.Normalize(prov)

	configJSON, _ := json.Marshal(prov.Connection)
	overridesJSON, _ := json.Marshal(prov.Mapping.Claims)
	metadataJSON, _ := json.Marshal(prov)

	_, err = tx.ExecContext(ctx,
		`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))`,
		provID,
		orgID,
		prov.DisplayName,
		prov.Protocol,
		providers.LegacyTemplateID(prov),
		string(configJSON),
		string(overridesJSON),
		providers.LegacyAutoRegister(prov),
		prov.Enabled,
		providers.DisplayOrder(prov),
		prov.Target.SchemaID,
		string(metadataJSON),
	)
	if err != nil {
		return err
	}

	logging.Printf("[seed] created provider %q (id: %s)", p.Name, provID)
	return nil
}

func seedApp(ctx context.Context, tx *sql.Tx, app SeedApp, orgID string) error {
	onConflict := app.OnConflict
	if onConflict == "" {
		onConflict = "skip"
	}

	var existingID string
	err := tx.QueryRowContext(ctx, `SELECT id FROM apps WHERE client_id = ?`, app.ClientID).Scan(&existingID)
	if err == nil {
		switch onConflict {
		case "update":
			logging.Printf("[seed] app %q already exists, updating (on_conflict: update)", app.ClientID)
			return updateExistingApp(ctx, tx, existingID, app, orgID)
		case "warn":
			logging.Printf("[seed] WARN: app %q already exists, skipping (on_conflict: warn)", app.ClientID)
			return nil
		default:
			logging.Printf("[seed] app %q already exists, skipping", app.ClientID)
			return nil
		}
	}

	appID := id.New()
	state := app.State
	if state == "" {
		state = "active"
	}

	schemaRec, err := resolveSeedAppSchema(ctx, tx, app.SchemaID)
	if err != nil {
		return err
	}

	payload := map[string]any{
		"client_name": app.Name,
		"app_type":    normalizeSeedAppType(app.AppType),
	}
	if len(app.RedirectURIs) > 0 {
		payload["redirect_uris"] = append([]string(nil), app.RedirectURIs...)
	}
	if len(app.PostLogoutRedirectURIs) > 0 {
		payload["post_logout_redirect_uris"] = append([]string(nil), app.PostLogoutRedirectURIs...)
	}
	if len(app.GrantTypes) > 0 {
		payload["grant_types"] = append([]string(nil), app.GrantTypes...)
	}
	if len(app.ResponseTypes) > 0 {
		payload["response_types"] = append([]string(nil), app.ResponseTypes...)
	}
	if app.Metadata != nil {
		payload["metadata"] = app.Metadata
	}
	payload, err = schema.ObjectMap(payload)
	if err != nil {
		return err
	}
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return err
	}

	redirectURIs := payload["redirect_uris"]
	if redirectURIs == nil {
		redirectURIs = []string{}
	}
	grantTypes := payload["grant_types"]
	if grantTypes == nil {
		grantTypes = []string{"authorization_code"}
	}
	responseTypes := payload["response_types"]
	if responseTypes == nil {
		responseTypes = []string{"code"}
	}

	redirectJSON, _ := json.Marshal(redirectURIs)
	grantJSON, _ := json.Marshal(grantTypes)
	responseJSON, _ := json.Marshal(responseTypes)

	clientSecret := ""
	if app.ClientSecret != "" {
		clientSecret, err = auth.HashSecret(app.ClientSecret)
		if err != nil {
			return fmt.Errorf("hash client secret: %w", err)
		}
	}

	metadataJSON := map[string]any{}
	if len(app.PostLogoutRedirectURIs) > 0 {
		metadataJSON["post_logout_redirect_uris"] = append([]string(nil), app.PostLogoutRedirectURIs...)
	}
	if app.Metadata != nil {
		metadataJSON["metadata"] = app.Metadata
	}
	encodedMetadata, _ := json.Marshal(metadataJSON)

	_, err = tx.ExecContext(ctx,
		`INSERT INTO apps (id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))`,
		appID,
		orgID,
		app.Name,
		normalizeSeedAppType(app.AppType),
		app.ClientID,
		clientSecret,
		string(redirectJSON),
		string(grantJSON),
		string(responseJSON),
		state,
		schemaRec.ID,
		string(encodedMetadata),
	)
	if err != nil {
		return err
	}

	logging.Printf("[seed] created app %q (id: %s)", app.ClientID, appID)
	return nil
}

func updateExistingApp(ctx context.Context, tx *sql.Tx, appID string, app SeedApp, orgID string) error {
	schemaRec, err := resolveSeedAppSchema(ctx, tx, app.SchemaID)
	if err != nil {
		return err
	}

	payload := map[string]any{
		"client_name": app.Name,
		"app_type":    normalizeSeedAppType(app.AppType),
	}
	if len(app.RedirectURIs) > 0 {
		payload["redirect_uris"] = append([]string(nil), app.RedirectURIs...)
	}
	if len(app.PostLogoutRedirectURIs) > 0 {
		payload["post_logout_redirect_uris"] = append([]string(nil), app.PostLogoutRedirectURIs...)
	}
	if len(app.GrantTypes) > 0 {
		payload["grant_types"] = append([]string(nil), app.GrantTypes...)
	}
	if len(app.ResponseTypes) > 0 {
		payload["response_types"] = append([]string(nil), app.ResponseTypes...)
	}
	if app.Metadata != nil {
		payload["metadata"] = app.Metadata
	}
	payload, err = schema.ObjectMap(payload)
	if err != nil {
		return err
	}
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return err
	}

	redirectURIs := payload["redirect_uris"]
	if redirectURIs == nil {
		redirectURIs = []string{}
	}
	redirectJSON, _ := json.Marshal(redirectURIs)
	grantTypes := payload["grant_types"]
	if grantTypes == nil {
		grantTypes = []string{"authorization_code"}
	}
	responseTypes := payload["response_types"]
	if responseTypes == nil {
		responseTypes = []string{"code"}
	}
	grantJSON, _ := json.Marshal(grantTypes)
	responseJSON, _ := json.Marshal(responseTypes)

	clientSecret := ""
	if app.ClientSecret != "" {
		clientSecret, err = auth.HashSecret(app.ClientSecret)
		if err != nil {
			return fmt.Errorf("hash client secret: %w", err)
		}
	} else {
		if err := tx.QueryRowContext(ctx, `SELECT COALESCE(client_secret, '') FROM apps WHERE id = ?`, appID).Scan(&clientSecret); err != nil {
			return err
		}
	}

	state := app.State
	if state == "" {
		state = "active"
	}

	metadataJSON := map[string]any{}
	if len(app.PostLogoutRedirectURIs) > 0 {
		metadataJSON["post_logout_redirect_uris"] = append([]string(nil), app.PostLogoutRedirectURIs...)
	}
	if app.Metadata != nil {
		metadataJSON["metadata"] = app.Metadata
	}
	encodedMetadata, _ := json.Marshal(metadataJSON)

	_, err = tx.ExecContext(ctx,
		`UPDATE apps
		 SET org_id = ?, name = ?, app_type = ?, client_secret = ?, redirect_uris = ?, grant_types = ?, response_types = ?, state = ?, schema_id = ?, metadata = ?, updated_at = datetime('now')
		 WHERE id = ?`,
		orgID,
		app.Name,
		normalizeSeedAppType(app.AppType),
		clientSecret,
		string(redirectJSON),
		string(grantJSON),
		string(responseJSON),
		state,
		schemaRec.ID,
		string(encodedMetadata),
		appID,
	)
	return err
}

func resolveSeedAppSchema(ctx context.Context, tx *sql.Tx, schemaID string) (*schema.SchemaRecord, error) {
	if strings.TrimSpace(schemaID) != "" {
		var rec schema.SchemaRecord
		err := tx.QueryRowContext(ctx,
			`SELECT id, type, schema FROM schemas WHERE id = ?`,
			schemaID,
		).Scan(&rec.ID, &rec.Type, &rec.Schema)
		if err != nil {
			return nil, err
		}
		if rec.Type != "app" {
			return nil, fmt.Errorf("schema %q is type %q, not an app schema", rec.ID, rec.Type)
		}
		return &rec, nil
	}

	var rec schema.SchemaRecord
	err := tx.QueryRowContext(ctx,
		`SELECT id, type, schema
		 FROM schemas
		 WHERE type = 'app' AND is_default = true
		 ORDER BY created_at ASC
		 LIMIT 1`,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err == nil {
		return &rec, nil
	}
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return nil, err
	}

	err = tx.QueryRowContext(ctx,
		`SELECT id, type, schema
		 FROM schemas
		 WHERE type = 'app'
		 ORDER BY version DESC, created_at ASC
		 LIMIT 1`,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err != nil {
		return nil, err
	}
	return &rec, nil
}

func normalizeSeedAppType(value string) string {
	switch strings.TrimSpace(value) {
	case "", "oidc":
		return "web"
	case "api":
		return "m2m"
	default:
		return strings.TrimSpace(value)
	}
}

func seedIdentity(ctx context.Context, tx *sql.Tx, ident SeedIdentity, orgID string) error {
	onConflict := ident.OnConflict
	if onConflict == "" {
		onConflict = "skip"
	}

	// Check if exists by identifier.
	var existingID string
	err := tx.QueryRowContext(ctx, `SELECT id FROM users WHERE identifier = ?`, ident.Identifier).Scan(&existingID)
	if err == nil {
		// Entity already exists — handle according to on_conflict.
		switch onConflict {
		case "update":
			logging.Printf("[seed] identity %q already exists, updating (on_conflict: update)", ident.Identifier)
			return updateExistingIdentity(ctx, tx, existingID, ident)
		case "warn":
			logging.Printf("[seed] WARN: identity %q already exists, skipping (on_conflict: warn)", ident.Identifier)
			return nil
		default: // "skip"
			logging.Printf("[seed] identity %q already exists, skipping", ident.Identifier)
			// Ensure org membership exists even on skip.
			if orgID != "" {
				tx.ExecContext(ctx,
					`INSERT OR IGNORE INTO memberships (resource_type, resource_id, user_id, role, added_at) VALUES ('org', ?, ?, 'member', datetime('now'))`,
					orgID, existingID)
			}
			// Still process linked accounts for this existing identity.
			for _, la := range ident.LinkedAccounts {
				seedLinkedAccount(ctx, tx, existingID, la)
			}
			// Ensure capabilities and PATs exist even on skip.
			seedCapabilities(ctx, tx, existingID, ident.Capabilities)
			seedPATs(ctx, tx, existingID, ident.PATs)
			return nil
		}
	}

	newID := id.New()

	state := ident.State
	if state == "" {
		state = "active"
	}
	schemaRec, err := resolveSeedUserSchema(ctx, tx, ident.SchemaID)
	if err != nil {
		return err
	}
	payload := schema.MaterializeUserData(schemaRec.Schema, ident.Identifier, ident.DisplayName, ident.Profile)
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return err
	}
	profileJSON := "{}"
	if ident.Profile != nil {
		b, _ := json.Marshal(ident.Profile)
		profileJSON = string(b)
	}

	_, err = tx.ExecContext(ctx,
		`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))`,
		newID, orgID, ident.Identifier, ident.DisplayName, func() string {
			if schemaRec.Type == "service_user" || schemaRec.Type == "ai_agent" {
				return schemaRec.Type
			}
			return "human"
		}(), state, schemaRec.ID, profileJSON)
	if err != nil {
		return err
	}
	if err := uniqueness.EnforceFromIdentifier(ctx, tx, newID, orgID, ident.Identifier); err != nil {
		return err
	}
	if err := uniqueness.Enforce(ctx, tx, newID, orgID, uniqueness.ExtractConstraints(schemaRec.Schema), payload); err != nil {
		return err
	}

	// Hash password and store as entity_credential.
	if ident.Password != "" {
		pw := auth.NewPasswordsDev(nil)
		hash, err := pw.Hash(ident.Password)
		if err == nil {
			credID := id.New()
			credJSON := auth.EncodeCredentialJSON(hash)
			tx.ExecContext(ctx,
				`INSERT INTO credentials (id, user_id, type, data) VALUES (?, ?, 'password', ?)`,
				credID, newID, credJSON)
		}
	}

	// Seed capabilities.
	seedCapabilities(ctx, tx, newID, ident.Capabilities)

	// Seed PATs.
	seedPATs(ctx, tx, newID, ident.PATs)

	// Insert org membership (only when a default org exists).
	if orgID != "" {
		tx.ExecContext(ctx,
			`INSERT OR IGNORE INTO memberships (resource_type, resource_id, user_id, role, added_at) VALUES ('org', ?, ?, 'member', datetime('now'))`,
			orgID, newID)
	}

	logging.Printf("[seed] created identity %q (id: %s)", ident.Identifier, newID)

	// Process linked accounts.
	for _, la := range ident.LinkedAccounts {
		seedLinkedAccount(ctx, tx, newID, la)
	}

	return nil
}

// updateExistingIdentity handles the on_conflict: update case.
func updateExistingIdentity(ctx context.Context, tx *sql.Tx, userID string, ident SeedIdentity) error {
	// Update password if provided.
	if ident.Password != "" {
		pw := auth.NewPasswordsDev(nil)
		hash, err := pw.Hash(ident.Password)
		if err == nil {
			// Delete existing + re-insert.
			tx.ExecContext(ctx, `DELETE FROM credentials WHERE user_id = ? AND type = 'password'`, userID)
			credID := id.New()
			credJSON := auth.EncodeCredentialJSON(hash)
			tx.ExecContext(ctx,
				`INSERT INTO credentials (id, user_id, type, data) VALUES (?, ?, 'password', ?)`,
				credID, userID, credJSON)
			logging.Printf("[seed]   updated password for %q", ident.Identifier)
		}
	}

	// Upsert capabilities.
	seedCapabilities(ctx, tx, userID, ident.Capabilities)

	// Upsert PATs.
	seedPATs(ctx, tx, userID, ident.PATs)

	// Process linked accounts.
	for _, la := range ident.LinkedAccounts {
		seedLinkedAccount(ctx, tx, userID, la)
	}

	return nil
}

func resolveSeedUserSchema(ctx context.Context, tx *sql.Tx, schemaID string) (*schema.SchemaRecord, error) {
	if strings.TrimSpace(schemaID) != "" {
		var rec schema.SchemaRecord
		err := tx.QueryRowContext(ctx,
			`SELECT id, type, schema FROM schemas WHERE id = ?`,
			schemaID,
		).Scan(&rec.ID, &rec.Type, &rec.Schema)
		if err != nil {
			return nil, err
		}
		if !schema.IsUserSchemaType(rec.Type) {
			return nil, fmt.Errorf("schema %q is type %q, not a user schema", rec.ID, rec.Type)
		}
		return &rec, nil
	}

	var rec schema.SchemaRecord
	err := tx.QueryRowContext(ctx,
		`SELECT id, type, schema
		 FROM schemas
		 WHERE type = 'human_user' AND is_default = true
		 ORDER BY created_at ASC
		 LIMIT 1`,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err == nil {
		return &rec, nil
	}
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return nil, err
	}

	err = tx.QueryRowContext(ctx,
		`SELECT id, type, schema
		 FROM schemas
		 WHERE type = 'human_user'
		 ORDER BY version DESC, created_at ASC
		 LIMIT 1`,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err != nil {
		return nil, err
	}
	return &rec, nil
}

// seedCapabilities is a no-op — capabilities are now handled by FGA.
func seedCapabilities(_ context.Context, _ *sql.Tx, _ string, _ []string) {
	// FGA tuples are written during bootstrap; seed caps are ignored.
}

// seedPATs creates PAT tokens for an entity (idempotent via name check).
func seedPATs(ctx context.Context, tx *sql.Tx, userID string, pats []SeedPAT) {
	for _, pat := range pats {
		// Skip if a PAT with this name already exists for this entity.
		var existingID string
		err := tx.QueryRowContext(ctx,
			`SELECT id FROM tokens WHERE user_id = ? AND name = ? AND type = 'pat' AND revoked_at IS NULL`,
			userID, pat.Name).Scan(&existingID)
		if err == nil {
			logging.Printf("[seed]   PAT %q already exists for entity %s, skipping", pat.Name, userID)
			continue
		}

		// Prefix the token if not already prefixed.
		rawToken := pat.Token
		if !strings.HasPrefix(rawToken, "zit_pat_") {
			rawToken = "zit_pat_" + rawToken
		}

		// Hash the token.
		tokenHash := crypto.HashTokenHex(rawToken)

		tokenID := id.New()

		scopes := pat.Scopes
		if len(scopes) == 0 {
			scopes = []string{"admin"}
		}
		scopesJSON, _ := json.Marshal(scopes)

		_, err = tx.ExecContext(ctx,
			`INSERT INTO tokens (id, type, token_hash, user_id, name, scopes, created_at)
			 VALUES (?, 'pat', ?, ?, ?, ?, datetime('now'))`,
			tokenID, tokenHash, userID, pat.Name, string(scopesJSON))
		if err != nil {
			logging.Printf("[seed]   failed to create PAT %q: %v", pat.Name, err)
			continue
		}

		logging.Printf("[seed]   created PAT %q for entity %s", pat.Name, userID)
	}
}

func seedLinkedAccount(ctx context.Context, tx *sql.Tx, userID string, la SeedLinkedAccount) {
	// Resolve provider by name or ID from providers table.
	var providerID string
	err := tx.QueryRowContext(ctx,
		`SELECT id FROM providers WHERE name = ? OR id = ?`,
		la.Provider, la.Provider).Scan(&providerID)
	if err != nil {
		logging.Printf("[seed] linked_account: provider %q not found, skipping", la.Provider)
		return
	}

	// Skip if already linked.
	var existingLink int64
	err = tx.QueryRowContext(ctx,
		`SELECT id FROM linked_identities WHERE provider_id = ? AND external_sub = ?`,
		providerID, la.ExternalSub).Scan(&existingLink)
	if err == nil {
		return // already linked
	}

	linkID := id.New()
	tx.ExecContext(ctx,
		`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
		 VALUES (?, ?, ?, ?, ?, '{}', datetime('now'))`,
		linkID, userID, providerID, la.ExternalSub, la.ExternalEmail)

	logging.Printf("[seed] linked identity %s → provider %s (sub: %s)", userID, providerID, la.ExternalSub)
}

func generateShortID() string {
	return id.New()
}
