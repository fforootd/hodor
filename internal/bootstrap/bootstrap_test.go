package bootstrap

import (
	"context"
	"testing"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/database"
)

func newBootstrapTestDB(t *testing.T) *database.DB {
	t.Helper()

	dir := t.TempDir()
	db, err := database.Open("sqlite://" + dir + "/test.db")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	t.Cleanup(func() { db.Close() })

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	return db
}

func TestEnsureAdmin_Idempotent(t *testing.T) {
	db := newBootstrapTestDB(t)

	ctx := context.Background()

	// First call should bootstrap the admin.
	if err := EnsureAdmin(ctx, db, ""); err != nil {
		t.Fatalf("first EnsureAdmin: %v", err)
	}

	var count1 int
	db.SQL().QueryRow("SELECT COUNT(*) FROM users").Scan(&count1)
	if count1 != 1 {
		t.Fatalf("expected 1 user after first bootstrap (admin), got %d", count1)
	}

	// Second call should be a no-op (idempotent).
	if err := EnsureAdmin(ctx, db, ""); err != nil {
		t.Fatalf("second EnsureAdmin: %v", err)
	}

	var count2 int
	db.SQL().QueryRow("SELECT COUNT(*) FROM users").Scan(&count2)
	if count2 != 1 {
		t.Fatalf("expected 1 user after second bootstrap (idempotent), got %d", count2)
	}
}

func TestEnsureAdmin_SeedsSchemas(t *testing.T) {
	db := newBootstrapTestDB(t)

	ctx := context.Background()
	if err := EnsureAdmin(ctx, db, ""); err != nil {
		t.Fatalf("EnsureAdmin: %v", err)
	}

	// Verify built-in schemas were seeded.
	var schemaCount int
	db.SQL().QueryRow("SELECT COUNT(*) FROM schemas WHERE visibility = 'public'").Scan(&schemaCount)
	if schemaCount < 5 {
		t.Errorf("expected at least 5 built-in schemas, got %d", schemaCount)
	}
}

func TestEnsureAdmin_AdminHasCapabilities(t *testing.T) {
	db := newBootstrapTestDB(t)

	ctx := context.Background()
	if err := EnsureAdmin(ctx, db, ""); err != nil {
		t.Fatalf("EnsureAdmin: %v", err)
	}

	// Admin should exist in users table with active state.
	var userType, state string
	db.SQL().QueryRow(`SELECT user_type, state FROM users WHERE identifier = 'admin'`).Scan(&userType, &state)
	if userType != "human" {
		t.Errorf("expected admin user_type=human, got %q", userType)
	}
	if state != "active" {
		t.Errorf("expected admin state=active, got %q", state)
	}

	// Verify admin has a password credential set.
	var credCount int
	db.SQL().QueryRow(`SELECT COUNT(*) FROM credentials
		WHERE user_id = (SELECT id FROM users WHERE identifier = 'admin')`).Scan(&credCount)
	if credCount < 1 {
		t.Errorf("expected at least 1 credential for admin, got %d", credCount)
	}
}

