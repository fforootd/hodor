// Package api provides REST+JSON handlers for the Zitadel v2 API.
// Identity and schema CRUD are served as plain JSON endpoints.
// OpenAPI spec is dynamically generated from the schema registry.
package api

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// API holds the REST handlers and their dependencies.
type API struct {
	db      *database.DB
	bus     *eventbus.Bus
	cookies *session.CookieConfig
}

// New creates a new API handler.
func New(db *database.DB, bus *eventbus.Bus, cookies *session.CookieConfig) *API {
	return &API{db: db, bus: bus, cookies: cookies}
}

// RegisterRoutes mounts all REST API routes on the given mux.
// Authorization is handled by the FGA middleware (FGAGate) in the
// server middleware chain — individual routes no longer wrap requireAdmin.
func (a *API) RegisterRoutes(mux *http.ServeMux) {
	// Identity CRUD — FGA middleware handles authz.
	mux.HandleFunc("POST /v1/entities", a.createIdentity)
	mux.HandleFunc("GET /v1/entities", a.listIdentities)
	mux.HandleFunc("GET /v1/entities/{id}", a.getIdentity)
	mux.HandleFunc("PATCH /v1/entities/{id}", a.updateIdentity)
	mux.HandleFunc("DELETE /v1/entities/{id}", a.deleteIdentity)

	// Schema CRUD
	mux.HandleFunc("POST /v1/schemas", a.createSchema)
	mux.HandleFunc("GET /v1/schemas", a.listSchemas)
	mux.HandleFunc("GET /v1/schemas/$meta", a.getMetaSchema)
	mux.HandleFunc("GET /v1/schemas/{id}", a.getSchema)
	mux.HandleFunc("PATCH /v1/schemas/{id}", a.updateSchema)
	mux.HandleFunc("POST /v1/schemas/{id}/promote", a.promoteSchema)
	mux.HandleFunc("GET /v1/schemas/{id}/diff", a.diffSchema)
	mux.HandleFunc("POST /v1/schemas/{id}/preview", a.previewSchema)
	mux.HandleFunc("GET /v1/schemas/{id}/identity-count", a.schemaIdentityCount)

	// Session CRUD
	a.RegisterSessionRoutes(mux, noopMiddleware)

	// PAT (Personal Access Token) management
	a.RegisterPATRoutes(mux)

	// Event read + streaming
	a.RegisterEventRoutes(mux)

	// Universal search
	mux.HandleFunc("GET /v1/search", a.search)

	// Self-service account
	a.RegisterAccountRoutes(mux)

	// Provider federation
	a.RegisterProviderRoutes(mux)

	// Import & bulk operations
	a.RegisterBulkRoutes(mux)

	// Customer-facing FGA API
	a.RegisterFGARoutes(mux)

	// Hierarchical settings CRUD (ADR-009)
	a.RegisterSettingsRoutes(mux)

	// Dynamic OpenAPI
	mux.HandleFunc("GET /openapi.json", a.openAPISpec)

	// Well-known discovery
	mux.HandleFunc("GET /.well-known/zitadel-identity-schema", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/v1/schemas/$meta", http.StatusPermanentRedirect)
	})

	// Batch entity counts for sidebar badges
	mux.HandleFunc("GET /v1/counts", a.entityCounts)

	// Schema-driven route aliases (e.g. /v1/users → entities?schema_type=human_user)
	a.registerAliasRoutes(mux)
}

// registerAliasRoutes reads the x-catalog from the meta schema and registers
// /v1/{path} aliases for entity types.
func (a *API) registerAliasRoutes(mux *http.ServeMux) {
	catalog, err := schema.Catalog()
	if err != nil {
		logging.Printf("[alias] failed to load catalog: %v", err)
		return
	}

	for typeName, entry := range catalog {
		if entry.Storage != "entities" || entry.Path == "" {
			continue // Skip system views (sessions, events, jobs).
		}

		st := typeName
		prefix := "/v1/" + entry.Path

		mux.HandleFunc("GET "+prefix, a.aliasHandler(st, a.listIdentities))
		mux.HandleFunc("POST "+prefix, a.aliasHandler(st, a.createIdentity))
		mux.HandleFunc("GET "+prefix+"/{id}", a.getIdentity)
		mux.HandleFunc("PATCH "+prefix+"/{id}", a.updateIdentity)
		mux.HandleFunc("DELETE "+prefix+"/{id}", a.deleteIdentity)

		logging.Printf("[alias] registered /v1/%s → entities (type=%s)", entry.Path, st)
	}
}

// aliasHandler wraps a handler to inject schema_type into query params.
func (a *API) aliasHandler(schemaType string, next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		q := r.URL.Query()
		if q.Get("schema_type") == "" {
			q.Set("schema_type", schemaType)
			r.URL.RawQuery = q.Encode()
		}
		next(w, r)
	}
}

// --- Identity types ---

type IdentityRequest struct {
	Identifier   string   `json:"identifier"`
	DisplayName  string   `json:"display_name,omitempty"`
	Profile      any      `json:"profile,omitempty"`
	State        string   `json:"state,omitempty"`
	Capabilities []string `json:"capabilities,omitempty"`
}

type IdentityResponse struct {
	ID           string   `json:"id"`
	OrgID        string   `json:"org_id"`
	Identifier   string   `json:"identifier"`
	DisplayName  string   `json:"display_name,omitempty"`
	State        string   `json:"state"`
	Profile      any      `json:"profile,omitempty"`
	Metadata     any      `json:"metadata,omitempty"`
	Data         any      `json:"data,omitempty"`
	Capabilities []string `json:"capabilities,omitempty"`
	CreatedAt    string   `json:"created_at"`
	UpdatedAt    string   `json:"updated_at"`
}

