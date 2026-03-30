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
	err = svc.OnOrgCreated(ctx, "org1", "admin", "inst_root")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}

	// admin should be org admin (via owner).
	allowed, err := svc.Check(ctx, "user:admin", "can_create_resource", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("org owner should have can_create_resource")
	}

	// Add bob as a member.
	err = svc.AddOrgMember(ctx, "bob", "org1")
	if err != nil {
		t.Fatalf("add member: %v", err)
	}

	// bob should be able to read resources.
	allowed, err = svc.Check(ctx, "user:bob", "can_read_resource", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("org member should have can_read_resource")
	}

	// bob should NOT be able to create resources (not admin).
	allowed, err = svc.Check(ctx, "user:bob", "can_create_resource", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if allowed {
		t.Error("org member should NOT have can_create_resource")
	}

	// Promote bob to admin.
	err = svc.AddOrgAdmin(ctx, "bob", "org1")
	if err != nil {
		t.Fatalf("add admin: %v", err)
	}

	// bob should now be able to create resources.
	allowed, err = svc.Check(ctx, "user:bob", "can_create_resource", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("org admin should have can_create_resource")
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

	err = svc.OnOrgCreated(ctx, "org1", "admin", "inst_root")
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

func TestCheck_ProjectPermissions(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Setup.
	err := svc.OnBootstrap(ctx, "admin")
	if err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	err = svc.OnOrgCreated(ctx, "org1", "admin", "inst_root")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}

	// Create project.
	err = svc.OnProjectCreated(ctx, "proj1", "admin", "org1")
	if err != nil {
		t.Fatalf("create project: %v", err)
	}

	// Admin (project owner) can manage project.
	allowed, err := svc.Check(ctx, "user:admin", "can_update", "project:proj1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("project owner should have can_update")
	}

	// Add alice as project member.
	err = svc.AddProjectMember(ctx, "alice", "proj1")
	if err != nil {
		t.Fatalf("add project member: %v", err)
	}

	// alice can read project.
	allowed, err = svc.Check(ctx, "user:alice", "can_read", "project:proj1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("project member should have can_read")
	}

	// alice cannot delete project.
	allowed, err = svc.Check(ctx, "user:alice", "can_delete", "project:proj1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if allowed {
		t.Error("project member should NOT have can_delete")
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
	allowed, err := svc.Check(ctx, "user:admin", "can_create_resource", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("instance owner should have can_create_resource on child org via parent inheritance")
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

func TestOnResourceCreatedAndDeleted(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Bootstrap.
	err := svc.OnBootstrap(ctx, "admin")
	if err != nil {
		t.Fatalf("bootstrap: %v", err)
	}
	err = svc.OnOrgCreated(ctx, "org1", "admin", "inst_root")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}

	// Create a resource (identity).
	err = svc.OnResourceCreated(ctx, "bob", "admin", "org1")
	if err != nil {
		t.Fatalf("resource created: %v", err)
	}

	// bob should now be an org member.
	allowed, err := svc.Check(ctx, "user:bob", "can_read_resource", "org:org1")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("newly created resource should have org membership")
	}

	// Delete resource.
	err = svc.OnResourceDeleted(ctx, "bob")
	if err != nil {
		t.Fatalf("delete resource: %v", err)
	}
}

func TestEnableModule_RBAC(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	// Enable RBAC module.
	err := svc.EnableModule(ctx, "rbac")
	if err != nil {
		t.Fatalf("enable rbac: %v", err)
	}

	if len(svc.EnabledModules()) != 1 {
		t.Errorf("expected 1 enabled module, got %d", len(svc.EnabledModules()))
	}

	// Setup org and create role tuple.
	err = svc.OnBootstrap(ctx, "admin")
	if err != nil {
		t.Fatalf("bootstrap: %v", err)
	}
	err = svc.OnOrgCreated(ctx, "org1", "admin", "inst_root")
	if err != nil {
		t.Fatalf("create org: %v", err)
	}

	// Write role tuples.
	err = svc.WriteTuples(ctx,
		[3]string{"org:org1", "org", "role:editor"},
		[3]string{"user:alice", "assignee", "role:editor"},
	)
	if err != nil {
		t.Fatalf("write role tuples: %v", err)
	}

	// alice should be able to use the role.
	allowed, err := svc.Check(ctx, "user:alice", "can_use", "role:editor")
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	if !allowed {
		t.Error("role assignee should have can_use")
	}

	// Idempotent enable.
	err = svc.EnableModule(ctx, "rbac")
	if err != nil {
		t.Fatalf("re-enable rbac: %v", err)
	}

	// Disable RBAC.
	err = svc.DisableModule(ctx, "rbac")
	if err != nil {
		t.Fatalf("disable rbac: %v", err)
	}

	if len(svc.EnabledModules()) != 0 {
		t.Errorf("expected 0 enabled modules, got %d", len(svc.EnabledModules()))
	}
}
