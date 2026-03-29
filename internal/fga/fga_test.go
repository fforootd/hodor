package fga

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite" // Pure Go SQLite driver
)

// newTestService creates an FGA service backed by an in-memory SQLite database.
func newTestService(t *testing.T) *Service {
	t.Helper()
	db, err := sql.Open("sqlite", filepath.Join(t.TempDir(), "fga_test.db"))
	if err != nil {
		t.Fatalf("open test db: %v", err)
	}
	t.Cleanup(func() { db.Close() })

	ctx := context.Background()
	svc, err := New(ctx, db, "sqlite3")
	if err != nil {
		t.Fatalf("create FGA service: %v", err)
	}
	return svc
}

func TestNewService(t *testing.T) {
	svc := newTestService(t)
	if svc.SystemStoreID() == "" {
		t.Fatal("expected system store ID to be set")
	}
}

func TestCheck_InstanceOwner(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Write: user:admin is owner of instance:default.
	err := svc.WriteTuple(ctx, "user:admin", "owner", "instance:default")
	if err != nil {
		t.Fatalf("write tuple: %v", err)
	}

	// owner should have admin rights.
	allowed, err := svc.Check(ctx, "user:admin", "can_manage_orgs", "instance:default")
	if err != nil {
		t.Fatalf("check failed: %v", err)
	}
	if !allowed {
		t.Error("instance owner should have can_manage_orgs")
	}

	// owner should have viewer rights.
	allowed, err = svc.Check(ctx, "user:admin", "can_view_audit", "instance:default")
	if err != nil {
		t.Fatalf("check failed: %v", err)
	}
	if !allowed {
		t.Error("instance owner should have can_view_audit")
	}

	// random user should NOT have admin rights.
	allowed, err = svc.Check(ctx, "user:random", "can_manage_orgs", "instance:default")
	if err != nil {
		t.Fatalf("check failed: %v", err)
	}
	if allowed {
		t.Error("random user should NOT have can_manage_orgs")
	}
}

func TestCheck_OrgHierarchy(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Setup: admin → instance owner.
	err := svc.OnBootstrap(ctx, "admin")
	if err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	// Explicitly create org (no default org at bootstrap).
	err = svc.OnOrgCreated(ctx, "org1", "admin")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}

	// admin should be org admin (via owner).
	allowed, err := svc.Check(ctx, "user:admin", "can_create_entity", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("org owner should have can_create_entity")
	}

	// Add bob as a member.
	err = svc.AddOrgMember(ctx, "bob", "org1")
	if err != nil {
		t.Fatalf("add member: %v", err)
	}

	// bob should be able to read entities.
	allowed, err = svc.Check(ctx, "user:bob", "can_read_entity", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("org member should have can_read_entity")
	}

	// bob should NOT be able to create entities (not admin).
	allowed, err = svc.Check(ctx, "user:bob", "can_create_entity", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if allowed {
		t.Error("org member should NOT have can_create_entity")
	}

	// Promote bob to admin.
	err = svc.AddOrgAdmin(ctx, "bob", "org1")
	if err != nil {
		t.Fatalf("add admin: %v", err)
	}

	// bob should now be able to create entities.
	allowed, err = svc.Check(ctx, "user:bob", "can_create_entity", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("org admin should have can_create_entity")
	}
}

func TestCheck_EntityPermissions(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Setup: admin owns instance, creates org + entity.
	err := svc.OnBootstrap(ctx, "admin")
	if err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	err = svc.OnOrgCreated(ctx, "org1", "admin")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}

	err = svc.OnResourceCreated(ctx, "entity1", "admin", "org1")
	if err != nil {
		t.Fatalf("entity created: %v", err)
	}

	// admin (entity owner) can read, update, delete.
	for _, perm := range []string{"can_read", "can_update", "can_delete"} {
		allowed, err := svc.Check(ctx, "user:admin", perm, "entity:entity1")
		if err != nil {
			t.Fatalf("check %s: %v", perm, err)
		}
		if !allowed {
			t.Errorf("entity owner should have %s", perm)
		}
	}

	// Add viewer to the org.
	err = svc.AddOrgMember(ctx, "viewer", "org1")
	if err != nil {
		t.Fatalf("add member: %v", err)
	}

	// viewer (org member) can read via entity#viewer ← org#member.
	allowed, err := svc.Check(ctx, "user:viewer", "can_read", "entity:entity1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("org member should have can_read on entity")
	}

	// viewer should NOT be able to update.
	allowed, err = svc.Check(ctx, "user:viewer", "can_update", "entity:entity1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if allowed {
		t.Error("org member should NOT have can_update on entity")
	}
}

