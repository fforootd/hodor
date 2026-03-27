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
	"github.com/zitadel/zitadel/internal/schema"
)

// EnsureAdmin checks if any entities exist. If not, it creates a default
// admin identity with a random password and prints the credentials to stdout.
func EnsureAdmin(ctx context.Context, db *database.DB) error {
	// Always seed built-in schemas (idempotent).
	if err := seedSchemas(ctx, db); err != nil {
		return fmt.Errorf("seed schemas: %w", err)
	}

	var count int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM entities`).Scan(&count)
	if err != nil {
		return fmt.Errorf("count entities: %w", err)
	}
	if count > 0 {
		return nil // Already bootstrapped.
	}

	log.Println("No entities found — bootstrapping admin account...")

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
		`INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, 'admin', 'Admin', 'active', 'human_user_v1', '{"email":"admin@zitadel.local"}', '{}', datetime('now'), datetime('now'))`,
		identityID,
	)
	if err != nil {
		return fmt.Errorf("insert admin identity: %w", err)
	}

	// Add capabilities — password + admin.
	for _, cap := range []string{"password", "admin"} {
		_, err = tx.ExecContext(ctx,
			`INSERT INTO entity_capabilities (entity_id, capability) VALUES (?, ?)`,
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
	fmt.Printf("   │  Username: admin                 \t\t\t\t │\n")
	fmt.Printf("   │  Password: %-36s  │\n", password)
	fmt.Println("  │                                                  │")
	fmt.Println("  │  Change this password on first login.            │")
	fmt.Println("  └──────────────────────────────────────────────────┘")
	fmt.Println()

	// Seed the default org.
	if err := seedDefaultOrg(ctx, db); err != nil {
		log.Printf("WARN: seed default org: %v", err)
	}

	// Seed the default console OIDC client (public SPA, no secret).
	if err := seedConsoleClient(ctx, db); err != nil {
		log.Printf("WARN: seed console client: %v", err)
	}

	return nil
}

// seedDefaultOrg creates the default organization if it doesn't exist.
func seedDefaultOrg(ctx context.Context, db *database.DB) error {
	var exists int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM entities WHERE identifier = 'default' AND schema_id = 'org_v1'`).Scan(&exists)
	if err != nil || exists > 0 {
		return nil
	}

	orgID, err := id.New()
	if err != nil {
		return fmt.Errorf("gen org id: %w", err)
	}

	orgData := `{
		"display_name": "Default",
		"branding": {
			"primary_color": "#1a1a2e"
		}
	}`

	_, err = db.SQL().ExecContext(ctx,
		`INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, data, created_at, updated_at)
		 VALUES (?, 0, 'default', 'Default', 'active', 'org_v1', ?, datetime('now'), datetime('now'))`,
		orgID, orgData,
	)
	if err != nil {
		return fmt.Errorf("insert default org: %w", err)
	}

	log.Println("seeded default organization (identifier=default)")
	return nil
}

// seedConsoleClient creates the default console OIDC client identity if it doesn't exist.
func seedConsoleClient(ctx context.Context, db *database.DB) error {
	var exists int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM entities WHERE identifier = 'console'`).Scan(&exists)
	if err != nil || exists > 0 {
		return nil // Already exists or DB error — skip silently.
	}

	consoleID, err := id.New()
	if err != nil {
		return fmt.Errorf("gen console id: %w", err)
	}

	consoleData := `{
		"client_name": "ZITADEL Console",
		"app_type": "spa",
		"redirect_uris": ["http://localhost:5173/console", "http://localhost:8080/console"],
		"post_logout_redirect_uris": ["http://localhost:5173", "http://localhost:8080"]
	}`

	_, err = db.SQL().ExecContext(ctx,
		`INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, data, created_at, updated_at)
		 VALUES (?, 1, 'console', 'ZITADEL Console', 'active', 'app_v1', ?, datetime('now'), datetime('now'))`,
		consoleID, consoleData,
	)
	if err != nil {
		return fmt.Errorf("insert console entity: %w", err)
	}

	log.Println("seeded default console OIDC client (client_id=console)")
	return nil
}

// seedSchemas reads the x-catalog from the meta schema and seeds each entity
// schema that has a schema_file into the database.
func seedSchemas(ctx context.Context, db *database.DB) error {
	catalog, err := schema.Catalog()
	if err != nil {
		return fmt.Errorf("load catalog: %w", err)
	}

	seeded := 0
	for typeName, entry := range catalog {
		if entry.SchemaFile == "" {
			continue // System views (sessions, events, jobs) have no schema file.
		}

		schemaJSON, err := schema.LoadSchemaFile(entry.SchemaFile)
		if err != nil {
			return fmt.Errorf("load schema file for %s: %w", typeName, err)
		}

		schemaID := typeName + "_v1"

		// Try with is_default column first; fall back without it for older schemas.
		_, err = db.SQL().ExecContext(ctx,
			`INSERT OR REPLACE INTO schemas (id, type, org_id, schema, version, is_default, created_at)
			 VALUES (?, ?, 0, ?, 1, true, datetime('now'))`,
			schemaID, typeName, schemaJSON,
		)
		if err != nil {
			// Column may not exist in fuzz worker subprocess or old DB.
			_, err = db.SQL().ExecContext(ctx,
				`INSERT OR REPLACE INTO schemas (id, type, org_id, schema, version, created_at)
				 VALUES (?, ?, 0, ?, 1, datetime('now'))`,
				schemaID, typeName, schemaJSON,
			)
		}
		if err != nil {
			return fmt.Errorf("seed schema %s: %w", schemaID, err)
		}
		seeded++
	}

	log.Printf("seeded %d built-in entity schemas from catalog", seeded)
	return nil
}
