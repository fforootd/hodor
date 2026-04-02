package crypto

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/httputil"
)

func newSecretStoreTestDB(t *testing.T) *database.DB {
	t.Helper()

	db, err := database.Open("sqlite://" + filepath.Join(t.TempDir(), "secrets.db"))
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate db: %v", err)
	}
	return db
}

func newSecretStoreForTest(t *testing.T, db *database.DB) *SecretStore {
	t.Helper()

	box, err := NewSecretBox("", nil)
	if err != nil {
		t.Fatalf("new secret box: %v", err)
	}
	return NewSecretStore(db, box)
}

func TestSecretStore_IsTenantScopedAndUpsertsWithinInstance(t *testing.T) {
	db := newSecretStoreTestDB(t)
	store := newSecretStoreForTest(t, db)

	ctxA := httputil.WithInstanceID(context.Background(), "tenant_a")
	ctxB := httputil.WithInstanceID(context.Background(), "tenant_b")

	if err := store.Put(ctxA, "sig-key", "oidc_signing", []byte("first")); err != nil {
		t.Fatalf("Put tenant_a: %v", err)
	}
	if err := store.Put(ctxA, "sig-key", "oidc_signing", []byte("second")); err != nil {
		t.Fatalf("Put tenant_a update: %v", err)
	}

	got, err := store.Get(ctxA, "sig-key")
	if err != nil {
		t.Fatalf("Get tenant_a: %v", err)
	}
	if string(got) != "second" {
		t.Fatalf("Get tenant_a = %q, want second", string(got))
	}

	if _, err := store.Get(ctxB, "sig-key"); err == nil {
		t.Fatal("Get tenant_b should not resolve tenant_a secret")
	}
}

func TestSecretStore_RejectsCrossTenantIDReuse(t *testing.T) {
	db := newSecretStoreTestDB(t)
	store := newSecretStoreForTest(t, db)

	ctxA := httputil.WithInstanceID(context.Background(), "tenant_a")
	ctxB := httputil.WithInstanceID(context.Background(), "tenant_b")

	if err := store.Put(ctxA, "shared-id", "oidc_signing", []byte("tenant-a")); err != nil {
		t.Fatalf("Put tenant_a: %v", err)
	}
	if err := store.Put(ctxB, "shared-id", "oidc_signing", []byte("tenant-b")); err == nil {
		t.Fatal("cross-tenant Put with same id should fail")
	}
}