type ListResponse struct {
	Items      any    `json:"items"`
	NextCursor string `json:"next_cursor,omitempty"`
	Total      int    `json:"total,omitempty"`
}

type ErrorResponse struct {
	Error   string `json:"error"`
	Code    int    `json:"code"`
	Details string `json:"details,omitempty"`
}

// --- Identity handlers ---

func (a *API) createIdentity(w http.ResponseWriter, r *http.Request) {
	var req IdentityRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Identifier == "" {
		httputil.WriteError(w, http.StatusBadRequest, "identifier is required")
		return
	}

	identityID := id.New()

	now := time.Now().UTC().Format(time.RFC3339)
	profileJSON := "{}"
	if req.Profile != nil {
		b, err := json.Marshal(req.Profile)
		if err != nil {
			httputil.WriteError(w, http.StatusBadRequest, "invalid profile field")
			return
		}
		profileJSON = string(b)
	}

	// TODO: validate data against schema if req.SchemaID is set

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, 'active', ?, '{}', ?, ?)`,
		identityID, req.Identifier, req.DisplayName, profileJSON, now, now,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusConflict, "identity already exists or database error")
		return
	}

	// Enforce uniqueness via unique_fields table (ADR-016).
	orgID := r.Header.Get("X-Org-Id")
	if orgID == "" {
		orgID = "1"
	}
	if err := uniqueness.EnforceFromIdentifier(r.Context(), tx, identityID, orgID, req.Identifier); err != nil {
		if v, ok := err.(*uniqueness.ViolationError); ok {
			httputil.WriteJSON(w, http.StatusConflict, map[string]any{
				"error": "uniqueness_violation",
				"field": v.Field,
				"value": v.Value,
				"scope": v.Scope,
			})
			return
		}
		httputil.WriteError(w, http.StatusConflict, "identifier already exists")
		return
	}

	// Insert capabilities.
	for _, cap := range req.Capabilities {
		_, err = tx.ExecContext(r.Context(),
			`INSERT INTO entity_capabilities (entity_id, capability) VALUES (?, ?)`,
			identityID, cap)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "failed to add capability")
			return
		}
	}

	// Promote indexed fields.
	promoteIndexes(r.Context(), tx, "identity", identityID, profileJSON)

	// Emit event.
	emitEvent(r.Context(), tx, "identity.created", identityID, identityID, "identity", map[string]any{
		"identifier": req.Identifier,
	})

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	// Wire FGA: write ownership + org tuples for the new entity.
	if svc := FGAService; svc != nil {
		creatorID := r.Header.Get("X-Identity-Id")
		if creatorID == "" {
			creatorID = "admin" // fallback for bootstrap
		}
		orgID := r.Header.Get("X-Org-Id")
		if orgID == "" {
			orgID = "1" // default org
		}
		if err := svc.OnEntityCreated(r.Context(), identityID, creatorID, orgID); err != nil {
			logging.Printf("[fga] warn: failed to write entity tuples: %v", err)
		}
	}

	resp := IdentityResponse{
		ID:           identityID,
		OrgID:        "org_default",
		Identifier:   req.Identifier,
		DisplayName:  req.DisplayName,
		State:        "active",
		Profile:      req.Profile,
		Capabilities: req.Capabilities,
		CreatedAt:    now,
		UpdatedAt:    now,
	}
	httputil.WriteJSON(w, http.StatusCreated, resp)
}

func (a *API) getIdentity(w http.ResponseWriter, r *http.Request) {
	identityID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	resp, err := a.loadIdentity(r, identityID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, resp)
}

func (a *API) listIdentities(w http.ResponseWriter, r *http.Request) {
	limit := 50
	if l := r.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 200 {
			limit = n
		}
	}
	var cursor string
	if c := r.URL.Query().Get("cursor"); c != "" {
		cursor = c
	}

	// Optional schema_type filter (e.g. ?schema_type=app for OIDC clients).
	schemaType := r.URL.Query().Get("schema_type")
	// Optional org_id filter for org context scoping.
	orgIDFilter := r.URL.Query().Get("org_id")

	var rows *sql.Rows
	var err error

	// Build query dynamically based on filters.
	var where []string
	var args []any
	baseSelect := `SELECT i.id, i.org_id, i.identifier, i.display_name, i.state, i.profile, i.metadata, i.data, i.created_at, i.updated_at
		 FROM entities i`
	if schemaType != "" {
		baseSelect += ` JOIN schemas s ON i.schema_id = s.id`
		where = append(where, `s.type = ?`)
		args = append(args, schemaType)
	}
	if orgIDFilter != "" {
		where = append(where, `i.org_id = ?`)
		args = append(args, orgIDFilter)
	}
	where = append(where, `i.id > ?`)
	args = append(args, cursor)

	query := baseSelect + ` WHERE ` + strings.Join(where, " AND ") + ` ORDER BY i.id ASC LIMIT ?`
	args = append(args, limit+1)
	rows, err = a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var identities []IdentityResponse
	for rows.Next() {
		ident, err := scanIdentityRow(rows)
		if err != nil {
			continue
		}
		// Load capabilities.
		ident.Capabilities = a.loadCapabilities(r, ident.ID)
		identities = append(identities, ident)
	}

	var nextCursor string
	if len(identities) > limit {
		identities = identities[:limit]
		nextCursor = identities[len(identities)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{
		Items:      identities,
		NextCursor: nextCursor,
	})
}

func (a *API) updateIdentity(w http.ResponseWriter, r *http.Request) {
	identityID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var req IdentityRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	setClauses := []string{"updated_at = ?"}
	args := []any{now}

	if req.State != "" {
		setClauses = append(setClauses, "state = ?")
		args = append(args, req.State)
	}
	if req.Profile != nil {
		profileJSON, _ := json.Marshal(req.Profile)
		setClauses = append(setClauses, "profile = ?")
		args = append(args, string(profileJSON))
		// Re-promote indexes.
		promoteIndexes(r.Context(), tx, "identity", identityID, string(profileJSON))
	}
	if req.DisplayName != "" {
		setClauses = append(setClauses, "display_name = ?")
		args = append(args, req.DisplayName)
	}
	args = append(args, identityID)

	query := "UPDATE entities SET " + strings.Join(setClauses, ", ") + " WHERE id = ?" //nolint:gosec // G202: setClauses are hardcoded column names, not user input.
	result, err := tx.ExecContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
		return
	}

	emitEvent(r.Context(), tx, "identity.updated", identityID, identityID, "identity", map[string]any{
		"state": req.State,
	})

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	resp, _ := a.loadIdentity(r, identityID)
	httputil.WriteJSON(w, http.StatusOK, resp)
}

func (a *API) deleteIdentity(w http.ResponseWriter, r *http.Request) {
	identityID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	// Clean up promoted indexes.
	_, _ = tx.ExecContext(r.Context(), `DELETE FROM entity_indexes WHERE entity_type = 'identity' AND entity_id = ?`, identityID)

	result, err := tx.ExecContext(r.Context(), `DELETE FROM entities WHERE id = ?`, identityID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
		return
	}

	emitEvent(r.Context(), tx, "identity.deleted", identityID, identityID, "identity", nil)

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	// Wire FGA: clean up all tuples for deleted entity.
	if svc := FGAService; svc != nil {
		if err := svc.OnEntityDeleted(r.Context(), identityID); err != nil {
			logging.Printf("[fga] warn: failed to delete entity tuples: %v", err)
		}
	}

	w.WriteHeader(http.StatusNoContent)
}

// --- Schema handlers ---

type SchemaRequest struct {
	ID      string `json:"id"`
	Type    string `json:"type"`
	OrgID   string `json:"org_id,omitempty"`
	Schema  any    `json:"schema"`  // JSON Schema document
	Message string `json:"message"` // Version commit message
}

type SchemaResponse struct {
	ID        string `json:"id"`
	Type      string `json:"type"`
	OrgID     string `json:"org_id"`
	Schema    any    `json:"schema"`
	Version   int    `json:"version"`
	IsDefault bool   `json:"is_default"`
	Message   string `json:"message"`
	CreatedBy string `json:"created_by,omitempty"`
	CreatedAt string `json:"created_at"`
}

func (a *API) createSchema(w http.ResponseWriter, r *http.Request) {
	var req SchemaRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Type == "" || req.Schema == nil {
		httputil.WriteError(w, http.StatusBadRequest, "type and schema are required")
		return
	}
	if req.OrgID == "" {
		req.OrgID = "org_default"
	}

	schemaJSON, err := json.Marshal(req.Schema)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid schema")
		return
	}

	// Validate x-auth-methods keys.
	if validationErr := validateSchemaAnnotations(schemaJSON); validationErr != "" {
		httputil.WriteError(w, http.StatusBadRequest, validationErr)
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)

	// Auto-increment version for this type+org.
	var maxVersion int
	a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(MAX(version), 0) FROM schemas WHERE type = ? AND org_id = ?`,
		req.Type, req.OrgID).Scan(&maxVersion)
	newVersion := maxVersion + 1

	// First version of a type becomes default automatically.
	var existingDefault int
	a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM schemas WHERE type = ? AND org_id = ? AND is_default = true`,
		req.Type, req.OrgID).Scan(&existingDefault)
	isDefault := existingDefault == 0

	// Generate ID: {type}_v{version}
	schemaID := req.ID
	if schemaID == "" {
		schemaID = fmt.Sprintf("%s_v%d", req.Type, newVersion)
	}

	// Get actor from session.
	createdBy := "" // TODO: extract from session when available

	_, err = a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO schemas (id, type, org_id, schema, version, is_default, message, created_by, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		schemaID, req.Type, req.OrgID, string(schemaJSON), newVersion, isDefault,
		req.Message, createdBy, now)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to save schema: "+err.Error())
		return
	}

	httputil.WriteJSON(w, http.StatusCreated, SchemaResponse{
		ID:        schemaID,
		Type:      req.Type,
		OrgID:     req.OrgID,
		Schema:    req.Schema,
		Version:   newVersion,
		IsDefault: isDefault,
		Message:   req.Message,
		CreatedBy: createdBy,
		CreatedAt: now,
	})
}

func (a *API) listSchemas(w http.ResponseWriter, r *http.Request) {
	typeFilter := r.URL.Query().Get("type")

	baseQuery := `SELECT id, type, org_id, schema, version, COALESCE(is_default, false), COALESCE(message,''), COALESCE(created_by,''), created_at FROM schemas`
	var args []any
	var query string
	if typeFilter != "" {
		query = baseQuery + ` WHERE type = ? ORDER BY version DESC`
		args = []any{typeFilter}
	} else {
		query = baseQuery + ` ORDER BY type, version DESC`
	}

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var schemas []SchemaResponse
	for rows.Next() {
		var s SchemaResponse
		var schemaStr string
		if err := rows.Scan(&s.ID, &s.Type, &s.OrgID, &schemaStr, &s.Version, &s.IsDefault, &s.Message, &s.CreatedBy, &s.CreatedAt); err != nil {
			continue
		}
		json.Unmarshal([]byte(schemaStr), &s.Schema)
		schemas = append(schemas, s)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: schemas})
}

func (a *API) getSchema(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	var s SchemaResponse
	var schemaStr string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, type, org_id, schema, version, COALESCE(is_default, false), COALESCE(message,''), COALESCE(created_by,''), created_at FROM schemas WHERE id = ?`, schemaID,
	).Scan(&s.ID, &s.Type, &s.OrgID, &schemaStr, &s.Version, &s.IsDefault, &s.Message, &s.CreatedBy, &s.CreatedAt)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "schema not found")
		return
	}
	json.Unmarshal([]byte(schemaStr), &s.Schema)

	httputil.WriteJSON(w, http.StatusOK, s)
}

