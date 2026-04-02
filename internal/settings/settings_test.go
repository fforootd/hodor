package settings

import (
	"context"
	"database/sql"
	"errors"
	"testing"

	_ "modernc.org/sqlite"
)

func testDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	// Create settings table matching the migration.
	_, err = db.Exec(`
		CREATE TABLE settings (
			id          TEXT PRIMARY KEY,
			instance_id TEXT NOT NULL DEFAULT 'default',
			type        TEXT NOT NULL,
			scope       TEXT NOT NULL DEFAULT 'instance',
			scope_id    TEXT NOT NULL DEFAULT '',
			data        TEXT NOT NULL DEFAULT '{}',
			created_at  TEXT NOT NULL DEFAULT (datetime('now')),
			updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
			UNIQUE(instance_id, type, scope, scope_id)
		)`)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func TestPutAndGet(t *testing.T) {
	db := testDB(t)
	ctx := context.Background()

	data := map[string]any{
		"requests_per_minute": float64(500),
		"burst":               float64(20),
	}

	if err := Put(ctx, db, "rate_limit", "instance", "", data); err != nil {
		t.Fatal(err)
	}

	got, err := Get(ctx, db, "rate_limit", "instance", "")
	if err != nil {
		t.Fatal(err)
	}

	if got["requests_per_minute"] != float64(500) {
		t.Errorf("rpm = %v, want 500", got["requests_per_minute"])
	}
	if got["burst"] != float64(20) {
		t.Errorf("burst = %v, want 20", got["burst"])
	}
}

func TestPutUpserts(t *testing.T) {
	db := testDB(t)
	ctx := context.Background()

	_ = Put(ctx, db, "rate_limit", "instance", "", map[string]any{"burst": float64(10)})
	_ = Put(ctx, db, "rate_limit", "instance", "", map[string]any{"burst": float64(99)})

	got, _ := Get(ctx, db, "rate_limit", "instance", "")
	if got["burst"] != float64(99) {
		t.Errorf("burst = %v, want 99 (upsert)", got["burst"])
	}
}

func TestGetMissing(t *testing.T) {
	db := testDB(t)
	ctx := context.Background()

	_, err := Get(ctx, db, "rate_limit", "org", "nonexistent")
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("expected ErrNotFound, got %v", err)
	}
}

func TestResolve_InstanceOnly(t *testing.T) {
	db := testDB(t)
	ctx := context.Background()

	_ = Put(ctx, db, "rate_limit", "instance", "", map[string]any{
		"requests_per_minute": float64(1000),
		"burst":               float64(50),
		"by_ip":               true,
	})

	got, err := Resolve(ctx, db, "rate_limit", "", "")
	if err != nil {
		t.Fatal(err)
	}
	if got["requests_per_minute"] != float64(1000) {
		t.Errorf("rpm = %v, want 1000", got["requests_per_minute"])
	}
}

func TestResolve_OrgOverride(t *testing.T) {
	db := testDB(t)
	ctx := context.Background()

	_ = Put(ctx, db, "rate_limit", "instance", "", map[string]any{
		"requests_per_minute": float64(1000),
		"burst":               float64(50),
		"by_ip":               true,
	})
	_ = Put(ctx, db, "rate_limit", "org", "org_1", map[string]any{
		"requests_per_minute": float64(200),
	})

	got, err := Resolve(ctx, db, "rate_limit", "org_1", "")
	if err != nil {
		t.Fatal(err)
	}

	// rpm should be overridden by org.
	if got["requests_per_minute"] != float64(200) {
		t.Errorf("rpm = %v, want 200 (org override)", got["requests_per_minute"])
	}
	// burst should be inherited from instance.
	if got["burst"] != float64(50) {
		t.Errorf("burst = %v, want 50 (inherited)", got["burst"])
	}
	// by_ip should be inherited from instance.
	if got["by_ip"] != true {
		t.Errorf("by_ip = %v, want true (inherited)", got["by_ip"])
	}
}

func TestResolve_AppOverride(t *testing.T) {
	db := testDB(t)
	ctx := context.Background()

	_ = Put(ctx, db, "rate_limit", "instance", "", map[string]any{
		"requests_per_minute": float64(1000),
		"burst":               float64(50),
	})
	_ = Put(ctx, db, "rate_limit", "org", "org_1", map[string]any{
		"requests_per_minute": float64(200),
	})
	_ = Put(ctx, db, "rate_limit", "app", "app_1", map[string]any{
		"burst": float64(5),
	})

	got, err := Resolve(ctx, db, "rate_limit", "org_1", "app_1")
	if err != nil {
		t.Fatal(err)
	}

	// rpm from org (200), burst from app (5).
	if got["requests_per_minute"] != float64(200) {
		t.Errorf("rpm = %v, want 200", got["requests_per_minute"])
	}
	if got["burst"] != float64(5) {
		t.Errorf("burst = %v, want 5 (app override)", got["burst"])
	}
}

func TestDelete(t *testing.T) {
	db := testDB(t)
	ctx := context.Background()

	_ = Put(ctx, db, "rate_limit", "org", "org_1", map[string]any{"burst": float64(10)})
	_ = Delete(ctx, db, "rate_limit", "org", "org_1")

	_, err := Get(ctx, db, "rate_limit", "org", "org_1")
	if !errors.Is(err, ErrNotFound) {
		t.Errorf("expected ErrNotFound after delete, got %v", err)
	}
}

func TestDeepMerge_NestedMaps(t *testing.T) {
	dst := map[string]any{
		"top": map[string]any{
			"a": 1,
			"b": 2,
		},
	}
	src := map[string]any{
		"top": map[string]any{
			"b": 99,
			"c": 3,
		},
	}

	deepMerge(dst, src)

	inner := dst["top"].(map[string]any)
	if inner["a"] != 1 {
		t.Errorf("a = %v, want 1 (preserved)", inner["a"])
	}
	if inner["b"] != 99 {
		t.Errorf("b = %v, want 99 (overridden)", inner["b"])
	}
	if inner["c"] != 3 {
		t.Errorf("c = %v, want 3 (added)", inner["c"])
	}
}
