package database

import (
	"fmt"
	"path/filepath"
	"testing"
)

func TestSchemaCreation(t *testing.T) {
	dir := t.TempDir()
	dbPath := "sqlite://" + filepath.Join(dir, "test.db")
	db, err := Open(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()

	if err := Migrate(db); err != nil {
		t.Fatalf("Migrate: %v", err)
	}

	// Verify core tables exist via pragma.
	var count int
	if err = db.sql.QueryRow("SELECT count(*) FROM pragma_table_info('schemas')").Scan(&count); err != nil {
		t.Fatalf("query pragma: %v", err)
	}
	fmt.Printf("Columns in schemas: %d\n", count)

	// Verify settings table from migration 00002.
	if err = db.sql.QueryRow("SELECT count(*) FROM pragma_table_info('settings')").Scan(&count); err != nil {
		t.Fatalf("query settings pragma: %v", err)
	}
	if count == 0 {
		t.Fatal("settings table not created by migration")
	}
	fmt.Printf("Columns in settings: %d\n", count)

	// Verify goose version tracking table exists.
	var name string
	err = db.sql.QueryRow("SELECT name FROM sqlite_master WHERE type='table' AND name='goose_db_version'").Scan(&name)
	if err != nil {
		t.Fatalf("goose_db_version table not found: %v", err)
	}
}
