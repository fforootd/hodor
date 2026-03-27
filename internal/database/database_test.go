package database

import (
	"os"
	"path/filepath"
	"testing"
)

func TestOpenSQLite(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test.db")

	db, err := Open("sqlite://" + path)
	if err != nil {
		t.Fatalf("Open sqlite: %v", err)
	}
	defer db.Close()

	if db.Dialect() != "sqlite" {
		t.Errorf("dialect = %q, want sqlite", db.Dialect())
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Error("database file not created")
	}
}

func TestOpenDefault(t *testing.T) {
	// Empty string should default to SQLite.
	origDir, _ := os.Getwd()
	dir := t.TempDir()
	_ = os.Chdir(dir)
	defer func() { _ = os.Chdir(origDir) }()

	db, err := Open("")
	if err != nil {
		t.Fatalf("Open empty: %v", err)
	}
	defer db.Close()

	if db.Dialect() != "sqlite" {
		t.Errorf("dialect = %q, want sqlite", db.Dialect())
	}
}

func TestOpenUnsupported(t *testing.T) {
	_, err := Open("mysql://localhost/test")
	if err == nil {
		t.Fatal("expected error for unsupported scheme")
	}
}

func TestMigrate(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "migrate-test.db")

	db, err := Open("sqlite://" + path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer db.Close()

	if err := EnsureSchema(db); err != nil {
		t.Fatalf("EnsureSchema: %v", err)
	}

	// Verify a core table exists.
	var name string
	err = db.SQL().QueryRow("SELECT name FROM sqlite_master WHERE type='table' AND name='identities'").Scan(&name)
	if err != nil {
		t.Fatalf("identities table not found: %v", err)
	}

	// Verify capabilities table exists.
	err = db.SQL().QueryRow("SELECT name FROM sqlite_master WHERE type='table' AND name='identity_capabilities'").Scan(&name)
	if err != nil {
		t.Fatalf("identity_capabilities table not found: %v", err)
	}

	// Verify events table exists.
	err = db.SQL().QueryRow("SELECT name FROM sqlite_master WHERE type='table' AND name='events'").Scan(&name)
	if err != nil {
		t.Fatalf("events table not found: %v", err)
	}
}

func TestDialectHelpers(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "helpers-test.db")

	db, err := Open("sqlite://" + path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer db.Close()

	// SQLite JSON extract.
	got := db.JSONExtract("profile", "name")
	want := "json_extract(profile, '$.name')"
	if got != want {
		t.Errorf("JSONExtract = %q, want %q", got, want)
	}

	// SQLite placeholder.
	if p := db.Placeholder(1); p != "?" {
		t.Errorf("Placeholder(1) = %q, want ?", p)
	}

	// SQLite timestamp.
	if ts := db.TimestampNow(); ts != "datetime('now')" {
		t.Errorf("TimestampNow = %q, want datetime('now')", ts)
	}
}
