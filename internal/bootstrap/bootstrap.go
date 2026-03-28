// Package bootstrap handles first-run initialization—creating the default
// admin identity and printing its credentials to stdout.
package bootstrap

import (
	"bufio"
	"context"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"os"
	"strings"
	"testing"

	"golang.org/x/term"

	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// EnsureAdmin checks if any entities exist. If not, it creates a default
// admin identity. The behavior depends on the seedFile parameter:
//   - If seedFile is set: skip admin creation (the seed file handles it).
//   - If no seedFile and interactive terminal: run the first-run wizard.
//   - If no seedFile and non-interactive: create admin with random password.
func EnsureAdmin(ctx context.Context, db *database.DB, seedFile string) error {
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

	// If a seed file is configured, it will handle admin creation.
	if seedFile != "" {
		logging.Println("Seed file configured — skipping bootstrap admin creation.")
		return nil
	}

	logging.Println("No entities found — bootstrapping admin account...")

	// Determine if we're running in an interactive terminal.
	if isInteractiveTerminal() {
		return runFirstRunWizard(ctx, db)
	}

	// Non-interactive fallback (CI/Docker): random password to stdout.
	return createRandomAdmin(ctx, db)
}

// isInteractiveTerminal checks if stdin is a terminal.
func isInteractiveTerminal() bool {
	return term.IsTerminal(int(os.Stdin.Fd()))
}

// runFirstRunWizard prompts the user for admin credentials.
func runFirstRunWizard(ctx context.Context, db *database.DB) error {
	fmt.Println()
	fmt.Println("  ┌──────────────────────────────────────────────────┐")
	fmt.Println("  │  Welcome to Zitadel!                             │")
	fmt.Println("  │  Let's set up your first admin account.          │")
	fmt.Println("  └──────────────────────────────────────────────────┘")
	fmt.Println()

	reader := bufio.NewReader(os.Stdin)

	// Username.
	fmt.Print("  Admin username [admin]: ")
	username, _ := reader.ReadString('\n')
	username = strings.TrimSpace(username)
	if username == "" {
		username = "admin"
	}

	// Email.
	fmt.Print("  Admin email [admin@zitadel.local]: ")
	email, _ := reader.ReadString('\n')
	email = strings.TrimSpace(email)
	if email == "" {
		email = "admin@zitadel.local"
	}

	// Password.
	fmt.Print("  Admin password: ")
	passwordBytes, err := term.ReadPassword(int(os.Stdin.Fd()))
	fmt.Println() // newline after hidden input
	if err != nil || len(passwordBytes) == 0 {
		// Fallback to visible input if terminal password reading fails.
		fmt.Print("  Admin password (visible): ")
		passwordStr, _ := reader.ReadString('\n')
		passwordBytes = []byte(strings.TrimSpace(passwordStr))
	}
	password := string(passwordBytes)
	if len(password) < 6 {
		return fmt.Errorf("password must be at least 6 characters")
	}

	// Confirm.
	fmt.Print("  Confirm password: ")
	confirmBytes, err := term.ReadPassword(int(os.Stdin.Fd()))
	fmt.Println()
	if err != nil {
		fmt.Print("  Confirm password (visible): ")
		confirmStr, _ := reader.ReadString('\n')
		confirmBytes = []byte(strings.TrimSpace(confirmStr))
	}
	if string(confirmBytes) != password {
		return fmt.Errorf("passwords do not match")
	}

	if err := createAdmin(ctx, db, username, email, password); err != nil {
		return err
	}

	fmt.Println()
	fmt.Printf("  ✓ Admin account created: %s\n", username)
	fmt.Println()

	return nil
}

// createRandomAdmin creates an admin with a random password and prints it.
func createRandomAdmin(ctx context.Context, db *database.DB) error {
	password, err := auth.GenerateRandomPassword(16)
	if err != nil {
		return fmt.Errorf("generate password: %w", err)
	}

	if err := createAdmin(ctx, db, "admin", "admin@zitadel.local", password); err != nil {
		return err
	}

	// Suppress banner during test runs — tests get the password
	// via DB query, not stdout.
	if testing.Testing() {
		logging.Printf("bootstrapped admin (password=%s)", password)
		return nil
	}

	fmt.Println()
	fmt.Println("  ┌──────────────────────────────────────────────────┐")
	fmt.Println("  │  Zitadel bootstrapped!                          │")
	fmt.Printf("   │  Username: admin                 \t\t\t\t │\n")
	fmt.Printf("   │  Password: %-36s  │\n", password)
	fmt.Println("  │                                                  │")
	fmt.Println("  │  Change this password on first login.            │")
	fmt.Println("  └──────────────────────────────────────────────────┘")
	fmt.Println()

	return nil
}

// createAdmin inserts the admin identity with capabilities and password.
func createAdmin(ctx context.Context, db *database.DB, username, email, password string) error {
	identityID := id.New()

	profileJSON := fmt.Sprintf(`{"email":%q}`, email)

	tx, err := db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	// Create the admin identity using the human_user schema.
	_, err = tx.ExecContext(ctx,
		`INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, 'Admin', 'active', 'human_user_v1', ?, '{}', datetime('now'), datetime('now'))`,
		identityID, username, profileJSON,
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

	// Enforce uniqueness (ADR-016): register identifier + email in unique_fields.
	if err := uniqueness.EnforceFromIdentifier(ctx, tx, identityID, "1", username); err != nil {
		logging.Printf("WARN: bootstrap unique identifier: %v", err)
	}
	// Also register email at instance scope.
	_ = uniqueness.Enforce(ctx, tx, identityID, "1",
		[]uniqueness.FieldConstraint{{FieldName: "email", Scope: uniqueness.ScopeInstance}},
		map[string]any{"email": email},
	)

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	// Set password (outside tx — uses its own transaction).
	pw := auth.NewPasswords(db)
	if err := pw.SetPassword(ctx, identityID, password); err != nil {
		return fmt.Errorf("set admin password: %w", err)
	}

	// Seed the default org.
	if err := seedDefaultOrg(ctx, db); err != nil {
		logging.Printf("WARN: seed default org: %v", err)
	}

	// Seed the default console OIDC client (public SPA, no secret).
	if err := seedConsoleClient(ctx, db); err != nil {
		logging.Printf("WARN: seed console client: %v", err)
	}

	// Bootstrap FGA tuples: admin → instance:owner, org parent, org owner.
	if fgaSvc := api.FGAService; fgaSvc != nil {
		// Use org_id from the admin entity (numeric, matches middleware resolution).
		var orgID string
		err := db.SQL().QueryRowContext(ctx,
			`SELECT org_id FROM entities WHERE identifier = ? LIMIT 1`,
			username,
		).Scan(&orgID)
		if err != nil || orgID == "" {
			logging.Printf("WARN: could not find org_id for FGA bootstrap: %v", err)
		} else {
			if err := fgaSvc.OnBootstrap(ctx, identityID, orgID); err != nil {
				logging.Printf("WARN: FGA bootstrap tuples failed: %v", err)
			} else {
				logging.Printf("[fga] bootstrap tuples written: admin=%s org=%s", identityID, orgID)
			}
		}
	}

	return nil
}

// seedDefaultOrg creates the default organization if it doesn't exist.
func seedDefaultOrg(ctx context.Context, db *database.DB) error {
	var exists int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM entities WHERE identifier = 'default' AND schema_id = 'org_v1'`).Scan(&exists)
	if err != nil {
		return fmt.Errorf("check default org: %w", err)
	}
	if exists > 0 {
		return nil
	}

	orgID := id.New()

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

	logging.Println("seeded default organization (identifier=default)")
	return nil
}

// seedConsoleClient creates the default console OIDC client identity if it doesn't exist.
func seedConsoleClient(ctx context.Context, db *database.DB) error {
	var exists int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM entities WHERE identifier = 'console'`).Scan(&exists)
	if err != nil {
		return fmt.Errorf("check console client: %w", err)
	}
	if exists > 0 {
		return nil
	}

	consoleID := id.New()

	consoleData := `{
		"client_name": "Zitadel Console",
		"app_type": "spa",
		"redirect_uris": ["http://localhost:5173/console", "http://localhost:8080/console"],
		"post_logout_redirect_uris": ["http://localhost:5173", "http://localhost:8080"]
	}`

	_, err = db.SQL().ExecContext(ctx,
		`INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, data, created_at, updated_at)
		 VALUES (?, 1, 'console', 'Zitadel Console', 'active', 'app_v1', ?, datetime('now'), datetime('now'))`,
		consoleID, consoleData,
	)
	if err != nil {
		return fmt.Errorf("insert console entity: %w", err)
	}

	logging.Println("seeded default console OIDC client (client_id=console)")
	return nil
}

// seedSchemas reads the x-catalog from the meta schema and seeds each entity
// schema that has a $ref into the database.
func seedSchemas(ctx context.Context, db *database.DB) error {
	catalog, err := schema.Catalog()
	if err != nil {
		return fmt.Errorf("load catalog: %w", err)
	}

	seeded := 0
	for typeName, entry := range catalog {
		if entry.Ref == "" {
			continue // System views (sessions, events, jobs, schema) have no $ref.
		}

		schemaJSON, err := schema.LoadSchemaFile(entry.Ref)
		if err != nil {
			return fmt.Errorf("load schema file for %s: %w", typeName, err)
		}

		schemaID := typeName + "_v1"

		// Try with visibility column first; fall back without it for older schemas.
		_, err = db.SQL().ExecContext(ctx,
			`INSERT OR REPLACE INTO schemas (id, type, org_id, schema, version, is_default, visibility, created_at)
			 VALUES (?, ?, 0, ?, 1, true, 'public', datetime('now'))`,
			schemaID, typeName, schemaJSON,
		)
		if err != nil {
			// Column may not exist in fuzz worker subprocess or old DB.
			_, err = db.SQL().ExecContext(ctx,
				`INSERT OR REPLACE INTO schemas (id, type, org_id, schema, version, is_default, created_at)
				 VALUES (?, ?, 0, ?, 1, true, datetime('now'))`,
				schemaID, typeName, schemaJSON,
			)
		}
		if err != nil {
			return fmt.Errorf("seed schema %s: %w", schemaID, err)
		}
		seeded++
	}

	logging.Printf("seeded %d built-in entity schemas from catalog", seeded)
	return nil
}
