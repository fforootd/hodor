package database

import (
	"path/filepath"
	"slices"
	"testing"
)

func TestSchemaCreation(t *testing.T) {
	db := newMigratedSQLiteDB(t, "schema-test.db")

	for _, table := range []string{"schemas", "settings", "users", "goose_db_version"} {
		if !sqliteObjectExists(t, db, "table", table) {
			t.Fatalf("expected table %q to exist after migrations", table)
		}
	}
}

func TestSQLiteMigrationRestoresTenantIndexes(t *testing.T) {
	db := newMigratedSQLiteDB(t, "schema-index-test.db")

	indexes := sqliteIndexNames(t, db)
	for _, name := range []string{
		"idx_users_instance",
		"idx_users_instance_state",
		"idx_users_instance_type",
		"idx_apps_instance",
		"idx_apps_instance_client",
		"idx_apps_instance_org",
		"idx_domains_instance",
		"idx_domains_instance_org",
		"idx_unique_fields_instance",
		"idx_unique_fields_instance_resource",
		"idx_unique_fields_instance_lookup",
	} {
		if !slices.Contains(indexes, name) {
			t.Fatalf("expected index %q after multi-tenant migration; indexes=%v", name, indexes)
		}
	}
}

func TestSQLiteOperationalTablesAllowPerInstanceRows(t *testing.T) {
	db := newMigratedSQLiteDB(t, "schema-operational-test.db")

	for _, stmt := range []string{
		`INSERT INTO jobs (instance_id, name, display_name, cron) VALUES ('tenant_a', 'session_gc', 'Session GC', '*/15 * * * *')`,
		`INSERT INTO jobs (instance_id, name, display_name, cron) VALUES ('tenant_b', 'session_gc', 'Session GC', '*/15 * * * *')`,
		`INSERT INTO cache (instance_id, namespace, key, data) VALUES ('tenant_a', 'catalog', 'templates', '{}')`,
		`INSERT INTO cache (instance_id, namespace, key, data) VALUES ('tenant_b', 'catalog', 'templates', '{}')`,
		`INSERT INTO consumer_cursors (instance_id, consumer_name, last_event_id) VALUES ('tenant_a', 'lake_writer', 'evt_a')`,
		`INSERT INTO consumer_cursors (instance_id, consumer_name, last_event_id) VALUES ('tenant_b', 'lake_writer', 'evt_b')`,
		`INSERT INTO retention_policies (instance_id, id, event_pattern, oltp_ttl, lake_ttl, priority) VALUES ('tenant_a', 'rp_default', '*', '14d', '365d', 0)`,
		`INSERT INTO retention_policies (instance_id, id, event_pattern, oltp_ttl, lake_ttl, priority) VALUES ('tenant_b', 'rp_default', '*', '14d', '365d', 0)`,
	} {
		if _, err := db.SQL().Exec(stmt); err != nil {
			t.Fatalf("exec %q: %v", stmt, err)
		}
	}
}

func newMigratedSQLiteDB(t *testing.T, filename string) *DB {
	t.Helper()

	db, err := Open("sqlite://" + filepath.Join(t.TempDir(), filename))
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	if err := Migrate(db); err != nil {
		t.Fatalf("Migrate: %v", err)
	}
	return db
}

func sqliteObjectExists(t *testing.T, db *DB, objectType, name string) bool {
	t.Helper()

	var found string
	err := db.SQL().QueryRow(`SELECT name FROM sqlite_master WHERE type = ? AND name = ?`, objectType, name).Scan(&found)
	return err == nil && found == name
}

func sqliteIndexNames(t *testing.T, db *DB) []string {
	t.Helper()

	rows, err := db.SQL().Query(`SELECT name FROM sqlite_master WHERE type = 'index' ORDER BY name`)
	if err != nil {
		t.Fatalf("query sqlite indexes: %v", err)
	}
	defer rows.Close()

	var indexes []string
	for rows.Next() {
		var name string
		if err := rows.Scan(&name); err != nil {
			t.Fatalf("scan sqlite index: %v", err)
		}
		indexes = append(indexes, name)
	}
	if err := rows.Err(); err != nil {
		t.Fatalf("iterate sqlite indexes: %v", err)
	}
	return indexes
}
