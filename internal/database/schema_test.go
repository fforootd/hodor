package database

import (
"fmt"
"strings"
"testing"
)

func TestSchemaCreation(t *testing.T) {
	dir := t.TempDir()
	dbPath := "sqlite://" + dir + "/test.db"
	db, err := Open(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	stmts := strings.Split(schemaDDL, ";")
	for i, stmt := range stmts {
		stmt = strings.TrimSpace(stmt)
		if stmt == "" { continue }
		_, err := db.sql.Exec(stmt)
		if err != nil {
			t.Fatalf("Failed at stmt %d: %q\nErr: %v", i, stmt, err)
		}
	}
	var count int
	err = db.sql.QueryRow("SELECT count(*) FROM pragma_table_info('schemas')").Scan(&count)
	fmt.Printf("Columns in schemas: %d\n", count)
}