// updateSchema creates a NEW version of the schema (append-only).
// The old version is preserved. The new version is NOT default until promoted.
func (a *API) updateSchema(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	var req struct {
		Schema  any    `json:"schema"`
		Message string `json:"message"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Schema == nil {
		httputil.WriteError(w, http.StatusBadRequest, "schema is required")
		return
	}

	// Load existing schema to get type+org.
	var schemaType string
	var orgID string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT type, org_id FROM schemas WHERE id = ?`, schemaID,
	).Scan(&schemaType, &orgID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "schema not found")
		return
	}

	schemaJSON, err := json.Marshal(req.Schema)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid schema")
		return
	}

	// Validate x-auth-methods keys.
	if validationErr := validateSchemaAnnotations(schemaJSON); validationErr != "" {
		httputil.WriteError(w, http.StatusBadRequest, validationErr)
		return
	}

	// Auto-increment version.
	var maxVersion int
	a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(MAX(version), 0) FROM schemas WHERE type = ? AND org_id = ?`,
		schemaType, orgID).Scan(&maxVersion)
	newVersion := maxVersion + 1
	newID := fmt.Sprintf("%s_v%d", schemaType, newVersion)

	createdBy := "" // TODO: extract from session when available
	now := time.Now().UTC().Format(time.RFC3339)

	// INSERT new version (append-only). NOT default until promoted.
	_, err = a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO schemas (id, type, org_id, schema, version, is_default, message, created_by, created_at)
		 VALUES (?, ?, ?, ?, ?, false, ?, ?, ?)`,
		newID, schemaType, orgID, string(schemaJSON), newVersion,
		req.Message, createdBy, now)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create version: "+err.Error())
		return
	}

	a.EmitAuthEvent(r.Context(), "schema.version_created", "", map[string]any{
		"schema_id":    newID,
		"type":         schemaType,
		"version":      newVersion,
		"message":      req.Message,
		"from_version": schemaID,
	})

	httputil.WriteJSON(w, http.StatusCreated, SchemaResponse{
		ID:        newID,
		Type:      schemaType,
		OrgID:     orgID,
		Schema:    req.Schema,
		Version:   newVersion,
		IsDefault: false,
		Message:   req.Message,
		CreatedBy: createdBy,
		CreatedAt: now,
	})
}

