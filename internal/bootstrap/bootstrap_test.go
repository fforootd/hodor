package bootstrap

import (
	"context"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
)

func TestEnsureAdmin_Idempotent(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + dir + "/test.db")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

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
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + dir + "/test.db")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

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
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + dir + "/test.db")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

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
