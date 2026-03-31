package api

import (
	"encoding/json"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"strconv"
	"strings"

	"github.com/zitadel/zitadel/internal/catalog"
	"github.com/zitadel/zitadel/internal/database"
)

// RegisterCatalogRoutes registers the catalog API endpoints.
func RegisterCatalogRoutes(mux *http.ServeMux, svc *catalog.Service, db *database.DB) {
	mux.HandleFunc("GET /v1/catalog", listCatalog(svc))
	mux.HandleFunc("GET /v1/catalog/{id}", getCatalogEntry(svc))
	mux.HandleFunc("POST /v1/catalog/{id}/install", installFromCatalog(svc))
	mux.HandleFunc("POST /v1/catalog/refresh", refreshCatalog(svc))
	mux.HandleFunc("POST /v1/schemas/{type}/preview-upgrade", previewSchemaUpgrade(db))
}

// listCatalog returns all templates, optionally filtered by type and tags.
// GET /v1/catalog?type=action&tags=security
func listCatalog(svc *catalog.Service) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		typeFilter := r.URL.Query().Get("type")
		tagFilter := r.URL.Query().Get("tags")

		templates := svc.List(typeFilter, tagFilter)

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"templates":   templates,
			"total":       len(templates),
			"can_refresh": svc.CanRefresh(),
		})
	}
}

// getCatalogEntry returns the full template detail with variables.
// GET /v1/catalog/{id}
func getCatalogEntry(svc *catalog.Service) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		templateID := r.PathValue("id")

		payload, tpl, err := svc.Get(templateID)
		if err != nil {
			http.Error(w, err.Error(), http.StatusNotFound)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"template":  tpl,
			"variables": payload.Variables,
			"payload":   payload.Payload,
		})
	}
}

// installFromCatalog installs a template by substituting variables and creating an entity.
// POST /v1/catalog/{id}/install
func installFromCatalog(svc *catalog.Service) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		templateID := r.PathValue("id")

		var req struct {
			Variables map[string]any `json:"variables"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "invalid request body", http.StatusBadRequest)
			return
		}

		userID, err := svc.Install(r.Context(), templateID, req.Variables)
		if err != nil {
			logging.Printf("[catalog] install %s failed: %v", templateID, err)
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]any{
			"id":          userID,
			"template_id": templateID,
			"status":      "installed",
		})
	}
}

// refreshCatalog forces a remote catalog refresh.
// POST /v1/catalog/refresh
func refreshCatalog(svc *catalog.Service) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		count, err := svc.Refresh(r.Context())
		if err != nil {
			logging.Printf("[catalog] refresh failed: %v", err)
			status := http.StatusBadGateway
			if strings.Contains(err.Error(), "no remote URL configured") {
				status = http.StatusConflict
			}
			http.Error(w, err.Error(), status)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"status": "refreshed",
			"new":    count,
		})
	}
}

// previewSchemaUpgrade shows the impact of a schema change on existing entities.
// POST /v1/schemas/{type}/preview-upgrade
func previewSchemaUpgrade(db *database.DB) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		schemaType := r.PathValue("type")

		var req struct {
			NewSchema  map[string]any `json:"new_schema"`
			SampleSize int            `json:"sample_size"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "invalid request body", http.StatusBadRequest)
			return
		}

		sampleSize := req.SampleSize
		if sampleSize == 0 {
			sizeStr := r.URL.Query().Get("sample_size")
			if sizeStr != "" {
				sampleSize, _ = strconv.Atoi(sizeStr)
			}
		}

		report, err := catalog.PreviewUpgrade(r.Context(), db.SQL(), schemaType, req.NewSchema, sampleSize, db.Dialect())
		if err != nil {
			logging.Printf("[catalog] preview-upgrade %s failed: %v", schemaType, err)
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(report)
	}
}