// promoteSchema sets a schema version as the default for its type.
func (a *API) promoteSchema(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	// Get type+org of the target schema.
	var schemaType string
	var orgID string
	var version int
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT type, org_id, version FROM schemas WHERE id = ?`, schemaID,
	).Scan(&schemaType, &orgID, &version)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "schema not found")
		return
	}

	// Count affected entities (those NOT pinned to a specific version).
	var affected int
	a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM entities i
		 JOIN schemas s ON i.schema_id = s.id
		 WHERE s.type = ? AND s.org_id = ? AND s.is_default = true`,
		schemaType, orgID).Scan(&affected)

	// Unset previous default.
	a.db.SQL().ExecContext(r.Context(),
		`UPDATE schemas SET is_default = false WHERE type = ? AND org_id = ?`,
		schemaType, orgID)

	// Set new default.
	a.db.SQL().ExecContext(r.Context(),
		`UPDATE schemas SET is_default = true WHERE id = ?`, schemaID)

	a.EmitAuthEvent(r.Context(), "schema.promoted", "", map[string]any{
		"schema_id": schemaID, "type": schemaType, "version": version,
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"status":            "promoted",
		"schema_id":         schemaID,
		"version":           version,
		"affected_entities": affected,
	})
}

// diffSchema returns a JSON diff between two schema versions.
func (a *API) diffSchema(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")
	compareID := r.URL.Query().Get("compare")
	if compareID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "compare query parameter required")
		return
	}

	// Load both schemas.
	var leftStr, rightStr string
	var leftVersion, rightVersion int
	var leftMsg, rightMsg string

	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT schema, version, COALESCE(message,'') FROM schemas WHERE id = ?`, schemaID,
	).Scan(&leftStr, &leftVersion, &leftMsg)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "schema not found: "+schemaID)
		return
	}

	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT schema, version, COALESCE(message,'') FROM schemas WHERE id = ?`, compareID,
	).Scan(&rightStr, &rightVersion, &rightMsg)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "schema not found: "+compareID)
		return
	}

	// Parse into maps for field-level diff.
	var leftSchema, rightSchema map[string]any
	json.Unmarshal([]byte(leftStr), &leftSchema)
	json.Unmarshal([]byte(rightStr), &rightSchema)

	// Extract properties for comparison.
	leftProps, _ := extractProperties(leftSchema)
	rightProps, _ := extractProperties(rightSchema)

	// Build diff.
	var changes []map[string]any
	allFields := make(map[string]bool)
	for k := range leftProps {
		allFields[k] = true
	}
	for k := range rightProps {
		allFields[k] = true
	}

	for field := range allFields {
		leftJSON, _ := json.Marshal(leftProps[field])
		rightJSON, _ := json.Marshal(rightProps[field])

		if leftProps[field] == nil {
			changes = append(changes, map[string]any{
				"field": field, "action": "added", "new": rightProps[field],
			})
		} else if rightProps[field] == nil {
			changes = append(changes, map[string]any{
				"field": field, "action": "removed", "old": leftProps[field],
			})
		} else if string(leftJSON) != string(rightJSON) {
			changes = append(changes, map[string]any{
				"field": field, "action": "modified", "old": leftProps[field], "new": rightProps[field],
			})
		}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"left":    map[string]any{"id": schemaID, "version": leftVersion, "message": leftMsg},
		"right":   map[string]any{"id": compareID, "version": rightVersion, "message": rightMsg},
		"changes": changes,
	})
}

