package analytics

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/events"
	"github.com/zitadel/zitadel/internal/testutil/storagetest"
)

func TestOLTPBackendQueryRejectsMutationsAndAddsLimit(t *testing.T) {
	storagetest.RunBackends(t, func(t *testing.T, db *database.DB, ctx context.Context) {
		insertAnalyticsUser(t, db, ctx, "user-a", "user-a@example.com")

		backend := NewOLTPBackend(db)
		if _, err := backend.Query(ctx, `UPDATE users SET identifier = 'x'`, 10); err == nil {
			t.Fatal("expected UPDATE query to be rejected")
		}

		result, err := backend.Query(ctx, `SELECT id, identifier FROM users ORDER BY id`, 1)
		if err != nil {
			t.Fatalf("query users: %v", err)
		}
		if result.RowCount != 1 {
			t.Fatalf("row count = %d, want 1", result.RowCount)
		}
	})
}

func TestOLTPBackendTablesAndHandlersAreTenantScoped(t *testing.T) {
	storagetest.RunBackends(t, func(t *testing.T, db *database.DB, ctx context.Context) {
		insertAnalyticsUser(t, db, ctx, "user-a", "user-a@example.com")
		insertAnalyticsSession(t, db, ctx, "session-a", "user-a")
		if err := events.Append(ctx, db.Scoped(ctx), "session.created", "user-a", "session-a", "session", nil); err != nil {
			t.Fatalf("append event: %v", err)
		}

		otherCtx := storagetest.Context("instance_other")
		insertAnalyticsUser(t, db, otherCtx, "user-b", "user-b@example.com")
		insertAnalyticsSession(t, db, otherCtx, "session-b", "user-b")
		if err := events.Append(otherCtx, db.Scoped(otherCtx), "session.created", "user-b", "session-b", "session", nil); err != nil {
			t.Fatalf("append foreign event: %v", err)
		}

		backend := NewOLTPBackend(db)
		tables, err := backend.Tables(ctx)
		if err != nil {
			t.Fatalf("tables: %v", err)
		}
		if len(tables) != 3 {
			t.Fatalf("len(tables) = %d, want 3", len(tables))
		}
		for _, table := range tables {
			if table.RowCount != 1 {
				t.Fatalf("table %s row_count = %d, want 1", table.Name, table.RowCount)
			}
		}

		engine := New(backend)
		mux := http.NewServeMux()
		engine.RegisterRoutes(mux)

		queryReq := httptest.NewRequest(http.MethodPost, "/v1/analytics/query", bytes.NewReader([]byte(`{"sql":"SELECT id FROM users","limit":10}`)))
		queryReq = queryReq.WithContext(ctx)
		queryRec := httptest.NewRecorder()
		mux.ServeHTTP(queryRec, queryReq)
		if queryRec.Code != http.StatusOK {
			t.Fatalf("query status = %d body=%s", queryRec.Code, queryRec.Body.String())
		}

		tablesReq := httptest.NewRequest(http.MethodGet, "/v1/analytics/tables", nil).WithContext(ctx)
		tablesRec := httptest.NewRecorder()
		mux.ServeHTTP(tablesRec, tablesReq)
		if tablesRec.Code != http.StatusOK {
			t.Fatalf("tables status = %d body=%s", tablesRec.Code, tablesRec.Body.String())
		}

		schemaReq := httptest.NewRequest(http.MethodGet, "/v1/analytics/schema", nil).WithContext(ctx)
		schemaRec := httptest.NewRecorder()
		mux.ServeHTTP(schemaRec, schemaReq)
		if schemaRec.Code != http.StatusOK {
			t.Fatalf("schema status = %d body=%s", schemaRec.Code, schemaRec.Body.String())
		}

		var decoded map[string]any
		if err := json.Unmarshal(schemaRec.Body.Bytes(), &decoded); err != nil {
			t.Fatalf("decode schema response: %v", err)
		}
		if _, ok := decoded["users"]; !ok {
			t.Fatalf("expected users table in schema response: %#v", decoded)
		}
	})
}

func insertAnalyticsUser(t *testing.T, db *database.DB, ctx context.Context, userID, identifier string) {
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
		t.Fatalf("insert analytics user %s: %v", userID, err)
	}
}

func insertAnalyticsSession(t *testing.T, db *database.DB, ctx context.Context, sessionID, userID string) {
	t.Helper()

	scoped := db.Scoped(ctx)
	now := time.Now().UTC().Format(time.RFC3339)
	expiresAt := time.Now().UTC().Add(time.Hour).Format(time.RFC3339)
	if _, err := scoped.ExecContext(ctx, scoped.Rebind(
		`INSERT INTO sessions (instance_id, id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at)
		 VALUES (?, ?, ?, '_global', ?, 'analytics-test', '127.0.0.1', '{}', ?, ?)`),
		scoped.InstanceID(),
		sessionID,
		userID,
		"hash-"+sessionID,
		now,
		expiresAt,
	); err != nil {
		t.Fatalf("insert analytics session %s: %v", sessionID, err)
	}
}
