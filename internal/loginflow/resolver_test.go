package loginflow

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/httputil"
)

func newResolverTestDB(t *testing.T) *database.DB {
	t.Helper()
	dir := t.TempDir()
	path := filepath.Join(dir, "resolver.db")
	db, err := database.Open("sqlite://" + path)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func insertTestFlow(t *testing.T, db *database.DB, instanceID, flowID, name string, priority int) {
	t.Helper()
	_, err := db.SQL().Exec(
		`INSERT INTO login_flows (instance_id, id, org_id, name, strategy, config, is_default, enabled, state, priority, audience, auth_methods, metadata, created_at, updated_at)
		 VALUES (?, ?, '1', ?, 'identifier_first', '{}', 1, 1, 'active', ?, '{}', '{}', '{}', datetime('now'), datetime('now'))`,
		instanceID, flowID, name, priority,
	)
	if err != nil {
		t.Fatalf("insert login flow %s: %v", flowID, err)
	}
}

func insertTestUser(t *testing.T, db *database.DB, instanceID, userID, identifier string) {
	t.Helper()
	_, err := db.SQL().Exec(
		`INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, '1', ?, ?, 'human', 'active', 'human_user_v1', '{}', datetime('now'), datetime('now'))`,
		userID, instanceID, identifier, identifier,
	)
	if err != nil {
		t.Fatalf("insert user %s: %v", userID, err)
	}
}

func TestResolverResolve_IsTenantScoped(t *testing.T) {
	db := newResolverTestDB(t)
	resolver := NewResolver(db)

	insertTestFlow(t, db, "tenant_a", "flow-tenant-a", "Tenant A Flow", 100)
	insertTestFlow(t, db, "tenant_b", "flow-tenant-b", "Tenant B Flow", 10)

	ctxA := httputil.WithInstanceID(context.Background(), "tenant_a")
	ctxB := httputil.WithInstanceID(context.Background(), "tenant_b")

	flowA, err := resolver.Resolve(ctxA, UserContext{OrgID: "1"})
	if err != nil {
		t.Fatalf("Resolve tenant_a: %v", err)
	}
	if flowA.ID != "flow-tenant-a" {
		t.Fatalf("tenant_a resolved %q, want flow-tenant-a", flowA.ID)
	}

	flowB, err := resolver.Resolve(ctxB, UserContext{OrgID: "1"})
	if err != nil {
		t.Fatalf("Resolve tenant_b: %v", err)
	}
	if flowB.ID != "flow-tenant-b" {
		t.Fatalf("tenant_b resolved %q, want flow-tenant-b", flowB.ID)
	}
}

func TestResolverTestAudience_UsesCurrentInstanceUsers(t *testing.T) {
	db := newResolverTestDB(t)
	resolver := NewResolver(db)

	insertTestFlow(t, db, "tenant_b", "flow-tenant-b", "Tenant B Audience Flow", 10)
	insertTestUser(t, db, "tenant_a", "user-tenant-a", "tenant-a@example.com")
	insertTestUser(t, db, "tenant_b", "user-tenant-b", "tenant-b@example.com")

	ctxB := httputil.WithInstanceID(context.Background(), "tenant_b")
	result, err := resolver.TestAudience(ctxB, "flow-tenant-b", 10)
	if err != nil {
		t.Fatalf("TestAudience tenant_b: %v", err)
	}

	if result.TotalUsers != 1 {
		t.Fatalf("TotalUsers = %d, want 1", result.TotalUsers)
	}
	if result.MatchingUsers != 1 {
		t.Fatalf("MatchingUsers = %d, want 1", result.MatchingUsers)
	}
	if len(result.Matches) != 1 {
		t.Fatalf("len(Matches) = %d, want 1", len(result.Matches))
	}
	if result.Matches[0].UserID != "user-tenant-b" {
		t.Fatalf("matched user = %q, want user-tenant-b", result.Matches[0].UserID)
	}
}

func TestResolverTestAudience_RejectsForeignInstanceFlow(t *testing.T) {
	db := newResolverTestDB(t)
	resolver := NewResolver(db)

	insertTestFlow(t, db, "tenant_a", "flow-tenant-a", "Tenant A Audience Flow", 10)

	ctxB := httputil.WithInstanceID(context.Background(), "tenant_b")
	if _, err := resolver.TestAudience(ctxB, "flow-tenant-a", 10); err == nil {
		t.Fatal("foreign-instance flow should not be visible to TestAudience")
	}
}