func TestCreateAdmin_ExplicitBootstrapCreatesOneAdmin(t *testing.T) {
	db := newBootstrapTestDB(t)
	ctx := context.Background()

	if err := SeedSystem(ctx, db); err != nil {
		t.Fatalf("SeedSystem: %v", err)
	}

	hasUsers, err := HasAnyUsers(ctx, db)
	if err != nil {
		t.Fatalf("HasAnyUsers before create: %v", err)
	}
	if hasUsers {
		t.Fatalf("expected fresh DB to have no users")
	}

	record, err := CreateAdmin(ctx, db, CreateAdminOptions{
		Username:  "admin",
		Email:     "admin@zitadel.local",
		Password:  "super-secret-password",
		Passwords: auth.NewPasswords(db),
	})
	if err != nil {
		t.Fatalf("CreateAdmin: %v", err)
	}
	if !record.Created {
		t.Fatalf("expected created record")
	}

	var userCount int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM users`).Scan(&userCount); err != nil {
		t.Fatalf("count users: %v", err)
	}
	if userCount != 1 {
		t.Fatalf("expected 1 user, got %d", userCount)
	}

	var credCount int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM credentials WHERE user_id = ?`, record.UserID).Scan(&credCount); err != nil {
		t.Fatalf("count credentials: %v", err)
	}
	if credCount != 1 {
		t.Fatalf("expected 1 credential, got %d", credCount)
	}

	var consoleCount int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM apps WHERE client_id = 'console'`).Scan(&consoleCount); err != nil {
		t.Fatalf("count console apps: %v", err)
	}
	if consoleCount != 1 {
		t.Fatalf("expected default console client to be seeded, got %d", consoleCount)
	}
}

func TestRecoverAdmin_ExistingUserResetsPasswordAndReactivates(t *testing.T) {
	db := newBootstrapTestDB(t)
	ctx := context.Background()

	if err := SeedSystem(ctx, db); err != nil {
		t.Fatalf("SeedSystem: %v", err)
	}
	record, err := CreateAdmin(ctx, db, CreateAdminOptions{
		Username:  "admin",
		Email:     "admin@zitadel.local",
		Password:  "old-secret-password",
		Passwords: auth.NewPasswords(db),
	})
	if err != nil {
		t.Fatalf("CreateAdmin: %v", err)
	}

	if _, err := db.SQL().Exec(`UPDATE users SET state = 'disabled' WHERE id = ?`, record.UserID); err != nil {
		t.Fatalf("disable user: %v", err)
	}

	recovered, err := RecoverAdmin(ctx, db, RecoverAdminOptions{
		Identifier: "admin",
		Password:   "new-secret-password",
		Passwords:  auth.NewPasswords(db),
	})
	if err != nil {
		t.Fatalf("RecoverAdmin: %v", err)
	}
	if recovered.Created {
		t.Fatalf("expected recovery to reuse the existing user")
	}

	var state string
	if err := db.SQL().QueryRow(`SELECT state FROM users WHERE id = ?`, record.UserID).Scan(&state); err != nil {
		t.Fatalf("load user state: %v", err)
	}
	if state != "active" {
		t.Fatalf("expected user to be active, got %q", state)
	}

	passwords := auth.NewPasswords(db)
	ok, err := passwords.CheckPassword(ctx, record.UserID, "new-secret-password")
	if err != nil {
		t.Fatalf("CheckPassword(new): %v", err)
	}
	if !ok {
		t.Fatalf("expected new password to verify")
	}
	ok, err = passwords.CheckPassword(ctx, record.UserID, "old-secret-password")
	if err != nil {
		t.Fatalf("CheckPassword(old): %v", err)
	}
	if ok {
		t.Fatalf("expected old password to be replaced")
	}
}

func TestRecoverAdmin_MissingFailsWithoutCreateIfMissing(t *testing.T) {
	db := newBootstrapTestDB(t)
	ctx := context.Background()

	if err := SeedSystem(ctx, db); err != nil {
		t.Fatalf("SeedSystem: %v", err)
	}

	_, err := RecoverAdmin(ctx, db, RecoverAdminOptions{
		Identifier: "breakglass",
		Password:   "new-secret-password",
		Passwords:  auth.NewPasswords(db),
	})
	if err != ErrRecoveryTargetNotFound {
		t.Fatalf("RecoverAdmin error = %v, want %v", err, ErrRecoveryTargetNotFound)
	}
}

func TestRecoverAdmin_MissingCanCreateBreakGlassUser(t *testing.T) {
	db := newBootstrapTestDB(t)
	ctx := context.Background()

	if err := SeedSystem(ctx, db); err != nil {
		t.Fatalf("SeedSystem: %v", err)
	}

	record, err := RecoverAdmin(ctx, db, RecoverAdminOptions{
		Identifier:      "breakglass",
		Password:        "new-secret-password",
		CreateIfMissing: true,
		Passwords:       auth.NewPasswords(db),
	})
	if err != nil {
		t.Fatalf("RecoverAdmin: %v", err)
	}
	if !record.Created {
		t.Fatalf("expected break-glass admin to be created")
	}
	if record.Email != "breakglass@zitadel.local" {
		t.Fatalf("default email = %q, want %q", record.Email, "breakglass@zitadel.local")
	}

	var count int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM users WHERE identifier = 'breakglass'`).Scan(&count); err != nil {
		t.Fatalf("count break-glass users: %v", err)
	}
	if count != 1 {
		t.Fatalf("expected 1 break-glass user, got %d", count)
	}
}

func TestRecoverAdmin_CreatePathRespectsUniquenessFailures(t *testing.T) {
	db := newBootstrapTestDB(t)
	ctx := context.Background()

	if err := SeedSystem(ctx, db); err != nil {
		t.Fatalf("SeedSystem: %v", err)
	}
	if _, err := CreateAdmin(ctx, db, CreateAdminOptions{
		Username:  "admin",
		Email:     "admin@zitadel.local",
		Password:  "existing-secret-password",
		Passwords: auth.NewPasswords(db),
	}); err != nil {
		t.Fatalf("CreateAdmin(existing): %v", err)
	}

	_, err := RecoverAdmin(ctx, db, RecoverAdminOptions{
		Identifier:      "breakglass",
		Email:           "admin@zitadel.local",
		Password:        "new-secret-password",
		CreateIfMissing: true,
		Passwords:       auth.NewPasswords(db),
	})
	if err == nil {
		t.Fatalf("expected uniqueness error")
	}

	var count int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM users`).Scan(&count); err != nil {
		t.Fatalf("count users: %v", err)
	}
	if count != 1 {
		t.Fatalf("expected no partial recovery user to be created, got %d users", count)
	}
}