func extractProperties(schema map[string]any) (map[string]any, bool) {
	props, ok := schema["properties"].(map[string]any)
	return props, ok
}

// schemaClaims maps identity data fields to standard OIDC claim names using
// x-claim (or legacy x-claim-mapping) annotations from the schema.
// Inline version to avoid import cycle with login package.
func schemaClaims(schemaJSON string, data map[string]any) map[string]any {
	var s struct {
		Properties map[string]map[string]any `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &s); err != nil {
		return nil
	}

	// Fallback: well-known field name → OIDC claim name
	knownClaims := map[string]string{
		"email": "email", "phone": "phone_number", "display_name": "name",
		"first_name": "given_name", "last_name": "family_name",
		"locale": "locale", "timezone": "zoneinfo", "avatar_url": "picture",
	}

	result := make(map[string]any)
	for field, def := range s.Properties {
		val, ok := data[field]
		if !ok || val == nil || val == "" {
			continue
		}
		claimName := ""
		if mapping, ok := def["x-claim"].(string); ok && strings.HasPrefix(mapping, "claims.") {
			// Extract "claims.email" → "email"
			rest := mapping[7:]
			end := 0
			for end < len(rest) {
				c := rest[end]
				if (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_' {
					end++
				} else {
					break
				}
			}
			if end > 0 {
				claimName = rest[:end]
			}
		}
		if claimName == "" {
			claimName = knownClaims[field]
		}
		if claimName != "" {
			result[claimName] = val
		}
	}
	return result
}

// previewSchema dry-runs claim mapping against a specific entity.
func (a *API) previewSchema(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	var req struct {
		EntityID string `json:"entity_id"` // identity ID or identifier
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.EntityID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "entity_id is required")
		return
	}

	// Load the draft schema.
	var draftSchemaStr string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT schema FROM schemas WHERE id = ?`, schemaID,
	).Scan(&draftSchemaStr)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "schema not found")
		return
	}

	// Load entity data.
	var dataStr, currentSchemaStr sql.NullString
	var identifier string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT i.identifier, i.data, COALESCE(sc.schema, '{}')
		 FROM entities i
		 LEFT JOIN schemas sc ON i.schema_id = sc.id
		 WHERE i.identifier = ? OR CAST(i.id AS TEXT) = ?`,
		req.EntityID, req.EntityID,
	).Scan(&identifier, &dataStr, &currentSchemaStr)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "entity not found")
		return
	}

	var data map[string]any
	if dataStr.Valid {
		json.Unmarshal([]byte(dataStr.String), &data)
	}
	if data == nil {
		data = make(map[string]any)
	}

	// Current claims (with existing schema).
	currentClaims := schemaClaims(currentSchemaStr.String, data)

	// Draft claims (with new schema).
	draftClaims := schemaClaims(draftSchemaStr, data)

	// Build diff.
	allClaims := make(map[string]bool)
	for k := range currentClaims {
		allClaims[k] = true
	}
	for k := range draftClaims {
		allClaims[k] = true
	}
	var claimChanges []map[string]any
	for claim := range allClaims {
		old := currentClaims[claim]
		new_ := draftClaims[claim]
		if fmt.Sprint(old) != fmt.Sprint(new_) {
			claimChanges = append(claimChanges, map[string]any{
				"claim": claim, "current": old, "draft": new_,
			})
		}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"entity":         identifier,
		"current_claims": currentClaims,
		"draft_claims":   draftClaims,
		"changes":        claimChanges,
	})
}

// entityCounts returns entity counts per schema type for sidebar badges.
// GET /v1/counts → { "human_user": 8, "service_user": 3, ... }
func (a *API) entityCounts(w http.ResponseWriter, r *http.Request) {
	counts := make(map[string]int)

	// Count entities by schema type.
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT s.type, COUNT(*) FROM entities i
		 JOIN schemas s ON i.schema_id = s.id
		 GROUP BY s.type`)
	if err == nil {
		defer rows.Close()
		for rows.Next() {
			var schemaType string
			var count int
			if rows.Scan(&schemaType, &count) == nil {
				counts[schemaType] = count
			}
		}
		_ = rows.Err()
	}

	// Count providers separately (dedicated table).
	var providerCount int
	if err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM providers`).Scan(&providerCount); err == nil {
		counts["provider"] = providerCount
	}

	// Count orgs separately (dedicated table, org_id on entities).
	var orgCount int
	if err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(DISTINCT org_id) FROM entities WHERE org_id > 0`).Scan(&orgCount); err == nil {
		counts["org"] = orgCount
	}

	httputil.WriteJSON(w, http.StatusOK, counts)
}

// getMetaSchema returns the canonical Zitadel identity schema meta-schema.
func (a *API) getMetaSchema(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/schema+json")
	w.WriteHeader(http.StatusOK)
	w.Write([]byte(schema.MetaSchema))
}

// validateSchemaAnnotations validates x-auth-methods keys against the allowed set.
func validateSchemaAnnotations(schemaJSON []byte) string {
	var raw map[string]json.RawMessage
	if json.Unmarshal(schemaJSON, &raw) != nil {
		return "invalid JSON"
	}

	// Validate x-auth-methods keys.
	if authMethodsRaw, ok := raw["x-auth-methods"]; ok {
		var methods map[string]any
		if json.Unmarshal(authMethodsRaw, &methods) != nil {
			return "x-auth-methods must be an object"
		}
		allowed := map[string]bool{
			"password": true, "passkey": true, "magic_link": true,
			"sso": true, "pat": true, "api_key": true, "client_cert": true,
		}
		for key := range methods {
			if !allowed[key] {
				return fmt.Sprintf("unknown auth method %q in x-auth-methods; allowed: password, passkey, magic_link, sso, pat, api_key, client_cert", key)
			}
		}
	}

	return ""
}

