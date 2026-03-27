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

	if err := database.EnsureSchema(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	ctx := context.Background()

	// First call should bootstrap the admin.
	if err := EnsureAdmin(ctx, db); err != nil {
		t.Fatalf("first EnsureAdmin: %v", err)
	}

	var count1 int
	db.SQL().QueryRow("SELECT COUNT(*) FROM entities").Scan(&count1)
	if count1 != 2 {
		t.Fatalf("expected 2 identities after first bootstrap (admin + console), got %d", count1)
	}

	// Second call should be a no-op (idempotent).
	if err := EnsureAdmin(ctx, db); err != nil {
		t.Fatalf("second EnsureAdmin: %v", err)
	}

	var count2 int
	db.SQL().QueryRow("SELECT COUNT(*) FROM entities").Scan(&count2)
	if count2 != 2 {
		t.Fatalf("expected 2 identities after second bootstrap (idempotent), got %d", count2)
	}
}

func TestEnsureAdmin_SeedsSchemas(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + dir + "/test.db")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.EnsureSchema(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	ctx := context.Background()
	if err := EnsureAdmin(ctx, db); err != nil {
		t.Fatalf("EnsureAdmin: %v", err)
	}

	// Verify built-in schemas were seeded.
	var schemaCount int
	db.SQL().QueryRow("SELECT COUNT(*) FROM schemas WHERE org_id = 0").Scan(&schemaCount)
	if schemaCount != len(builtinSchemas) {
		t.Errorf("expected %d built-in schemas, got %d", len(builtinSchemas), schemaCount)
	}
}

func TestEnsureAdmin_AdminHasCapabilities(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + dir + "/test.db")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.EnsureSchema(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	ctx := context.Background()
	if err := EnsureAdmin(ctx, db); err != nil {
		t.Fatalf("EnsureAdmin: %v", err)
	}

	// Admin should have "admin" and "password" capabilities.
	var capCount int
	db.SQL().QueryRow(`SELECT COUNT(*) FROM entity_capabilities
		WHERE entity_id = (SELECT id FROM entities WHERE identifier = 'admin')`).Scan(&capCount)
	if capCount != 2 {
		t.Errorf("expected 2 capabilities (admin, password), got %d", capCount)
	}
}
