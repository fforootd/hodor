package api_test

import (
	"context"
	"testing"

	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/testutil"
)

// TestCrossTenantIsolation verifies that data created under one instance_id
// is completely invisible to requests scoped to a different instance_id.
// This exercises the instance_id WHERE clause on every tenant-scoped table.
func TestCrossTenantIsolation(t *testing.T) {
	srv := testutil.NewTestServerMultiTenant(t)

	// ── Bootstrap per-tenant admin users ────────────────────────────────────
	// We insert admin users directly into the DB with the correct instance_id,
	// then create sessions (also instance-scoped) so we can authenticate.

	tenantAAdminID := bootstrapTenantAdmin(t, srv, "tenant_a", "admin_a", "Admin A")
	tenantAToken := createTenantSession(t, srv, "tenant_a", tenantAAdminID)

	tenantBAdminID := bootstrapTenantAdmin(t, srv, "tenant_b", "admin_b", "Admin B")
	tenantBToken := createTenantSession(t, srv, "tenant_b", tenantBAdminID)

	// Grant FGA admin access to both tenant admins.
	// Each tenant has its own FGA store, so we must use a context with the
	// correct instance_id so StoreForInstance resolves the right store.
	if svc := api.FGAService; svc != nil {
		ctxA := httputil.WithInstanceID(context.Background(), "tenant_a")
		if err := svc.EnsureInstanceOwner(ctxA, tenantAAdminID); err != nil {
			t.Fatalf("FGA bootstrap tenant_a: %v", err)
		}
		ctxB := httputil.WithInstanceID(context.Background(), "tenant_b")
		if err := svc.EnsureInstanceOwner(ctxB, tenantBAdminID); err != nil {
			t.Fatalf("FGA bootstrap tenant_b: %v", err)
		}
	}

	// ── Step 1: Create data in tenant_a ────────────────────────────────────

	// Create an org.
	orgCode, orgBody := srv.RequestWithHeaders("POST", "/v1/orgs", map[string]string{
		"Authorization": "Bearer " + tenantAToken,
		"X-Instance-Id": "tenant_a",
	}, map[string]any{
		"data": map[string]any{
			"display_name": "Tenant A Org",
		},
	})
	if orgCode != 200 && orgCode != 201 {
		t.Fatalf("create org in tenant_a: expected 200/201, got %d: %v", orgCode, orgBody)
	}

	// Create a user.
	userCode, userBody := srv.RequestWithHeaders("POST", "/v1/users", map[string]string{
		"Authorization": "Bearer " + tenantAToken,
		"X-Instance-Id": "tenant_a",
	}, map[string]any{
		"identifier":   "alice@tenant-a.com",
		"display_name": "Alice",
		"schema_id":    "human_user_v1",
		"state":        "active",
	})
	if userCode != 200 && userCode != 201 {
		t.Fatalf("create user in tenant_a: expected 200/201, got %d: %v", userCode, userBody)
	}

	// ── Step 2: Query tenant_b — should see NO tenant_a data ───────────────

	t.Run("tenant_b sees no orgs from tenant_a", func(t *testing.T) {
		code, body := srv.RequestWithHeaders("GET", "/v1/orgs", map[string]string{
			"Authorization": "Bearer " + tenantBToken,
			"X-Instance-Id": "tenant_b",
		}, nil)
		if code != 200 {
			t.Fatalf("list orgs in tenant_b: expected 200, got %d: %v", code, body)
		}
		orgs, _ := body["items"].([]any)
		if len(orgs) != 0 {
			t.Fatalf("tenant_b should see 0 orgs, got %d: %v", len(orgs), orgs)
		}
	})

	t.Run("tenant_b sees only its own admin user", func(t *testing.T) {
		code, body := srv.RequestWithHeaders("GET", "/v1/users", map[string]string{
			"Authorization": "Bearer " + tenantBToken,
			"X-Instance-Id": "tenant_b",
		}, nil)
		if code != 200 {
			t.Fatalf("list users in tenant_b: expected 200, got %d: %v", code, body)
		}
		users, _ := body["items"].([]any)
		// tenant_b should only see the admin_b we bootstrapped.
		if len(users) != 1 {
			t.Fatalf("tenant_b should see 1 user (admin_b), got %d: %v", len(users), users)
		}
		first, _ := users[0].(map[string]any)
		if first["identifier"] != "admin_b" {
			t.Fatalf("tenant_b user should be admin_b, got %v", first["identifier"])
		}
	})

	// ── Step 3: Query tenant_a — should see its own data ──────────────────

	t.Run("tenant_a sees its org", func(t *testing.T) {
		code, body := srv.RequestWithHeaders("GET", "/v1/orgs", map[string]string{
			"Authorization": "Bearer " + tenantAToken,
			"X-Instance-Id": "tenant_a",
		}, nil)
		if code != 200 {
			t.Fatalf("list orgs in tenant_a: expected 200, got %d: %v", code, body)
		}
		orgs, _ := body["items"].([]any)
		if len(orgs) != 1 {
			t.Fatalf("tenant_a should see 1 org, got %d: %v", len(orgs), orgs)
		}
	})

	t.Run("tenant_a sees its users", func(t *testing.T) {
		code, body := srv.RequestWithHeaders("GET", "/v1/users", map[string]string{
			"Authorization": "Bearer " + tenantAToken,
			"X-Instance-Id": "tenant_a",
		}, nil)
		if code != 200 {
			t.Fatalf("list users in tenant_a: expected 200, got %d: %v", code, body)
		}
		users, _ := body["items"].([]any)
		// tenant_a should see admin_a + alice = 2 users.
		if len(users) != 2 {
			t.Fatalf("tenant_a should see 2 users, got %d: %v", len(users), users)
		}
	})

	// ── Step 4: Default instance does not see tenant_a data ───────────────

	t.Run("default instance does not see tenant_a data", func(t *testing.T) {
		defaultToken := srv.LoginAdmin()

		code, body := srv.RequestWithHeaders("GET", "/v1/users", map[string]string{
			"Authorization": "Bearer " + defaultToken,
			// No X-Instance-Id → defaults to "default"
		}, nil)
		if code != 200 {
			t.Fatalf("list users in default: expected 200, got %d: %v", code, body)
		}
		users, _ := body["items"].([]any)
		if len(users) == 0 {
			t.Fatal("default instance should have at least the bootstrap admin")
		}
		for _, raw := range users {
			u, _ := raw.(map[string]any)
			if u["identifier"] == "alice@tenant-a.com" {
				t.Fatal("default instance should NOT see tenant_a's alice user")
			}
			if u["identifier"] == "admin_a" {
				t.Fatal("default instance should NOT see tenant_a's admin_a user")
			}
		}
	})
}

