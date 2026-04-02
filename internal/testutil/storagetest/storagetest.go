package storagetest

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/httputil"
)

const DefaultInstanceID = "instance_test"

func Context(instanceID string) context.Context {
	if instanceID == "" {
		instanceID = DefaultInstanceID
	}
	return httputil.WithInstanceID(context.Background(), instanceID)
}

func OpenSQLite(t *testing.T) *database.DB {
	t.Helper()

	dir := t.TempDir()
	path := filepath.Join(dir, "storage.db")
	db, err := database.Open("sqlite://" + path)
	if err != nil {
		t.Fatalf("open sqlite db: %v", err)
	}
	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate sqlite db: %v", err)
	}
	t.Cleanup(func() {
		_ = db.Close()
	})
	return db
}

func OpenPostgres(t *testing.T) *database.DB {
	t.Helper()

	url := os.Getenv("ZITADEL_TEST_POSTGRES_URL")
	if url == "" {
		t.Skip("set ZITADEL_TEST_POSTGRES_URL to run Postgres storage tests")
	}

	db, err := database.Open(url)
	if err != nil {
		t.Fatalf("open postgres db: %v", err)
	}
	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate postgres db: %v", err)
	}
	t.Cleanup(func() {
		_ = db.Close()
	})
	return db
}

func RunBackends(t *testing.T, fn func(t *testing.T, db *database.DB, ctx context.Context)) {
	t.Helper()

	t.Run("sqlite", func(t *testing.T) {
		fn(t, OpenSQLite(t), Context(DefaultInstanceID))
	})

	if os.Getenv("ZITADEL_TEST_POSTGRES_URL") != "" {
		t.Run("postgres", func(t *testing.T) {
			fn(t, OpenPostgres(t), Context(DefaultInstanceID))
		})
	}
}