func (a *API) schemaIdentityCount(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	var count int
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM entities WHERE schema_id = ?`, schemaID,
	).Scan(&count)
	if err != nil {
		count = 0
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"count": count})
}

func (a *API) loadIdentity(r *http.Request, identityID string) (IdentityResponse, error) {
	var resp IdentityResponse
	var displayName, profileStr, metaStr, dataStr sql.NullString
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, org_id, identifier, display_name, state, profile, metadata, data, created_at, updated_at
		 FROM entities WHERE id = ?`, identityID,
	).Scan(&resp.ID, &resp.OrgID, &resp.Identifier, &displayName, &resp.State,
		&profileStr, &metaStr, &dataStr, &resp.CreatedAt, &resp.UpdatedAt)
	if err != nil {
		return resp, err
	}
	if displayName.Valid {
		resp.DisplayName = displayName.String
	}
	if profileStr.Valid {
		json.Unmarshal([]byte(profileStr.String), &resp.Profile)
	}
	if metaStr.Valid {
		json.Unmarshal([]byte(metaStr.String), &resp.Metadata)
	}
	if dataStr.Valid {
		json.Unmarshal([]byte(dataStr.String), &resp.Data)
	}
	resp.Capabilities = a.loadCapabilities(r, identityID)
	return resp, nil
}

func (a *API) loadCapabilities(r *http.Request, identityID string) []string {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT capability FROM entity_capabilities WHERE entity_id = ?`, identityID)
	if err != nil {
		return nil
	}
	defer rows.Close()
	var caps []string
	for rows.Next() {
		var c string
		rows.Scan(&c)
		caps = append(caps, c)
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return caps
}

func scanIdentityRow(rows *sql.Rows) (IdentityResponse, error) {
	var resp IdentityResponse
	var displayName, profileStr, metaStr, dataStr sql.NullString
	err := rows.Scan(&resp.ID, &resp.OrgID, &resp.Identifier, &displayName, &resp.State,
		&profileStr, &metaStr, &dataStr, &resp.CreatedAt, &resp.UpdatedAt)
	if err != nil {
		return resp, err
	}
	if displayName.Valid {
		resp.DisplayName = displayName.String
	}
	if profileStr.Valid {
		json.Unmarshal([]byte(profileStr.String), &resp.Profile)
	}
	if metaStr.Valid {
		json.Unmarshal([]byte(metaStr.String), &resp.Metadata)
	}
	if dataStr.Valid {
		json.Unmarshal([]byte(dataStr.String), &resp.Data)
	}
	return resp, nil
}

func parseID(r *http.Request, name string) (string, error) {
	v := r.PathValue(name)
	if v == "" {
		return "", fmt.Errorf("missing path param %q", name)
	}
	return v, nil
}

// writeError wraps httputil.WriteError with the API's ErrorResponse format.
func writeError(w http.ResponseWriter, status int, msg string) {
	httputil.WriteJSON(w, status, ErrorResponse{Error: msg, Code: status})
}

// --- Universal Search ---

type SearchResult struct {
	ResourceType string `json:"resource_type"` // entity, schema, event, session, provider
	ID           string `json:"id"`
	Title        string `json:"title"`
	Subtitle     string `json:"subtitle,omitempty"`
	Link         string `json:"link"`
}

func (a *API) search(w http.ResponseWriter, r *http.Request) {
	q := strings.TrimSpace(r.URL.Query().Get("q"))
	if q == "" {
		httputil.WriteJSON(w, http.StatusOK, map[string]any{"results": []any{}, "query": ""})
		return
	}

	limit := 10
	if l := r.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 50 {
			limit = n
		}
	}

	pattern := "%" + q + "%"
	results := make([]SearchResult, 0, limit*5)

	results = append(results, a.searchEntities(r, pattern, limit)...)
	results = append(results, a.searchEntityIndexes(r, pattern, limit)...)
	results = append(results, a.searchSchemas(r, pattern, limit)...)
	results = append(results, a.searchEvents(r, pattern, limit)...)
	results = append(results, a.searchProviders(r, pattern, limit)...)

	// Deduplicate entities (may appear from both direct + index search)
	seen := map[string]bool{}
	var deduped []SearchResult
	for _, res := range results {
		key := res.ResourceType + ":" + res.ID
		if !seen[key] {
			seen[key] = true
			deduped = append(deduped, res)
		}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"results": deduped,
		"query":   q,
		"count":   len(deduped),
	})
}

func (a *API) searchEntities(r *http.Request, pattern string, limit int) []SearchResult {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, identifier, display_name, state FROM entities
		 WHERE identifier LIKE ? OR display_name LIKE ?
		 ORDER BY id DESC LIMIT ?`,
		pattern, pattern, limit)
	if err != nil {
		return nil
	}
	defer rows.Close()
	var results []SearchResult
	for rows.Next() {
		var id string
		var ident, displayName, state string
		var dn sql.NullString
		if err := rows.Scan(&id, &ident, &dn, &state); err != nil {
			continue
		}
		if dn.Valid {
			displayName = dn.String
		}
		results = append(results, SearchResult{
			ResourceType: "identity",
			ID:           id,
			Title:        ident,
			Subtitle:     displayName + " · " + state,
			Link:         "/admin/entities/" + id + "/edit",
		})
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return results
}

