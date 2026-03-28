package database_test

import (
	"context"
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/testcontainers/testcontainers-go"
	"github.com/testcontainers/testcontainers-go/modules/postgres"
	"github.com/testcontainers/testcontainers-go/wait"
	"github.com/zitadel/zitadel/internal/database"
)

func TestPostgresMigrations(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping testcontainers test in short mode")
	}

	ctx := context.Background()

	pgVersion := os.Getenv("TEST_POSTGRES_VERSION")
	if pgVersion == "" {
		pgVersion = "16"
	}
	imageName := fmt.Sprintf("postgres:%s-alpine", pgVersion)

	// Spin up generic Postgres container.
	pgContainer, err := postgres.Run(ctx,
		imageName,
		postgres.WithDatabase("zitadel_test"),
		postgres.WithUsername("zitadel"),
		postgres.WithPassword("password"),
		testcontainers.WithWaitStrategy(
			wait.ForLog("database system is ready to accept connections").
				WithOccurrence(2).
				WithStartupTimeout(30*time.Second)),
	)
	if err != nil {
		t.Fatalf("failed to start postgres container: %s", err)
	}

	// Clean up container when test finishes.
	t.Cleanup(func() {
		if err := pgContainer.Terminate(ctx); err != nil {
			t.Fatalf("failed to terminate container: %s", err)
		}
	})

	connString, err := pgContainer.ConnectionString(ctx, "sslmode=disable")
	if err != nil {
		t.Fatalf("failed to get connection string: %s", err)
	}

	// Connect to Postgres
	db, err := database.Open(connString)
	if err != nil {
		t.Fatalf("failed to connect to database: %v", err)
	}
	defer db.Close()

	if db.Dialect() != "postgres" {
		t.Fatalf("expected postgres dialect, got %s", db.Dialect())
	}

	// With Goose migrations, Postgres should now work.
	err = database.Migrate(db)
	if err != nil {
		t.Fatalf("Migrate (postgres) failed: %v", err)
	}

	// Verify core tables exist.
	var tableName string
	err = db.SQL().QueryRow("SELECT tablename FROM pg_tables WHERE tablename = 'entities' AND schemaname = 'public'").Scan(&tableName)
	if err != nil {
		t.Fatalf("entities table not found in postgres: %v", err)
	}

	// Verify settings table from migration 00002.
	err = db.SQL().QueryRow("SELECT tablename FROM pg_tables WHERE tablename = 'settings' AND schemaname = 'public'").Scan(&tableName)
	if err != nil {
		t.Fatalf("settings table not found in postgres: %v", err)
	}

	// Verify goose version tracking.
	var version int64
	err = db.SQL().QueryRow("SELECT version_id FROM goose_db_version ORDER BY id DESC LIMIT 1").Scan(&version)
	if err != nil {
		t.Fatalf("goose version not found: %v", err)
	}
	if version < 2 {
		t.Errorf("expected goose version >= 2, got %d", version)
	}

	t.Logf("Postgres migration successful: goose version=%d", version)
}