// TestCrossTenantTokenIsolation verifies that a session token created in one
// tenant cannot be used to authenticate in a different tenant.
func TestCrossTenantTokenIsolation(t *testing.T) {
	srv := testutil.NewTestServerMultiTenant(t)

	// Bootstrap tenant_a with an admin user and session token.
	tenantAAdminID := bootstrapTenantAdmin(t, srv, "tenant_a", "admin_tok_a", "Admin Token A")
	tenantAToken := createTenantSession(t, srv, "tenant_a", tenantAAdminID)

	// Grant FGA admin access in tenant_a's store.
	if svc := api.FGAService; svc != nil {
		ctxA := httputil.WithInstanceID(context.Background(), "tenant_a")
		if err := svc.EnsureInstanceOwner(ctxA, tenantAAdminID); err != nil {
			t.Fatalf("FGA bootstrap tenant_a: %v", err)
		}
	}

	t.Run("token works in its own tenant", func(t *testing.T) {
		code, body := srv.RequestWithHeaders("GET", "/v1/users", map[string]string{
			"Authorization": "Bearer " + tenantAToken,
			"X-Instance-Id": "tenant_a",
		}, nil)
		if code != 200 {
			t.Fatalf("tenant_a token in tenant_a: expected 200, got %d: %v", code, body)
		}
	})

	t.Run("token fails in different tenant", func(t *testing.T) {
		code, body := srv.RequestWithHeaders("GET", "/v1/users", map[string]string{
			"Authorization": "Bearer " + tenantAToken,
			"X-Instance-Id": "tenant_b",
		}, nil)
		if code != 401 {
			t.Fatalf("tenant_a token in tenant_b: expected 401, got %d: %v", code, body)
		}
	})

	t.Run("token fails in default instance", func(t *testing.T) {
		code, body := srv.RequestWithHeaders("GET", "/v1/users", map[string]string{
			"Authorization": "Bearer " + tenantAToken,
			// No X-Instance-Id → defaults to "default"
		}, nil)
		if code != 401 {
			t.Fatalf("tenant_a token in default instance: expected 401, got %d: %v", code, body)
		}
	})
}

// ── Helpers ────────────────────────────────────────────────────────────────

// bootstrapTenantAdmin inserts an admin user directly into the DB for the
// given instance_id. This bypasses the API since we need the user to exist
// before we can authenticate. Returns the user ID.
func bootstrapTenantAdmin(t *testing.T, srv *testutil.TestServer, instanceID, identifier, displayName string) string {
	t.Helper()

	userID := identifier + "-id" // deterministic for readability
	now := "2025-01-01 00:00:00"

	_, err := srv.DB.SQL().Exec(
		`INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, '', ?, ?, 'human', 'active', 'human_user_v1', '{"is_admin":true}', ?, ?)`,
		userID, instanceID, identifier, displayName, now, now)
	if err != nil {
		t.Fatalf("bootstrap tenant admin %s: %v", instanceID, err)
	}

	return userID
}

// createTenantSession inserts a session+token directly into the DB with the
// correct instance_id. Returns the raw token string.
func createTenantSession(t *testing.T, srv *testutil.TestServer, instanceID, userID string) string {
	t.Helper()

	hexPart, err := crypto.RandomHex(32)
	if err != nil {
		t.Fatalf("random hex: %v", err)
	}
	raw := "zit_ses_" + hexPart
	hash := crypto.HashTokenHex(raw)

	sessionID := instanceID + "-session-" + userID
	tokenID := instanceID + "-token-" + userID
	now := "2025-01-01 00:00:00"
	expiresAt := "2099-12-31 23:59:59"

	_, err = srv.DB.SQL().Exec(
		`INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at)
		 VALUES (?, ?, ?, '', ?, 'test', '127.0.0.1', '{}', ?, ?)`,
		sessionID, instanceID, userID, hash, now, expiresAt)
	if err != nil {
		t.Fatalf("insert session for %s: %v", instanceID, err)
	}

	_, err = srv.DB.SQL().Exec(
		`INSERT INTO tokens (id, instance_id, type, token_hash, user_id, session_id, scopes, expires_at, created_at)
		 VALUES (?, ?, 'session', ?, ?, ?, '[]', ?, ?)`,
		tokenID, instanceID, hash, userID, sessionID, expiresAt, now)
	if err != nil {
		t.Fatalf("insert token for %s: %v", instanceID, err)
	}

	return raw
}
