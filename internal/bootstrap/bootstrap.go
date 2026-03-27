// Package bootstrap handles first-run initialization—creating the default
// admin identity and printing its credentials to stdout.
package bootstrap

import (
	"context"
	"fmt"
	"log"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
)

// Built-in identity schemas shipped with every ZITADEL instance.
var builtinSchemas = []struct {
	ID     string
	Type   string
	Schema string
}{
	{
		ID:   "human_user_v1",
		Type: "human_user",
		Schema: `{
  "type": "object",
  "x-auth-methods": {
    "password":    {"enabled": true,  "interactive": true,  "position": 1},
    "passkey":     {"enabled": false, "interactive": true,  "position": 0},
    "magic_link":  {"enabled": true,  "interactive": true,  "position": 2},
    "sso":         {"enabled": true,  "interactive": true,  "position": 3},
    "pat":         {"enabled": false, "interactive": false}
  },
  "x-login": {
    "preset": "identifier_first",
    "mfa_required": false,
    "registration_allowed": true
  },
  "properties": {
    "display_name":  {"type": "string", "description": "Full name shown in UI", "x-user-editable": true, "x-claim-mapping": "claims.name ?? (claims.given_name + ' ' + claims.family_name)"},
    "email":         {"type": "string", "format": "email", "x-user-editable": true, "x-claim-mapping": "claims.email", "x-auth": {"identifier": true, "verification": "email", "recovery": "email"}},
    "phone":         {"type": "string", "x-user-editable": true, "x-sensitive": true, "x-claim-mapping": "claims.phone_number ?? ''", "x-auth": {"identifier": true, "mfa": "sms"}},
    "locale":        {"type": "string", "description": "BCP-47 language tag, e.g. en-US", "x-user-editable": true, "x-claim-mapping": "claims.locale ?? ''"},
    "timezone":      {"type": "string", "description": "IANA timezone, e.g. America/New_York", "x-user-editable": true, "x-claim-mapping": "claims.zoneinfo ?? ''"},
    "avatar_url":    {"type": "string", "format": "uri", "x-user-editable": true, "x-claim-mapping": "claims.picture ?? ''"},
    "metadata":      {"type": "object", "description": "Arbitrary key-value pairs", "x-user-editable": false, "x-hidden": true, "x-source": "admin"}
  },
  "required": ["display_name"]
}`,
	},
	{
		ID:   "service_user_v1",
		Type: "service_user",
		Schema: `{
  "type": "object",
  "x-auth-methods": {
    "pat":         {"enabled": true,  "interactive": false, "max_tokens": 10},
    "api_key":     {"enabled": true,  "interactive": false},
    "password":    {"enabled": true,  "interactive": true,  "position": 1}
  },
  "x-login": {
    "preset": "identifier_first"
  },
  "properties": {
    "display_name":  {"type": "string", "description": "Service account name"},
    "description":   {"type": "string", "description": "What this service does"},
    "owner":         {"type": "string", "description": "Team or person responsible"},
    "api_scopes":    {"type": "array", "items": {"type": "string"}, "description": "Allowed API scopes"},
    "rate_limit":    {"type": "integer", "description": "Requests per minute, 0 = unlimited"},
    "expires_at":    {"type": "string", "format": "date-time", "description": "Optional expiry"},
    "metadata":      {"type": "object"}
  },
  "required": ["display_name"]
}`,
	},
	{
		ID:   "app_v1",
		Type: "app",
		Schema: `{
  "type": "object",
  "properties": {
    "display_name":   {"type": "string", "description": "Application name"},
    "description":    {"type": "string"},
    "app_type":       {"type": "string", "enum": ["web", "native", "spa", "m2m"], "description": "Application type"},
    "redirect_uris":  {"type": "array", "items": {"type": "string", "format": "uri"}, "description": "OAuth redirect URIs"},
    "grant_types":    {"type": "array", "items": {"type": "string", "enum": ["authorization_code", "client_credentials", "refresh_token", "device_code"]}, "description": "Allowed OAuth grant types"},
    "logo_url":       {"type": "string", "format": "uri"},
    "homepage_url":   {"type": "string", "format": "uri"},
    "metadata":       {"type": "object"}
  },
  "required": ["display_name", "app_type"]
}`,
	},
	{
		ID:   "ai_agent_v1",
		Type: "ai_agent",
		Schema: `{
  "type": "object",
  "x-auth-methods": {
    "pat":          {"enabled": true,  "interactive": false, "max_tokens": 5},
    "client_cert":  {"enabled": false, "interactive": false}
  },
  "properties": {
    "display_name":    {"type": "string", "description": "Agent name"},
    "description":     {"type": "string", "description": "What this agent does"},
    "model":           {"type": "string", "description": "Model identifier, e.g. gpt-4o, claude-3.5-sonnet"},
    "provider":        {"type": "string", "description": "AI provider, e.g. openai, anthropic, google"},
    "tool_access":     {"type": "array", "items": {"type": "string"}, "description": "Tools/APIs this agent can invoke"},
    "max_tokens":      {"type": "integer", "description": "Max token budget per request"},
    "delegation_chain": {"type": "string", "description": "Parent identity that delegated authority"},
    "trust_level":     {"type": "string", "enum": ["sandboxed", "supervised", "autonomous"], "description": "Level of autonomous action allowed"},
    "metadata":        {"type": "object"}
  },
  "required": ["display_name", "model", "trust_level"]
}`,
	},
}