func (a *API) searchEntityIndexes(r *http.Request, pattern string, limit int) []SearchResult {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT DISTINCT ei.entity_id, e.identifier, e.display_name, ei.field, ei.value
		 FROM entity_indexes ei
		 JOIN entities e ON ei.entity_id = e.id
		 WHERE ei.value LIKE ?
		 LIMIT ?`,
		pattern, limit)
	if err != nil {
		return nil
	}
	defer rows.Close()
	seen := map[string]bool{}
	var results []SearchResult
	for rows.Next() {
		var entityID string
		var ident, field, value string
		var dn sql.NullString
		if err := rows.Scan(&entityID, &ident, &dn, &field, &value); err != nil {
			continue
		}
		idStr := entityID
		if seen[idStr] {
			continue
		}
		seen[idStr] = true
		displayName := ""
		if dn.Valid {
			displayName = dn.String
		}
		results = append(results, SearchResult{
			ResourceType: "identity",
			ID:           idStr,
			Title:        ident,
			Subtitle:     fmt.Sprintf("%s: %s · %s", field, value, displayName),
			Link:         "/admin/entities/" + entityID + "/edit",
		})
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return results
}

func (a *API) searchSchemas(r *http.Request, pattern string, limit int) []SearchResult {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, type FROM schemas WHERE id LIKE ? OR type LIKE ? LIMIT ?`,
		pattern, pattern, limit)
	if err != nil {
		return nil
	}
	defer rows.Close()
	var results []SearchResult
	for rows.Next() {
		var schemaID, schemaType string
		if err := rows.Scan(&schemaID, &schemaType); err != nil {
			continue
		}
		results = append(results, SearchResult{
			ResourceType: "schema",
			ID:           schemaID,
			Title:        schemaType,
			Subtitle:     schemaID,
			Link:         "/console/schemas",
		})
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return results
}

func (a *API) searchEvents(r *http.Request, pattern string, limit int) []SearchResult {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, event_type, created_at FROM events WHERE event_type LIKE ? ORDER BY id DESC LIMIT ?`,
		pattern, limit)
	if err != nil {
		return nil
	}
	defer rows.Close()
	var results []SearchResult
	for rows.Next() {
		var evtID string
		var evtType, createdAt string
		if err := rows.Scan(&evtID, &evtType, &createdAt); err != nil {
			continue
		}
		results = append(results, SearchResult{
			ResourceType: "event",
			ID:           evtID,
			Title:        evtType,
			Subtitle:     createdAt,
			Link:         "/console/events",
		})
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return results
}

func (a *API) searchProviders(r *http.Request, pattern string, limit int) []SearchResult {
	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, name, protocol, template FROM providers
		 WHERE name LIKE ? OR template LIKE ?
		 ORDER BY name LIMIT ?`,
		pattern, pattern, limit)
	if err != nil {
		return nil
	}
	defer rows.Close()
	var results []SearchResult
	for rows.Next() {
		var provID, name, protocol, tmpl string
		if err := rows.Scan(&provID, &name, &protocol, &tmpl); err != nil {
			continue
		}
		results = append(results, SearchResult{
			ResourceType: "provider",
			ID:           provID,
			Title:        name,
			Subtitle:     protocol + " · " + tmpl,
			Link:         "/console/providers",
		})
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return results
}

func promoteIndexes(ctx context.Context, tx *sql.Tx, entityType string, entityID string, dataJSON string) {
	_, _ = tx.ExecContext(ctx,
		`DELETE FROM entity_indexes WHERE entity_type = ? AND entity_id = ?`,
		entityType, entityID)

	var data map[string]any
	if json.Unmarshal([]byte(dataJSON), &data) != nil {
		return
	}
	for field, val := range data {
		if strVal, ok := val.(string); ok && strVal != "" {
			_, _ = tx.ExecContext(ctx,
				`INSERT OR REPLACE INTO entity_indexes (entity_type, entity_id, field, value) VALUES (?, ?, ?, ?)`,
				entityType, entityID, field, strVal)
		}
	}
}

func emitEvent(ctx context.Context, tx *sql.Tx, eventType string, actorID, aggregateID string, aggregateType string, payload map[string]any) {
	eventID := id.New()
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	traceID := telemetry.TraceIDFromContext(ctx)
	spanID := telemetry.SpanIDFromContext(ctx)
	parentSpanID := telemetry.ParentSpanIDFromContext(ctx)
	sessionID := telemetry.SessionIDFromContext(ctx)
	tx.ExecContext(ctx,
		`INSERT INTO events (id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, span_id, parent_span_id, session_id, created_at)
		 VALUES (?, ?, ?, '0', ?, '', ?, ?, ?, '{}', ?, ?, ?, ?, datetime('now'))`,
		eventID, eventType, eventCategory(eventType), actorID, aggregateID, aggregateType, payloadJSON, traceID, spanID, parentSpanID, sessionID)
}

func (a *API) EmitAuthEvent(ctx context.Context, eventType string, actorID string, payload map[string]any) {
	eventID := id.New()
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	traceID := telemetry.TraceIDFromContext(ctx)
	spanID := telemetry.SpanIDFromContext(ctx)
	parentSpanID := telemetry.ParentSpanIDFromContext(ctx)
	sessionID := telemetry.SessionIDFromContext(ctx)
	a.db.SQL().ExecContext(ctx,
		`INSERT INTO events (id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, span_id, parent_span_id, session_id, created_at)
		 VALUES (?, ?, ?, '0', ?, '', ?, 'auth', ?, '{}', ?, ?, ?, ?, datetime('now'))`,
		eventID, eventType, eventCategory(eventType), actorID, actorID, payloadJSON, traceID, spanID, parentSpanID, sessionID)
	a.bus.Signal()
}

