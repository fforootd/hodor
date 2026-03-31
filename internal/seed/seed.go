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
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"os"
	"regexp"
	"slices"
	"strings"
	"time"

	"gopkg.in/yaml.v3"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/resourcedata"
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
func LoadAndApply(ctx context.Context, db *sql.DB, path string, dialect ...string) error {
	seed, err := LoadFile(path)
	if err != nil {
		return err
	}
	seedDialect := effectiveDialect(dialect...)

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
		if err := seedProvider(ctx, tx, p, defaultOrgID, seedDialect); err != nil {
			return fmt.Errorf("seed provider %q: %w", p.Name, err)
		}
	}

	// Phase 2: Apps.
	for _, app := range seed.Apps {
		if err := seedApp(ctx, tx, app, defaultOrgID, seedDialect); err != nil {
			return fmt.Errorf("seed app %q: %w", app.ClientID, err)
		}
	}

	// Phase 3: Identities (with inline linked accounts, capabilities, PATs).
	for _, ident := range seed.Identities {
		if err := seedIdentity(ctx, tx, ident, defaultOrgID, seedDialect); err != nil {
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

func effectiveDialect(dialect ...string) string {
	if len(dialect) > 0 && strings.TrimSpace(dialect[0]) != "" {
		return strings.TrimSpace(dialect[0])
	}
	return "sqlite"
}

func bindSeedQuery(query, dialect string) string {
	return database.RebindPlaceholders(query, dialect)
}

func seedNow() string {
	return time.Now().UTC().Format(time.RFC3339)
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

func seedProvider(ctx context.Context, tx *sql.Tx, p SeedProvider, orgID, dialect string) error {
	// Skip if exists by name in providers table.
	var existing string
	err := tx.QueryRowContext(ctx,
		bindSeedQuery(`SELECT id FROM providers WHERE name = ?`, dialect), p.Name).Scan(&existing)
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

	targetSchemaID, targetSchemaType, err := providers.ResolveTargetSchema(ctx, tx, prov.Target, dialect)
	if err != nil {
		return err
	}
	prov.Target.SchemaID = targetSchemaID
	prov.Target.SchemaType = targetSchemaType
	resourceSchema, err := schema.ResolveSchemaForType(ctx, tx, "provider", prov.SchemaID, dialect)
	if err != nil {
		return err
	}
	prov.SchemaID = resourceSchema.ID
	prov = providers.Normalize(prov)
	provData, err := providers.SchemaData(prov)
	if err != nil {
		return err
	}
	if err := schema.ValidateData(resourceSchema.Schema, provData); err != nil {
		return err
	}

	configJSON, _ := json.Marshal(prov.Connection)
	overridesJSON, _ := json.Marshal(prov.Mapping.Claims)
	metadataJSON, _ := json.Marshal(prov)
	now := seedNow()

	_, err = tx.ExecContext(ctx,
		bindSeedQuery(`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, schema_id, target_schema_id, target_schema_type, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`, dialect),
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
		prov.SchemaID,
		prov.Target.SchemaID,
		prov.Target.SchemaType,
		string(metadataJSON),
		now,
		now,
	)
	if err != nil {
		return err
	}

	logging.Printf("[seed] created provider %q (id: %s)", p.Name, provID)
	return nil
}

func seedApp(ctx context.Context, tx *sql.Tx, app SeedApp, orgID, dialect string) error {
	onConflict := app.OnConflict
	if onConflict == "" {
		onConflict = "skip"
	}

	var existingID string
	err := tx.QueryRowContext(ctx, bindSeedQuery(`SELECT id FROM apps WHERE client_id = ?`, dialect), app.ClientID).Scan(&existingID)
	if err == nil {
		switch onConflict {
		case "update":
			logging.Printf("[seed] app %q already exists, updating (on_conflict: update)", app.ClientID)
			return updateExistingApp(ctx, tx, existingID, app, orgID, dialect)
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

	schemaRec, err := resolveSeedAppSchema(ctx, tx, app.SchemaID, dialect)
	if err != nil {
		return err
	}

	appMetadata := map[string]any{}
	if app.Metadata != nil {
		appMetadata["metadata"] = app.Metadata
	}
	payload, err := schema.ObjectMap(resourcedata.AppCanonicalData(
		app.Name,
		"",
		normalizeSeedAppType(app.AppType),
		app.RedirectURIs,
		app.PostLogoutRedirectURIs,
		app.GrantTypes,
		app.ResponseTypes,
		"",
		appMetadata,
	))
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
	now := seedNow()

	_, err = tx.ExecContext(ctx,
		bindSeedQuery(`INSERT INTO apps (id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`, dialect),
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
		now,
		now,
	)
	if err != nil {
		return err
	}

	logging.Printf("[seed] created app %q (id: %s)", app.ClientID, appID)
	return nil
}

func updateExistingApp(ctx context.Context, tx *sql.Tx, appID string, app SeedApp, orgID, dialect string) error {
	schemaRec, err := resolveSeedAppSchema(ctx, tx, app.SchemaID, dialect)
	if err != nil {
		return err
	}

	appMetadata := map[string]any{}
	if app.Metadata != nil {
		appMetadata["metadata"] = app.Metadata
	}
	payload, err := schema.ObjectMap(resourcedata.AppCanonicalData(
		app.Name,
		"",
		normalizeSeedAppType(app.AppType),
		app.RedirectURIs,
		app.PostLogoutRedirectURIs,
		app.GrantTypes,
		app.ResponseTypes,
		"",
		appMetadata,
	))
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
		if err := tx.QueryRowContext(ctx, bindSeedQuery(`SELECT COALESCE(client_secret, '') FROM apps WHERE id = ?`, dialect), appID).Scan(&clientSecret); err != nil {
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
	now := seedNow()

	_, err = tx.ExecContext(ctx,
		bindSeedQuery(`UPDATE apps
		 SET org_id = ?, name = ?, app_type = ?, client_secret = ?, redirect_uris = ?, grant_types = ?, response_types = ?, state = ?, schema_id = ?, metadata = ?, updated_at = ?
		 WHERE id = ?`, dialect),
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
		now,
		appID,
	)
	return err
}

func resolveSeedAppSchema(ctx context.Context, tx *sql.Tx, schemaID, dialect string) (*schema.SchemaRecord, error) {
	return schema.ResolveSchemaForType(ctx, tx, "app", schemaID, dialect)
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

func seedIdentity(ctx context.Context, tx *sql.Tx, ident SeedIdentity, orgID, dialect string) error {
	onConflict := ident.OnConflict
	if onConflict == "" {
		onConflict = "skip"
	}

	// Check if exists by identifier.
	var existingID string
	err := tx.QueryRowContext(ctx, bindSeedQuery(`SELECT id FROM users WHERE identifier = ?`, dialect), ident.Identifier).Scan(&existingID)
	if err == nil {
		// Entity already exists — handle according to on_conflict.
		switch onConflict {
		case "update":
			logging.Printf("[seed] identity %q already exists, updating (on_conflict: update)", ident.Identifier)
			return updateExistingIdentity(ctx, tx, existingID, ident, dialect)
		case "warn":
			logging.Printf("[seed] WARN: identity %q already exists, skipping (on_conflict: warn)", ident.Identifier)
			return nil
		default: // "skip"
			logging.Printf("[seed] identity %q already exists, skipping", ident.Identifier)
			// Ensure org membership exists even on skip.
			if orgID != "" {
				tx.ExecContext(ctx,
					bindSeedQuery(`INSERT INTO memberships (resource_type, resource_id, user_id, role, added_at) VALUES ('org', ?, ?, 'member', ?) ON CONFLICT(resource_type, resource_id, user_id) DO NOTHING`, dialect),
					orgID, existingID, seedNow())
			}
			// Still process linked accounts for this existing identity.
			for _, la := range ident.LinkedAccounts {
				seedLinkedAccount(ctx, tx, existingID, la, dialect)
			}
			// Ensure capabilities and PATs exist even on skip.
			seedCapabilities(ctx, tx, existingID, ident.Capabilities)
			seedPATs(ctx, tx, existingID, ident.PATs, dialect)
			return nil
		}
	}

	newID := id.New()

	state := ident.State
	if state == "" {
		state = "active"
	}
	schemaRec, err := resolveSeedUserSchema(ctx, tx, ident.SchemaID, dialect)
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
	now := seedNow()

	_, err = tx.ExecContext(ctx,
		bindSeedQuery(`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`, dialect),
		newID, orgID, ident.Identifier, ident.DisplayName, func() string {
			if schemaRec.Type == "service_user" || schemaRec.Type == "ai_agent" {
				return schemaRec.Type
			}
			return "human"
		}(), state, schemaRec.ID, profileJSON, now, now)
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
				bindSeedQuery(`INSERT INTO credentials (id, user_id, type, data) VALUES (?, ?, 'password', ?)`, dialect),
				credID, newID, credJSON)
		}
	}

	// Seed capabilities.
	seedCapabilities(ctx, tx, newID, ident.Capabilities)

	// Seed PATs.
	seedPATs(ctx, tx, newID, ident.PATs, dialect)

	// Insert org membership (only when a default org exists).
	if orgID != "" {
		tx.ExecContext(ctx,
			bindSeedQuery(`INSERT INTO memberships (resource_type, resource_id, user_id, role, added_at) VALUES ('org', ?, ?, 'member', ?) ON CONFLICT(resource_type, resource_id, user_id) DO NOTHING`, dialect),
			orgID, newID, now)
	}

	logging.Printf("[seed] created identity %q (id: %s)", ident.Identifier, newID)

	// Process linked accounts.
	for _, la := range ident.LinkedAccounts {
		seedLinkedAccount(ctx, tx, newID, la, dialect)
	}

	return nil
}

// updateExistingIdentity handles the on_conflict: update case.
func updateExistingIdentity(ctx context.Context, tx *sql.Tx, userID string, ident SeedIdentity, dialect string) error {
	// Update password if provided.
	if ident.Password != "" {
		pw := auth.NewPasswordsDev(nil)
		hash, err := pw.Hash(ident.Password)
		if err == nil {
			// Delete existing + re-insert.
			tx.ExecContext(ctx, bindSeedQuery(`DELETE FROM credentials WHERE user_id = ? AND type = 'password'`, dialect), userID)
			credID := id.New()
			credJSON := auth.EncodeCredentialJSON(hash)
			tx.ExecContext(ctx,
				bindSeedQuery(`INSERT INTO credentials (id, user_id, type, data) VALUES (?, ?, 'password', ?)`, dialect),
				credID, userID, credJSON)
			logging.Printf("[seed]   updated password for %q", ident.Identifier)
		}
	}

	// Upsert capabilities.
	seedCapabilities(ctx, tx, userID, ident.Capabilities)

	// Upsert PATs.
	seedPATs(ctx, tx, userID, ident.PATs, dialect)

	// Process linked accounts.
	for _, la := range ident.LinkedAccounts {
		seedLinkedAccount(ctx, tx, userID, la, dialect)
	}

	return nil
}

func resolveSeedUserSchema(ctx context.Context, tx *sql.Tx, schemaID, dialect string) (*schema.SchemaRecord, error) {
	return schema.ResolveUserSchemaForWrite(ctx, tx, schemaID, dialect)
}

// seedCapabilities is a no-op — capabilities are now handled by FGA.
func seedCapabilities(_ context.Context, _ *sql.Tx, _ string, _ []string) {
	// FGA tuples are written during bootstrap; seed caps are ignored.
}

// seedPATs creates PAT tokens for an entity (idempotent via name check).
func seedPATs(ctx context.Context, tx *sql.Tx, userID string, pats []SeedPAT, dialect string) {
	for _, pat := range pats {
		// Skip if a PAT with this name already exists for this entity.
		var existingID string
		err := tx.QueryRowContext(ctx,
			bindSeedQuery(`SELECT id FROM tokens WHERE user_id = ? AND name = ? AND type = 'pat' AND revoked_at IS NULL`, dialect),
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
			bindSeedQuery(`INSERT INTO tokens (id, type, token_hash, user_id, name, scopes, created_at)
			 VALUES (?, 'pat', ?, ?, ?, ?, ?)`, dialect),
			tokenID, tokenHash, userID, pat.Name, string(scopesJSON), seedNow())
		if err != nil {
			logging.Printf("[seed]   failed to create PAT %q: %v", pat.Name, err)
			continue
		}

		logging.Printf("[seed]   created PAT %q for entity %s", pat.Name, userID)
	}
}

func seedLinkedAccount(ctx context.Context, tx *sql.Tx, userID string, la SeedLinkedAccount, dialect string) {
	// Resolve provider by name or ID from providers table.
	var providerID string
	err := tx.QueryRowContext(ctx,
		bindSeedQuery(`SELECT id FROM providers WHERE name = ? OR id = ?`, dialect),
		la.Provider, la.Provider).Scan(&providerID)
	if err != nil {
		logging.Printf("[seed] linked_account: provider %q not found, skipping", la.Provider)
		return
	}

	// Skip if already linked.
	var existingLink int64
	err = tx.QueryRowContext(ctx,
		bindSeedQuery(`SELECT id FROM linked_identities WHERE provider_id = ? AND external_sub = ?`, dialect),
		providerID, la.ExternalSub).Scan(&existingLink)
	if err == nil {
		return // already linked
	}

	linkID := id.New()
	tx.ExecContext(ctx,
		bindSeedQuery(`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
		 VALUES (?, ?, ?, ?, ?, '{}', ?)`, dialect),
		linkID, userID, providerID, la.ExternalSub, la.ExternalEmail, seedNow())

	logging.Printf("[seed] linked identity %s → provider %s (sub: %s)", userID, providerID, la.ExternalSub)
}

func generateShortID() string {
	return id.New()
}
