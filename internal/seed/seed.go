// Package seed loads declarative YAML seed files and applies them to the database.
//
// Seed files describe a desired state: schemas → providers → entities → linked accounts.
// They support environment variable substitution (${VAR}) and are idempotent (on_conflict: skip).
package seed

import (
	"bytes"
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"regexp"
	"strings"

	"golang.org/x/crypto/bcrypt"
	"gopkg.in/yaml.v3"

	"github.com/zitadel/zitadel/internal/id"
)

// SeedFile represents the top-level structure of a seed YAML file.
type SeedFile struct {
	Providers  []SeedProvider `yaml:"providers"`
	Identities []SeedIdentity `yaml:"entities"`
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

// SeedIdentity defines an identity to seed.
type SeedIdentity struct {
	Identifier     string              `yaml:"identifier"`
	DisplayName    string              `yaml:"display_name"`
	SchemaID       string              `yaml:"schema_id"`
	State          string              `yaml:"state"`
	Password       string              `yaml:"password"`
	Profile        map[string]any      `yaml:"profile"`
	Capabilities   []string            `yaml:"capabilities"`    // NEW: ["admin", "password"]
	PATs           []SeedPAT           `yaml:"pats"`            // NEW: personal access tokens
	OnConflict     string              `yaml:"on_conflict"`     // NEW: "skip" (default), "warn", "update"
	LinkedAccounts []SeedLinkedAccount `yaml:"linked_accounts"`
}

// SeedPAT defines a personal access token to seed for an identity.
type SeedPAT struct {
	Name   string   `yaml:"name"`
	Token  string   `yaml:"token"`   // raw token (will be prefixed + hashed)
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

// LoadAndApply reads a seed YAML file and applies it to the database.
// It is idempotent — existing records are handled per on_conflict setting.
func LoadAndApply(ctx context.Context, db *sql.DB, path string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("read seed file %s: %w", path, err)
	}

	// Substitute environment variables.
	data = substituteEnvVars(data)

	var seed SeedFile
	decoder := yaml.NewDecoder(bytes.NewReader(data))
	if err := decoder.Decode(&seed); err != nil {
		return fmt.Errorf("parse seed file: %w", err)
	}

	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin transaction: %w", err)
	}
	defer tx.Rollback()

	// Phase 1: Providers.
	for _, p := range seed.Providers {
		if err := seedProvider(ctx, tx, p); err != nil {
			return fmt.Errorf("seed provider %q: %w", p.Name, err)
		}
	}

	// Phase 2: Identities (with inline linked accounts, capabilities, PATs).
	for _, ident := range seed.Identities {
		if err := seedIdentity(ctx, tx, ident); err != nil {
			return fmt.Errorf("seed identity %q: %w", ident.Identifier, err)
		}
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	totalItems := len(seed.Providers) + len(seed.Identities)
	log.Printf("[seed] applied %d items from %s", totalItems, path)
	return nil
}

func seedProvider(ctx context.Context, tx *sql.Tx, p SeedProvider) error {
	// Skip if exists by name.
	var existing string
	err := tx.QueryRowContext(ctx, `SELECT id FROM providers WHERE name = ?`, p.Name).Scan(&existing)
	if err == nil {
		log.Printf("[seed] provider %q already exists, skipping", p.Name)
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
	configJSON := "{}"
	if p.Config != nil {
		b, _ := json.Marshal(p.Config)
		configJSON = string(b)
	}
	overridesJSON := "{}"
	if p.ClaimOverrides != nil {
		b, _ := json.Marshal(p.ClaimOverrides)
		overridesJSON = string(b)
	}
	autoReg := true
	if p.AutoRegister != nil {
		autoReg = *p.AutoRegister
	}

	_, err = tx.ExecContext(ctx,
		`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, created_at, updated_at)
		 VALUES (?, 1, ?, ?, ?, ?, ?, ?, 1, 0, datetime('now'), datetime('now'))`,
		provID, p.Name, p.Protocol, p.Template, configJSON, overridesJSON, autoReg)
	if err != nil {
		return err
	}

	log.Printf("[seed] created provider %q (id: %s)", p.Name, provID)
	return nil
}

func seedIdentity(ctx context.Context, tx *sql.Tx, ident SeedIdentity) error {
	onConflict := ident.OnConflict
	if onConflict == "" {
		onConflict = "skip"
	}

	// Check if exists by identifier.
	var existingID int64
	err := tx.QueryRowContext(ctx, `SELECT id FROM entities WHERE identifier = ?`, ident.Identifier).Scan(&existingID)
	if err == nil {
		// Entity already exists — handle according to on_conflict.
		switch onConflict {
		case "update":
			log.Printf("[seed] identity %q already exists, updating (on_conflict: update)", ident.Identifier)
			return updateExistingIdentity(ctx, tx, existingID, ident)
		case "warn":
			log.Printf("[seed] WARN: identity %q already exists, skipping (on_conflict: warn)", ident.Identifier)
			return nil
		default: // "skip"
			log.Printf("[seed] identity %q already exists, skipping", ident.Identifier)
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

	newID, err := id.New()
	if err != nil {
		return fmt.Errorf("generate id: %w", err)
	}

	state := ident.State
	if state == "" {
		state = "active"
	}
	schemaID := ident.SchemaID
	if schemaID == "" {
		schemaID = "human_user_v1"
	}
	profileJSON := "{}"
	if ident.Profile != nil {
		b, _ := json.Marshal(ident.Profile)
		profileJSON = string(b)
	}

	_, err = tx.ExecContext(ctx,
		`INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, profile, data, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, ?, ?, ?, ?, '{}', datetime('now'), datetime('now'))`,
		newID, ident.Identifier, ident.DisplayName, state, schemaID, profileJSON, profileJSON)
	if err != nil {
		return err
	}

	// Hash password if provided.
	if ident.Password != "" {
		hash, err := bcrypt.GenerateFromPassword([]byte(ident.Password), bcrypt.DefaultCost)
		if err == nil {
			tx.ExecContext(ctx,
				`INSERT INTO passwords (entity_id, password_hash, created_at) VALUES (?, ?, datetime('now'))`,
				newID, string(hash))
		}
	}

	// Seed capabilities.
	seedCapabilities(ctx, tx, newID, ident.Capabilities)

	// Seed PATs.
	seedPATs(ctx, tx, newID, ident.PATs)

	log.Printf("[seed] created identity %q (id: %d)", ident.Identifier, newID)

	// Process linked accounts.
	for _, la := range ident.LinkedAccounts {
		seedLinkedAccount(ctx, tx, newID, la)
	}

	return nil
}

// updateExistingIdentity handles the on_conflict: update case.
func updateExistingIdentity(ctx context.Context, tx *sql.Tx, entityID int64, ident SeedIdentity) error {
	// Update password if provided.
	if ident.Password != "" {
		hash, err := bcrypt.GenerateFromPassword([]byte(ident.Password), bcrypt.DefaultCost)
		if err == nil {
			// Delete existing + re-insert.
			tx.ExecContext(ctx, `DELETE FROM passwords WHERE entity_id = ?`, entityID)
			tx.ExecContext(ctx,
				`INSERT INTO passwords (entity_id, password_hash, created_at) VALUES (?, ?, datetime('now'))`,
				entityID, string(hash))
			log.Printf("[seed]   updated password for %q", ident.Identifier)
		}
	}

	// Upsert capabilities.
	seedCapabilities(ctx, tx, entityID, ident.Capabilities)

	// Upsert PATs.
	seedPATs(ctx, tx, entityID, ident.PATs)

	// Process linked accounts.
	for _, la := range ident.LinkedAccounts {
		seedLinkedAccount(ctx, tx, entityID, la)
	}

	return nil
}

// seedCapabilities inserts capabilities for an entity (idempotent via INSERT OR IGNORE).
func seedCapabilities(ctx context.Context, tx *sql.Tx, entityID int64, caps []string) {
	for _, cap := range caps {
		tx.ExecContext(ctx,
			`INSERT OR IGNORE INTO entity_capabilities (entity_id, capability) VALUES (?, ?)`,
			entityID, cap)
	}
}

// seedPATs creates PAT tokens for an entity (idempotent via name check).
func seedPATs(ctx context.Context, tx *sql.Tx, entityID int64, pats []SeedPAT) {
	for _, pat := range pats {
		// Skip if a PAT with this name already exists for this entity.
		var existingID int64
		err := tx.QueryRowContext(ctx,
			`SELECT id FROM tokens WHERE entity_id = ? AND name = ? AND type = 'pat' AND revoked_at IS NULL`,
			entityID, pat.Name).Scan(&existingID)
		if err == nil {
			log.Printf("[seed]   PAT %q already exists for entity %d, skipping", pat.Name, entityID)
			continue
		}

		// Prefix the token if not already prefixed.
		rawToken := pat.Token
		if !strings.HasPrefix(rawToken, "zit_pat_") {
			rawToken = "zit_pat_" + rawToken
		}

		// Hash the token.
		h := sha256.Sum256([]byte(rawToken))
		tokenHash := hex.EncodeToString(h[:])

		tokenID, err := id.New()
		if err != nil {
			log.Printf("[seed]   failed to generate PAT id: %v", err)
			continue
		}

		scopes := pat.Scopes
		if len(scopes) == 0 {
			scopes = []string{"admin"}
		}
		scopesJSON, _ := json.Marshal(scopes)

		_, err = tx.ExecContext(ctx,
			`INSERT INTO tokens (id, type, token_hash, entity_id, name, scopes, created_at)
			 VALUES (?, 'pat', ?, ?, ?, ?, datetime('now'))`,
			tokenID, tokenHash, entityID, pat.Name, string(scopesJSON))
		if err != nil {
			log.Printf("[seed]   failed to create PAT %q: %v", pat.Name, err)
			continue
		}

		log.Printf("[seed]   created PAT %q for entity %d", pat.Name, entityID)
	}
}

func seedLinkedAccount(ctx context.Context, tx *sql.Tx, identityID int64, la SeedLinkedAccount) {
	// Resolve provider by name or ID.
	var providerID string
	err := tx.QueryRowContext(ctx, `SELECT id FROM providers WHERE name = ? OR id = ?`, la.Provider, la.Provider).Scan(&providerID)
	if err != nil {
		log.Printf("[seed] linked_account: provider %q not found, skipping", la.Provider)
		return
	}

	// Skip if already linked.
	var existingLink int64
	err = tx.QueryRowContext(ctx,
		`SELECT id FROM linked_accounts WHERE provider_id = ? AND external_sub = ?`,
		providerID, la.ExternalSub).Scan(&existingLink)
	if err == nil {
		return // already linked
	}

	linkID, _ := id.New()
	tx.ExecContext(ctx,
		`INSERT INTO linked_accounts (id, entity_id, provider_id, external_sub, external_email, raw_claims, linked_at)
		 VALUES (?, ?, ?, ?, ?, '{}', datetime('now'))`,
		linkID, identityID, providerID, la.ExternalSub, la.ExternalEmail)

	log.Printf("[seed] linked identity %d → provider %s (sub: %s)", identityID, providerID, la.ExternalSub)
}

func generateShortID() string {
	newID, err := id.New()
	if err != nil {
		return "unknown"
	}
	return fmt.Sprintf("%d", newID)
}
