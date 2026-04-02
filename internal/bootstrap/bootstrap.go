// Package bootstrap handles first-run initialization—creating the default
// admin user and printing its credentials to stdout.
package bootstrap

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/schema"
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

	// Always seed the default login flow (idempotent).
	if err := seedDefaultLoginFlow(ctx, db); err != nil {
		logging.Printf("WARN: seed default login flow: %v", err)
	}

	hasUsers, err := HasAnyUsers(ctx, db)
	if err != nil {
		return err
	}
	if hasUsers {
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
	return IsInteractive(os.Stdin)
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

	password, err := PromptPassword(os.Stdin, os.Stdout, "  Admin password: ", "  Confirm password: ")
	if err != nil {
		return err
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
	record, err := CreateAdmin(ctx, db, CreateAdminOptions{
		Username:  username,
		Email:     email,
		Password:  password,
		Passwords: auth.NewPasswordsDev(db),
	})
	if err != nil {
		return err
	}

	// Seed the default console OIDC client.
	if err := seedConsoleClient(ctx, db); err != nil {
		logging.Printf("WARN: seed console client: %v", err)
	}

	// Bootstrap FGA tuples (instance-level only; no default org).
	if fgaSvc := api.FGAService; fgaSvc != nil {
		if err := fgaSvc.OnBootstrap(ctx, record.UserID); err != nil {
			logging.Printf("WARN: FGA bootstrap tuples failed: %v", err)
		} else {
			logging.Printf("[fga] bootstrap tuples written: admin=%s", record.UserID)
		}
	}

	return nil
}

// seedConsoleClient creates the default console OIDC client in the apps table.
func seedConsoleClient(ctx context.Context, db *database.DB) error {
	instanceID := httputil.DefaultInstanceID
	var exists int
	err := db.SQL().QueryRowContext(ctx, fmt.Sprintf(`SELECT COUNT(*) FROM apps WHERE client_id = 'console' AND instance_id = %s`, db.Placeholder(1)), instanceID).Scan(&exists)
	if err != nil {
		return fmt.Errorf("check console client: %w", err)
	}
	if exists > 0 {
		return nil
	}

	consoleID := id.New()
	redirectURIs := `["http://localhost:5173/console", "http://localhost:8080/console"]`

	query := fmt.Sprintf(`INSERT INTO apps (id, instance_id, org_id, name, app_type, client_id, redirect_uris, state, schema_id, created_at, updated_at)
		 VALUES (%s, %s, '', 'Zitadel Console', 'oidc', 'console', %s, 'active', 'app_v1', %s, %s)`,
		db.Placeholder(1), db.Placeholder(2), db.Placeholder(3), db.TimestampNow(), db.TimestampNow())

	_, err = db.SQL().ExecContext(ctx, query,
		consoleID, instanceID, redirectURIs,
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
	typeNames := make([]string, 0, len(catalog))
	for typeName := range catalog {
		typeNames = append(typeNames, typeName)
	}
	sort.Strings(typeNames)

	for _, typeName := range typeNames {
		entry := catalog[typeName]
		if entry.Ref == "" {
			continue // System views (sessions, events, jobs, schema) have no $ref.
		}

		schemaJSON, err := schema.LoadSchemaFile(entry.Ref)
		if err != nil {
			return fmt.Errorf("load schema file for %s: %w", typeName, err)
		}

		schemaID := typeName + "_v1"

		_, err = db.SQL().ExecContext(ctx,
			fmt.Sprintf(`INSERT INTO schemas (id, type, schema, version, is_default, visibility, created_at)
			 VALUES (%s, %s, %s, 1, true, 'public', %s)
			 ON CONFLICT(id) DO UPDATE SET
			 	type = excluded.type,
			 	schema = excluded.schema,
			 	version = excluded.version,
			 	is_default = excluded.is_default,
			 	visibility = excluded.visibility,
			 	created_at = excluded.created_at`,
				db.Placeholder(1), db.Placeholder(2), db.Placeholder(3), db.Placeholder(4)),
			schemaID, typeName, schemaJSON, time.Now().UTC().Format(time.RFC3339),
		)
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
	instanceID := httputil.DefaultInstanceID
	var exists int
	err := db.SQL().QueryRowContext(ctx,
		fmt.Sprintf(`SELECT COUNT(*) FROM login_flows WHERE (is_default = 1 OR is_default = true) AND instance_id = %s`, db.Placeholder(1)), instanceID).Scan(&exists)
	if err != nil {
		return fmt.Errorf("check default login flow: %w", err)
	}
	if exists > 0 {
		return nil
	}

	flowID := id.New()
	defaultConfig := `{"captcha":{"provider":"altcha","mode":"never","difficulty":3},"fingerprint":{"enabled":true,"provider":"thumbmarkjs"},"rate_limit":{"max_attempts":5,"window_seconds":300,"scope":"ip"},"telemetry":{"enabled":true,"sample_rate":1.0}}`

	_, err = db.SQL().ExecContext(ctx,
		fmt.Sprintf(`INSERT INTO login_flows (id, instance_id, name, strategy, is_default, enabled, state, priority, config, audience, auth_methods, schema_id, created_at, updated_at)
		 VALUES (%s, %s, 'Default Login', 'identifier_first', true, true, 'active', 0, %s, '{}', '{}', 'login_flow_v1', %s, %s)`,
			db.Placeholder(1), db.Placeholder(2), db.Placeholder(3), db.Placeholder(4), db.Placeholder(5)),
		flowID, instanceID, defaultConfig, time.Now().UTC().Format(time.RFC3339), time.Now().UTC().Format(time.RFC3339),
	)
	if err != nil {
		return fmt.Errorf("insert default login flow: %w", err)
	}

	logging.Printf("seeded default login flow (id=%s)", flowID)
	return nil
}