// emitEventSimple is a package-level helper for event emission outside transactions.
func emitEventSimple(ctx context.Context, db interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}, eventType string, actorID string, aggregateID, aggregateType string, payload map[string]any) {
	eventIDVal := id.New()
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	traceID := telemetry.TraceIDFromContext(ctx)
	spanID := telemetry.SpanIDFromContext(ctx)
	parentSpanID := telemetry.ParentSpanIDFromContext(ctx)
	sessionID := telemetry.SessionIDFromContext(ctx)
	db.ExecContext(ctx, //nolint:errcheck // fire-and-forget audit event
		`INSERT INTO events (id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, span_id, parent_span_id, session_id, created_at)
		 VALUES (?, ?, ?, '0', ?, '', ?, ?, ?, '{}', ?, ?, ?, ?, datetime('now'))`,
		eventIDVal, eventType, eventCategory(eventType), actorID, aggregateID, aggregateType, payloadJSON, traceID, spanID, parentSpanID, sessionID)
}

// eventCategory derives the event category from the event_type prefix.
func eventCategory(eventType string) string {
	for i := 0; i < len(eventType); i++ {
		if eventType[i] == '.' {
			prefix := eventType[:i]
			switch prefix {
			case "entity", "identity", "provider", "settings", "schema":
				return "entity"
			case "auth":
				return "auth"
			case "session":
				return "session"
			case "token":
				return "token"
			case "request", "api":
				return "request"
			case "log":
				return "log"
			case "signal":
				return "signal"
			case "threat":
				return "threat"
			case "notification":
				return "system"
			}
			return prefix
		}
	}
	return "system"
}

// GetIdentityByID is an exported helper for the UI to get an identity (for edit form).
func (a *API) GetIdentityByID(r *http.Request, identityID string) (IdentityResponse, error) {
	return a.loadIdentity(r, identityID)
}

// CreateIdentityInternal is an exported helper for the UI to create an identity.
func (a *API) CreateIdentityInternal(r *http.Request, req IdentityRequest) (IdentityResponse, error) {
	identityID := id.New()

	now := time.Now().UTC().Format(time.RFC3339)
	profileJSON := "{}"
	if req.Profile != nil {
		b, _ := json.Marshal(req.Profile)
		profileJSON = string(b)
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		return IdentityResponse{}, fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, 'active', ?, '{}', ?, ?)`,
		identityID, req.Identifier, req.DisplayName, profileJSON, now, now)
	if err != nil {
		return IdentityResponse{}, fmt.Errorf("insert: %w", err)
	}

	for _, cap := range req.Capabilities {
		tx.ExecContext(r.Context(),
			`INSERT INTO entity_capabilities (entity_id, capability) VALUES (?, ?)`,
			identityID, cap)
	}

	promoteIndexes(r.Context(), tx, "identity", identityID, profileJSON)
	emitEvent(r.Context(), tx, "identity.created", identityID, identityID, "identity", map[string]any{
		"identifier": req.Identifier,
	})

	if err := tx.Commit(); err != nil {
		return IdentityResponse{}, fmt.Errorf("commit: %w", err)
	}
	a.bus.Signal()

	return IdentityResponse{
		ID: identityID, OrgID: "org_default", Identifier: req.Identifier, DisplayName: req.DisplayName,
		State: "active", Profile: req.Profile, Capabilities: req.Capabilities,
		CreatedAt: now, UpdatedAt: now,
	}, nil
}

// UpdateIdentityInternal is an exported helper for the UI to update an identity.
func (a *API) UpdateIdentityInternal(r *http.Request, identityID string, req IdentityRequest) (IdentityResponse, error) {
	now := time.Now().UTC().Format(time.RFC3339)
	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		return IdentityResponse{}, fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	setClauses := []string{"updated_at = ?"}
	args := []any{now}
	if req.State != "" {
		setClauses = append(setClauses, "state = ?")
		args = append(args, req.State)
	}
	if req.Profile != nil {
		profileJSON, _ := json.Marshal(req.Profile)
		setClauses = append(setClauses, "profile = ?")
		args = append(args, string(profileJSON))
		promoteIndexes(r.Context(), tx, "identity", identityID, string(profileJSON))
	}
	args = append(args, identityID)

	query := "UPDATE entities SET " + strings.Join(setClauses, ", ") + " WHERE id = ?" //nolint:gosec // G202: setClauses are hardcoded column names, not user input.
	result, err := tx.ExecContext(r.Context(), query, args...)
	if err != nil {
		return IdentityResponse{}, fmt.Errorf("update: %w", err)
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		return IdentityResponse{}, fmt.Errorf("identity %s", identityID)
	}

	emitEvent(r.Context(), tx, "identity.updated", identityID, identityID, "identity", nil)
	if err := tx.Commit(); err != nil {
		return IdentityResponse{}, fmt.Errorf("commit: %w", err)
	}
	a.bus.Signal()

	return a.loadIdentity(r, identityID)
}

// DeleteIdentityInternal is an exported helper for the UI to delete an identity.
func (a *API) DeleteIdentityInternal(r *http.Request, identityID string) error {
	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	tx.ExecContext(r.Context(), `DELETE FROM entity_indexes WHERE entity_type = 'identity' AND entity_id = ?`, identityID)
	result, err := tx.ExecContext(r.Context(), `DELETE FROM entities WHERE id = ?`, identityID)
	if err != nil {
		return fmt.Errorf("delete: %w", err)
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		return fmt.Errorf("identity %s", identityID)
	}

	emitEvent(r.Context(), tx, "identity.deleted", identityID, identityID, "identity", nil)
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}
	a.bus.Signal()
	return nil
}

// DB returns the database instance for direct queries (used by UI session handling).
func (a *API) DB() *database.DB { return a.db }
