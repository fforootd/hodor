package server

import (
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
)

func TestReadyzReturns200WhenServerReady(t *testing.T) {
	srv, db := newReadyzTestServer(t)
	defer db.Close()

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/readyz", nil)

	srv.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("readyz status = %d, want %d", rec.Code, http.StatusOK)
	}
	if body := rec.Body.String(); body != "ready" {
		t.Fatalf("readyz body = %q, want %q", body, "ready")
	}
}

func TestReadyzReturns503WhenStartupNotComplete(t *testing.T) {
	srv, db := newReadyzTestServer(t)
	defer db.Close()
	srv.ready.Store(false)

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/readyz", nil)

	srv.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusServiceUnavailable {
		t.Fatalf("readyz status = %d, want %d", rec.Code, http.StatusServiceUnavailable)
	}
}

func TestReadyzReturns503WhenDatabasePingFails(t *testing.T) {
	srv, db := newReadyzTestServer(t)
	db.Close()

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/readyz", nil)

	srv.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusServiceUnavailable {
		t.Fatalf("readyz status = %d, want %d", rec.Code, http.StatusServiceUnavailable)
	}
}

func newReadyzTestServer(t *testing.T) (*Server, *database.DB) {
	t.Helper()

	cfg := config.Defaults()
	cfg.Database.URL = "sqlite://" + filepath.Join(t.TempDir(), "readyz.db")

	db, err := database.Open(cfg.Database.URL)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	if err := bootstrap.EnsureAdmin(t.Context(), db, ""); err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	return New(cfg, db, eventbus.New()), db
}