// EnsureAdmin checks if any identities exist. If not, it creates a default
// admin identity with a random password and prints the credentials to stdout.
func EnsureAdmin(ctx context.Context, db *database.DB) error {
	// Always seed built-in schemas (idempotent).
	if err := seedSchemas(ctx, db); err != nil {
		return fmt.Errorf("seed schemas: %w", err)
	}

	var count int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM identities`).Scan(&count)
	if err != nil {
		return fmt.Errorf("count identities: %w", err)
	}
	if count > 0 {
		return nil // Already bootstrapped.
	}

	log.Println("No identities found — bootstrapping admin account...")

	password, err := auth.GenerateRandomPassword(16)
	if err != nil {
		return fmt.Errorf("generate password: %w", err)
	}

	identityID, err := id.New()
	if err != nil {
		return fmt.Errorf("generate identity id: %w", err)
	}

	tx, err := db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	// Create the admin identity using the human_user schema.
	_, err = tx.ExecContext(ctx,
		`INSERT INTO identities (id, org_id, identifier, display_name, state, schema_id, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, 'admin', 'Admin', 'active', 'human_user_v1', '{"email":"admin@zitadel.local"}', '{}', datetime('now'), datetime('now'))`,
		identityID,
	)
	if err != nil {
		return fmt.Errorf("insert admin identity: %w", err)
	}

	// Add capabilities — password + admin.
	for _, cap := range []string{"password", "admin"} {
		_, err = tx.ExecContext(ctx,
			`INSERT INTO identity_capabilities (identity_id, capability) VALUES (?, ?)`,
			identityID, cap,
		)
		if err != nil {
			return fmt.Errorf("insert capability %s: %w", cap, err)
		}
	}

	// Promote display_name to entity_indexes.
	_, _ = tx.ExecContext(ctx,
		`INSERT INTO entity_indexes (entity_type, entity_id, field, value) VALUES ('identity', ?, 'display_name', 'Admin')`,
		identityID)

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	// Set password (outside tx — uses its own transaction).
	pw := auth.NewPasswords(db)
	if err := pw.SetPassword(ctx, identityID, password); err != nil {
		return fmt.Errorf("set admin password: %w", err)
	}

	fmt.Println()
	fmt.Println("  ┌──────────────────────────────────────────────────┐")
	fmt.Println("  │  ZITADEL bootstrapped!                          │")
	fmt.Printf("   │  Username: admin                 				 │\n")
	fmt.Printf("   │  Password: %-36s  │\n", password)
	fmt.Println("  │                                                  │")
	fmt.Println("  │  Change this password on first login.            │")
	fmt.Println("  └──────────────────────────────────────────────────┘")
	fmt.Println()

	return nil
}

// seedSchemas inserts or updates the built-in identity schemas.
func seedSchemas(ctx context.Context, db *database.DB) error {
	for _, s := range builtinSchemas {
		// Try with is_default column first; fall back without it for older schemas.
		_, err := db.SQL().ExecContext(ctx,
			`INSERT OR REPLACE INTO schemas (id, type, org_id, schema, version, is_default, created_at)
			 VALUES (?, ?, 0, ?, 1, true, datetime('now'))`,
			s.ID, s.Type, s.Schema,
		)
		if err != nil {
			// Column may not exist in fuzz worker subprocess or old DB.
			_, err = db.SQL().ExecContext(ctx,
				`INSERT OR REPLACE INTO schemas (id, type, org_id, schema, version, created_at)
				 VALUES (?, ?, 0, ?, 1, datetime('now'))`,
				s.ID, s.Type, s.Schema,
			)
		}
		if err != nil {
			return fmt.Errorf("seed schema %s: %w", s.ID, err)
		}
	}
	log.Printf("seeded %d built-in identity schemas", len(builtinSchemas))
	return nil
}
