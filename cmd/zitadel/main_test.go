package main

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
)

func newNonTTYFile(t *testing.T, content string) *os.File {
	t.Helper()

	f, err := os.CreateTemp(t.TempDir(), "stdin-*")
	if err != nil {
		t.Fatalf("CreateTemp: %v", err)
	}
	if _, err := f.WriteString(content); err != nil {
		t.Fatalf("WriteString: %v", err)
	}
	if _, err := f.Seek(0, io.SeekStart); err != nil {
		t.Fatalf("Seek: %v", err)
	}
	t.Cleanup(func() { f.Close() })
	return f
}

func TestBootstrapAdminRejectsBothPasswordFlags(t *testing.T) {
	stdin := newNonTTYFile(t, "")
	cmd := newRootCmdWithIO(cliIO{
		in:        stdin,
		out:       &bytes.Buffer{},
		errOut:    &bytes.Buffer{},
		stdinFile: stdin,
	})
	cmd.SetArgs([]string{"bootstrap", "admin", "--password", "secret123", "--password-stdin"})

	err := cmd.Execute()
	if err == nil || !strings.Contains(err.Error(), "exactly one of --password or --password-stdin") {
		t.Fatalf("Execute() error = %v, want exactly-one validation", err)
	}
}

func TestBootstrapAdminRejectsMissingPasswordOnNonTTY(t *testing.T) {
	stdin := newNonTTYFile(t, "")
	cmd := newRootCmdWithIO(cliIO{
		in:        stdin,
		out:       &bytes.Buffer{},
		errOut:    &bytes.Buffer{},
		stdinFile: stdin,
	})
	cmd.SetArgs([]string{"bootstrap", "admin"})

	err := cmd.Execute()
	if err == nil || !strings.Contains(err.Error(), "password required") {
		t.Fatalf("Execute() error = %v, want password-required validation", err)
	}
}

func TestRecoverAdminRequiresCreateIfMissingToCreateNewUser(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "recover.db")
	db, err := database.Open("sqlite://" + dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	if err := db.Close(); err != nil {
		t.Fatalf("close db: %v", err)
	}

	t.Setenv("ZITADEL_DATABASE_URL", "sqlite://"+dbPath)

	stdin := newNonTTYFile(t, "")
	cmd := newRootCmdWithIO(cliIO{
		in:        stdin,
		out:       &bytes.Buffer{},
		errOut:    &bytes.Buffer{},
		stdinFile: stdin,
	})
	cmd.SetArgs([]string{"recover", "admin", "--identifier", "missing-admin", "--password", "secret123"})

	err = cmd.Execute()
	if err == nil || !strings.Contains(err.Error(), "--create-if-missing") {
		t.Fatalf("Execute() error = %v, want create-if-missing hint", err)
	}
}

func TestRedactDatabaseURL(t *testing.T) {
	got := redactDatabaseURL("libsql://db.turso.io?authToken=secret")
	if strings.Contains(got, "secret") {
		t.Fatalf("expected auth token to be redacted, got %q", got)
	}

	got = redactDatabaseURL("postgres://user:secret@localhost:5432/zitadel?sslmode=disable")
	if strings.Contains(got, "secret") {
		t.Fatalf("expected password to be redacted, got %q", got)
	}
}
