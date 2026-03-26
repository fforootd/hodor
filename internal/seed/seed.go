// Package seed loads declarative YAML seed files and applies them to the database.
//
// Seed files describe a desired state: schemas → providers → identities → linked accounts.
// They support environment variable substitution (${VAR}) and are idempotent (on_conflict: skip).
package seed

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"regexp"

	"golang.org/x/crypto/bcrypt"
	"gopkg.in/yaml.v3"

	"github.com/zitadel/zitadel/internal/id"
)

// SeedFile represents the top-level structure of a seed YAML file.
type SeedFile struct {
	Providers  []SeedProvider `yaml:"providers"`
	Identities []SeedIdentity `yaml:"identities"`
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
	LinkedAccounts []SeedLinkedAccount `yaml:"linked_accounts"`
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
// It is idempotent — existing records are skipped.
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

	// Phase 2: Identities (with inline linked accounts).
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
	// Skip if exists by identifier.
	var existingID int64
	err := tx.QueryRowContext(ctx, `SELECT id FROM identities WHERE identifier = ?`, ident.Identifier).Scan(&existingID)
	if err == nil {
		log.Printf("[seed] identity %q already exists, skipping", ident.Identifier)
		// Still process linked accounts for this existing identity.
		for _, la := range ident.LinkedAccounts {
			seedLinkedAccount(ctx, tx, existingID, la)
		}
		return nil
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
		`INSERT INTO identities (id, org_id, identifier, display_name, state, schema_id, profile, data, metadata, created_at, updated_at)
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
				`INSERT INTO passwords (identity_id, password_hash, created_at) VALUES (?, ?, datetime('now'))`,
				newID, string(hash))
		}
	}

	log.Printf("[seed] created identity %q (id: %d)", ident.Identifier, newID)

	// Process linked accounts.
	for _, la := range ident.LinkedAccounts {
		seedLinkedAccount(ctx, tx, newID, la)
	}

	return nil
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
		`INSERT INTO linked_accounts (id, identity_id, provider_id, external_sub, external_email, raw_claims, linked_at)
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
