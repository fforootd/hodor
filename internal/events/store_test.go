package events

import (
	"context"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/testutil/storagetest"
)

func TestStoreListAggregateAndMaxID(t *testing.T) {
	storagetest.RunBackends(t, func(t *testing.T, db *database.DB, ctx context.Context) {
		store := NewStore(db)

		scoped := db.Scoped(ctx)
		if err := Append(ctx, scoped, "session.created", "user-a", "session-a", "session", map[string]any{"kind": "session"}); err != nil {
			t.Fatalf("append first event: %v", err)
		}
		if err := Append(ctx, scoped, "auth.login_completed", "user-a", "user-a", "auth", map[string]any{"kind": "auth"}); err != nil {
			t.Fatalf("append second event: %v", err)
		}

		otherCtx := storagetest.Context("instance_other")
		if err := Append(otherCtx, db.Scoped(otherCtx), "session.created", "user-b", "session-b", "session", nil); err != nil {
			t.Fatalf("append foreign event: %v", err)
		}

		page, nextCursor, err := store.List(ctx, Filter{Limit: 1})
		if err != nil {
			t.Fatalf("list first page: %v", err)
		}
		if len(page) != 1 {
			t.Fatalf("len(first page) = %d, want 1", len(page))
		}
		if nextCursor == "" {
			t.Fatal("expected next cursor")
		}

		all, _, err := store.List(ctx, Filter{Limit: 10, Types: []string{"session.created", "auth.login_completed"}})
		if err != nil {
			t.Fatalf("list all: %v", err)
		}
		if len(all) != 2 {
			t.Fatalf("len(all) = %d, want 2", len(all))
		}

		rows, err := store.AggregateCountsByEventType(ctx, "0")
		if err != nil {
			t.Fatalf("aggregate counts: %v", err)
		}
		if len(rows) != 2 {
			t.Fatalf("len(aggregate rows) = %d, want 2", len(rows))
		}

		maxID, err := store.MaxID(ctx)
		if err != nil {
			t.Fatalf("max id: %v", err)
		}
		if maxID == "" {
			t.Fatal("expected max id")
		}
	})
}
