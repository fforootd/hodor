package database_test

import (
	"context"
	"fmt"
	"os"
	"strings"
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

	// The wait strategy already waits for 2 occurrences of the ready log.

	// Connect to Postgres
	db, err := database.Open(connString)
	if err != nil {
		t.Fatalf("failed to connect to database: %v", err)
	}
	defer db.Close()

	if db.Dialect() != "postgres" {
		t.Fatalf("expected postgres dialect, got %s", db.Dialect())
	}

	// schema.sql is SQLite-only — EnsureSchema should return a clear error.
	err = database.EnsureSchema(db)
	if err == nil {
		t.Fatal("expected error running SQLite DDL against Postgres")
	}
	if !strings.Contains(err.Error(), "SQLite-only") {
		t.Fatalf("expected SQLite-only error, got: %v", err)
	}
	t.Logf("correctly rejected Postgres migration: %v", err)
}