func TestCheck_GroupMembership(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Setup org.
	err := svc.OnBootstrap(ctx, "admin")
	if err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	err = svc.OnOrgCreated(ctx, "org1", "admin")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}

	// Create group.
	err = svc.OnGroupCreated(ctx, "group1", "admin", "org1")
	if err != nil {
		t.Fatalf("group created: %v", err)
	}

	// Admin can manage group.
	allowed, err := svc.Check(ctx, "user:admin", "can_manage_members", "group:group1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("group owner should have can_manage_members")
	}

	// Add alice as group member.
	err = svc.AddGroupMember(ctx, "alice", "group1")
	if err != nil {
		t.Fatalf("add group member: %v", err)
	}

	// alice can read group.
	allowed, err = svc.Check(ctx, "user:alice", "can_read", "group:group1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("group member should have can_read")
	}

	// alice cannot manage members.
	allowed, err = svc.Check(ctx, "user:alice", "can_manage_members", "group:group1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if allowed {
		t.Error("group member should NOT have can_manage_members")
	}
}

func TestCheck_InstanceAdminInheritance(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Setup: admin owns instance, org1 parent is instance.
	err := svc.WriteTuples(ctx,
		[3]string{"user:admin", "owner", "instance:default"},
		[3]string{"instance:default", "parent", "org:org1"},
	)
	if err != nil {
		t.Fatalf("write tuples: %v", err)
	}

	// Instance admin should inherit org admin via parent relation.
	allowed, err := svc.Check(ctx, "user:admin", "can_create_entity", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("instance owner should have can_create_entity on child org via parent inheritance")
	}
}

func TestCheck_ReadTuples(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	err := svc.WriteTuples(ctx,
		[3]string{"user:admin", "owner", "instance:default"},
		[3]string{"user:bob", "member", "org:org1"},
	)
	if err != nil {
		t.Fatalf("write tuples: %v", err)
	}

	// Read all tuples for org:org1.
	tuples, err := svc.ReadTuples(ctx, "", "", "org:org1")
	if err != nil {
		t.Fatalf("read tuples: %v", err)
	}
	if len(tuples) != 1 {
		t.Errorf("expected 1 tuple for org:org1, got %d", len(tuples))
	}

	// Read all tuples for instance:default.
	tuples, err = svc.ReadTuples(ctx, "", "", "instance:default")
	if err != nil {
		t.Fatalf("read tuples: %v", err)
	}
	if len(tuples) != 1 {
		t.Errorf("expected 1 tuple for instance:default, got %d", len(tuples))
	}
	if tuples[0]["user"] != "user:admin" {
		t.Errorf("expected user:admin, got %s", tuples[0]["user"])
	}
}

func TestDeleteTuples(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	err := svc.WriteTuple(ctx, "user:bob", "member", "org:org1")
	if err != nil {
		t.Fatalf("write tuple: %v", err)
	}

	// Verify present.
	allowed, err := svc.Check(ctx, "user:bob", "viewer", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("member should be viewer")
	}

	// Remove.
	err = svc.RemoveOrgMember(ctx, "bob", "org1")
	if err != nil {
		t.Fatalf("delete tuple: %v", err)
	}

	// Should no longer be a viewer.
	allowed, err = svc.Check(ctx, "user:bob", "viewer", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if allowed {
		t.Error("removed member should NOT be viewer")
	}
}

func TestOnResourceDeleted(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Create entity.
	err := svc.OnBootstrap(ctx, "admin")
	if err != nil {
		t.Fatalf("bootstrap: %v", err)
	}
	err = svc.OnOrgCreated(ctx, "org1", "admin")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}
	err = svc.OnResourceCreated(ctx, "ent1", "admin", "org1")
	if err != nil {
		t.Fatalf("entity created: %v", err)
	}

	// Verify tuples exist.
	tuples, err := svc.ReadTuples(ctx, "", "", "entity:ent1")
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if len(tuples) == 0 {
		t.Fatal("expected tuples for entity:ent1")
	}

	// Delete entity.
	err = svc.OnResourceDeleted(ctx, "ent1")
	if err != nil {
		t.Fatalf("delete entity: %v", err)
	}

	// Verify tuples removed.
	tuples, err = svc.ReadTuples(ctx, "", "", "entity:ent1")
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if len(tuples) != 0 {
		t.Errorf("expected 0 tuples after delete, got %d", len(tuples))
	}
}
