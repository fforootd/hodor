// Package bootstrap handles first-run initialization—creating the default
// admin user and printing its credentials to stdout.
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

// EnsureAdmin checks if any users exist. If not, it creates a default
// admin user. The behavior depends on the seedFile parameter:
//   - If seedFile is set: skip admin creation (the seed file handles it).
//   - If no seedFile and interactive terminal: run the first-run wizard.
//   - If no seedFile and non-interactive: create admin with random password.
func EnsureAdmin(ctx context.Context, db *database.DB, seedFile string) error {
	// Always seed built-in schemas (idempotent).
	if err := seedSchemas(ctx, db); err != nil {
		return fmt.Errorf("seed schemas: %w", err)
	}

	var count int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM users`).Scan(&count)
	if err != nil {
		return fmt.Errorf("count users: %w", err)
	}
	if count > 0 {
		return nil // Already bootstrapped.
	}

	// If a seed file is configured, it will handle admin creation.
	if seedFile != "" {
		logging.Println("Seed file configured — skipping bootstrap admin creation.")
		return nil
	}

	logging.Println("No users found — bootstrapping admin account...")

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

// createAdmin inserts the admin user with password credentials.
func createAdmin(ctx context.Context, db *database.DB, username, email, password string) error {
	userID := id.New()

	// Seed the default org first so we can reference its real ID.
	orgID, err := seedDefaultOrg(ctx, db)
	if err != nil {
		logging.Printf("WARN: seed default org: %v", err)
		orgID = "org_default" // fallback
	}

	tx, err := db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	// Create the admin user with the actual default org ID.
	_, err = tx.ExecContext(ctx,
		`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, 'Admin', 'human', 'active', 'human_user_v1', ?, datetime('now'), datetime('now'))`,
		userID, orgID, username, fmt.Sprintf(`{"email":%q}`, email),
	)
	if err != nil {
		return fmt.Errorf("insert admin user: %w", err)
	}

	// Enforce uniqueness (ADR-016): register identifier + email in unique_fields.
	if err := uniqueness.EnforceFromIdentifier(ctx, tx, userID, orgID, username); err != nil {
		logging.Printf("WARN: bootstrap unique identifier: %v", err)
	}
	// Also register email at instance scope.
	_ = uniqueness.Enforce(ctx, tx, userID, orgID,
		[]uniqueness.FieldConstraint{{FieldName: "email", Scope: uniqueness.ScopeInstance}},
		map[string]any{"email": email},
	)

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	// Set password (outside tx — uses its own transaction).
	pw := auth.NewPasswords(db)
	if err := pw.SetPassword(ctx, userID, password); err != nil {
		return fmt.Errorf("set admin password: %w", err)
	}

	// Seed the default console OIDC client.
	if err := seedConsoleClient(ctx, db); err != nil {
		logging.Printf("WARN: seed console client: %v", err)
	}

	// Seed the default login flow.
	if err := seedDefaultLoginFlow(ctx, db); err != nil {
		logging.Printf("WARN: seed default login flow: %v", err)
	}

	// Bootstrap FGA tuples using the real org ID.
	if fgaSvc := api.FGAService; fgaSvc != nil {
		if err := fgaSvc.OnBootstrap(ctx, userID, orgID); err != nil {
			logging.Printf("WARN: FGA bootstrap tuples failed: %v", err)
		} else {
			logging.Printf("[fga] bootstrap tuples written: admin=%s org=%s", userID, orgID)
		}
	}

	return nil
}

// seedDefaultOrg creates the default organization in the orgs table if it doesn't exist.
// Returns the org ID (existing or newly created).
func seedDefaultOrg(ctx context.Context, db *database.DB) (string, error) {
	// Check if default org already exists and return its ID.
	var existingID string
	err := db.SQL().QueryRowContext(ctx, `SELECT id FROM orgs WHERE name = 'Default' LIMIT 1`).Scan(&existingID)
	if err == nil && existingID != "" {
		return existingID, nil
	}

	orgID := id.New()

	_, err = db.SQL().ExecContext(ctx,
		`INSERT INTO orgs (id, instance_id, name, state, metadata, created_at, updated_at)
		 VALUES (?, 'inst_default', 'Default', 'active', '{"branding":{"primary_color":"#1a1a2e"}}', datetime('now'), datetime('now'))`,
		orgID,
	)
	if err != nil {
		return "", fmt.Errorf("insert default org: %w", err)
	}

	logging.Println("seeded default organization (name=Default)")
	return orgID, nil
}

// seedConsoleClient creates the default console OIDC client in the apps table.
func seedConsoleClient(ctx context.Context, db *database.DB) error {
	var exists int
	err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM apps WHERE client_id = 'console'`).Scan(&exists)
	if err != nil {
		return fmt.Errorf("check console client: %w", err)
	}
	if exists > 0 {
		return nil
	}

	consoleID := id.New()
	redirectURIs := `["http://localhost:5173/console", "http://localhost:8080/console"]`

	_, err = db.SQL().ExecContext(ctx,
		`INSERT INTO apps (id, org_id, name, app_type, client_id, redirect_uris, state, schema_id, created_at, updated_at)
		 VALUES (?, '1', 'Zitadel Console', 'oidc', 'console', ?, 'active', 'app_v1', datetime('now'), datetime('now'))`,
		consoleID, redirectURIs,
	)
	if err != nil {
		return fmt.Errorf("insert console app: %w", err)
	}

	logging.Println("seeded default console OIDC client (client_id=console)")
	return nil
}

// seedSchemas reads the x-catalog from the meta schema and seeds each
// schema into the database.
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

	logging.Printf("seeded %d built-in schemas from catalog", seeded)
	return nil
}

// seedDefaultLoginFlow creates the default login flow if none exists.
// This is the bootstrap seed for the instance-level default flow.
func seedDefaultLoginFlow(ctx context.Context, db *database.DB) error {
	var exists int
	err := db.SQL().QueryRowContext(ctx,
		`SELECT COUNT(*) FROM login_flows WHERE is_default = 1 OR is_default = true`).Scan(&exists)
	if err != nil {
		return fmt.Errorf("check default login flow: %w", err)
	}
	if exists > 0 {
		return nil
	}

	flowID := id.New()
	defaultConfig := `{"captcha":{"provider":"altcha","mode":"risk_based","difficulty":3},"fingerprint":{"enabled":true,"provider":"thumbmarkjs"},"rate_limit":{"max_attempts":5,"window_seconds":300,"scope":"ip"},"telemetry":{"enabled":true,"sample_rate":1.0}}`

	_, err = db.SQL().ExecContext(ctx,
		`INSERT INTO login_flows (id, name, preset, is_default, enabled, state, priority, config, audience, auth_methods, created_at, updated_at)
		 VALUES (?, 'Default Login', 'identifier_first', 1, 1, 'active', 0, ?, '{}', '{}', datetime('now'), datetime('now'))`,
		flowID, defaultConfig,
	)
	if err != nil {
		return fmt.Errorf("insert default login flow: %w", err)
	}

	logging.Printf("seeded default login flow (id=%s)", flowID)
	return nil
}
