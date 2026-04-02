package session

import (
	"context"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/testutil/storagetest"
)

func TestStoreCreateGetListAndRevoke(t *testing.T) {
	storagetest.RunBackends(t, func(t *testing.T, db *database.DB, ctx context.Context) {
		insertSessionTestUser(t, db, ctx, "user-a", "user-a@example.com")
		store := NewStore(db)
		now := time.Now().UTC()

		record, err := store.Create(ctx, CreateParams{
			SessionID: "session-a",
			TokenID:   "token-a",
			UserID:    "user-a",
			OrgID:     "_global",
			TokenHash: "hash-a",
			UserAgent: "unit-test",
			IPAddress: "127.0.0.1",
			Metadata:  map[string]any{"auth_method": "password"},
			CreatedAt: now,
			ExpiresAt: now.Add(time.Hour),
			SessionCreatedPayload: map[string]any{
				"user_id": "user-a",
			},
			RiskEvaluatedPayload: map[string]any{
				"recommendation": "allow_and_log",
			},
		})
		if err != nil {
			t.Fatalf("create session: %v", err)
		}
		if record.AuthMethod != "password" {
			t.Fatalf("auth method = %q, want password", record.AuthMethod)
		}

		loaded, err := store.Get(ctx, "session-a")
		if err != nil {
			t.Fatalf("get session: %v", err)
		}
		if loaded.UserID != "user-a" {
			t.Fatalf("loaded user_id = %q, want user-a", loaded.UserID)
		}

		otherCtx := storagetest.Context("instance_other")
		if _, err := store.Get(otherCtx, "session-a"); err == nil {
			t.Fatal("foreign tenant should not read the session")
		}

		list, err := store.List(ctx, "", 10)
		if err != nil {
			t.Fatalf("list sessions: %v", err)
		}
		if len(list) != 1 {
			t.Fatalf("len(list) = %d, want 1", len(list))
		}

		if err := store.Revoke(ctx, "session-a", "test_revoke"); err != nil {
			t.Fatalf("revoke session: %v", err)
		}

		revoked, err := store.Get(ctx, "session-a")
		if err != nil {
			t.Fatalf("get revoked session: %v", err)
		}
		if revoked.RevokedAt == nil || *revoked.RevokedAt == "" {
			t.Fatal("expected revoked_at to be set")
		}

		scoped := db.Scoped(ctx)
		var revokedAt string
		if err := scoped.QueryRowContext(ctx,
			scoped.Rebind(`SELECT COALESCE(revoked_at, '') FROM tokens WHERE instance_id = ? AND session_id = ?`),
			scoped.InstanceID(),
			"session-a",
		).Scan(&revokedAt); err != nil {
			t.Fatalf("load revoked token: %v", err)
		}
		if revokedAt == "" {
			t.Fatal("expected session token to be revoked")
		}
	})
}

func insertSessionTestUser(t *testing.T, db *database.DB, ctx context.Context, userID, identifier string) {
	t.Helper()

	scoped := db.Scoped(ctx)
	now := time.Now().UTC().Format(time.RFC3339)
	if _, err := scoped.ExecContext(ctx, scoped.Rebind(
		`INSERT INTO users (instance_id, id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, '1', ?, ?, 'human', 'active', 'human_user_v1', '{}', ?, ?)`),
		scoped.InstanceID(),
		userID,
		identifier,
		identifier,
		now,
		now,
	); err != nil {
		t.Fatalf("insert test user %s: %v", userID, err)
	}
}
