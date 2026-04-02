package database_test

import (
	"context"
	"errors"
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
	pgContainer, err := runPostgresContainer(ctx,
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
		t.Skipf("skipping postgres migration test: %v", err)
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
	err = db.SQL().QueryRow("SELECT tablename FROM pg_tables WHERE tablename = 'users' AND schemaname = 'public'").Scan(&tableName)
	if err != nil {
		t.Fatalf("users table not found in postgres: %v", err)
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
	if version < 1 {
		t.Errorf("expected goose version >= 1, got %d", version)
	}

	t.Logf("Postgres migration successful: goose version=%d", version)
}

func runPostgresContainer(ctx context.Context, imageName string, opts ...testcontainers.ContainerCustomizer) (_ *postgres.PostgresContainer, err error) {
	defer func() {
		if r := recover(); r != nil {
			err = fmt.Errorf("docker runtime unavailable: %v", r)
		}
	}()

	container, err := postgres.Run(ctx, imageName, opts...)
	if err != nil {
		if looksLikeDockerUnavailable(err) {
			return nil, fmt.Errorf("docker runtime unavailable: %w", err)
		}
		return nil, err
	}
	return container, nil
}

func looksLikeDockerUnavailable(err error) bool {
	if err == nil {
		return false
	}
	msg := err.Error()
	return errors.Is(err, context.DeadlineExceeded) ||
		msg == "checked path: $XDG_RUNTIME_DIR" ||
		containsAny(msg,
			"Cannot connect to the Docker daemon",
			"docker daemon",
			"checked path:",
			"no such host",
			"permission denied while trying to connect to the Docker daemon socket",
			"Cannot connect to the Docker Engine",
		)
}

func containsAny(value string, needles ...string) bool {
	for _, needle := range needles {
		if needle != "" && strings.Contains(value, needle) {
			return true
		}
	}
	return false
}
