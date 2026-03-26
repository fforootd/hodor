package database

import (
	"database/sql"
	"fmt"

	"github.com/pressly/goose/v3"
)

func init() {
	goose.AddMigrationNoTx(up007Providers, down007Providers)
}

// up007Providers creates the provider federation tables:
//   - providers: configured external identity sources (OIDC, SAML, SCIM)
//   - linked_accounts: user ↔ provider links
//   - sso_states: ephemeral OIDC authorization state (PKCE, nonce)
func up007Providers(db *sql.DB) error {
	dialect := detectDialect(db)

	bigint := "INTEGER"
	timestampDefault := "TEXT NOT NULL DEFAULT (datetime('now'))"

	if dialect == "postgres" {
		bigint = "BIGINT"
		timestampDefault = "TIMESTAMPTZ NOT NULL DEFAULT NOW()"
	}

	stmts := []string{
		// Providers — protocol-agnostic external identity sources.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS providers (
			id              TEXT PRIMARY KEY,
			org_id          %s NOT NULL DEFAULT 1,
			name            TEXT NOT NULL,
			protocol        TEXT NOT NULL DEFAULT 'oidc',
			template        TEXT NOT NULL DEFAULT 'custom',
			config          TEXT NOT NULL DEFAULT '{}',
			claim_overrides TEXT NOT NULL DEFAULT '{}',
			auto_register   BOOLEAN NOT NULL DEFAULT 1,
			enabled         BOOLEAN NOT NULL DEFAULT 1,
			display_order   INTEGER NOT NULL DEFAULT 0,
			created_at      %s,
			updated_at      %s
		)`, bigint, timestampDefault, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_providers_org ON providers(org_id)`,

		// Linked accounts — user ↔ external provider links.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS linked_accounts (
			id             %s PRIMARY KEY,
			identity_id    %s NOT NULL,
			provider_id    TEXT NOT NULL,
			external_sub   TEXT NOT NULL,
			external_email TEXT DEFAULT '',
			raw_claims     TEXT DEFAULT '{}',
			linked_at      %s,
			last_used_at   TEXT,
			UNIQUE(provider_id, external_sub)
		)`, bigint, bigint, timestampDefault),

		`CREATE INDEX IF NOT EXISTS idx_linked_identity ON linked_accounts(identity_id)`,
		`CREATE INDEX IF NOT EXISTS idx_linked_provider ON linked_accounts(provider_id, external_sub)`,

		// SSO states — ephemeral OIDC authorization flow state.
		fmt.Sprintf(`CREATE TABLE IF NOT EXISTS sso_states (
			state         TEXT PRIMARY KEY,
			provider_id   TEXT NOT NULL,
			pkce_verifier TEXT NOT NULL,
			nonce         TEXT NOT NULL,
			redirect_uri  TEXT DEFAULT '',
			created_at    %s
		)`, timestampDefault),
	}

	for _, stmt := range stmts {
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("migration 007 providers: %w\nSQL: %s", err, stmt)
		}
	}

	return nil
}

func down007Providers(db *sql.DB) error {
	for _, t := range []string{"sso_states", "linked_accounts", "providers"} {
		if _, err := db.Exec("DROP TABLE IF EXISTS " + t); err != nil {
			return err
		}
	}
	return nil
}
