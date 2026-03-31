package api

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/zitadel/zitadel/internal/catalog"
	"github.com/zitadel/zitadel/internal/config"
)

func TestRefreshCatalogReturnsConflictWhenRemoteNotConfigured(t *testing.T) {
	svc := catalog.New(config.CatalogConfig{}, nil)

	req := httptest.NewRequest(http.MethodPost, "/v1/catalog/refresh", nil)
	rec := httptest.NewRecorder()

	refreshCatalog(svc).ServeHTTP(rec, req)

	if rec.Code != http.StatusConflict {
		t.Fatalf("expected %d, got %d: %s", http.StatusConflict, rec.Code, rec.Body.String())
	}
}

func TestListCatalogReportsRefreshCapability(t *testing.T) {
	svc := catalog.New(config.CatalogConfig{}, nil)

	req := httptest.NewRequest(http.MethodGet, "/v1/catalog", nil)
	rec := httptest.NewRecorder()

	listCatalog(svc).ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected %d, got %d: %s", http.StatusOK, rec.Code, rec.Body.String())
	}

	var body struct {
		CanRefresh bool `json:"can_refresh"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode response: %v", err)
	}

	if body.CanRefresh {
		t.Fatalf("expected can_refresh=false for unconfigured catalog")
	}
}
