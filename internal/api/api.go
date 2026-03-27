// Package api provides REST+JSON handlers for the Zitadel v2 API.
// Identity and schema CRUD are served as plain JSON endpoints.
// OpenAPI spec is dynamically generated from the schema registry.
package api

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/session"
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
func (a *API) RegisterRoutes(mux *http.ServeMux) {
	// Identity CRUD
	mux.HandleFunc("POST /v1/entities", a.createIdentity)
	mux.HandleFunc("GET /v1/entities", a.listIdentities)
	mux.HandleFunc("GET /v1/entities/{id}", a.getIdentity)
	mux.HandleFunc("PATCH /v1/entities/{id}", a.updateIdentity)
	mux.HandleFunc("DELETE /v1/entities/{id}", a.deleteIdentity)

	// Schema CRUD (write = admin-only, read = public)
	mux.HandleFunc("POST /v1/schemas", a.requireAdmin(a.createSchema))
	mux.HandleFunc("GET /v1/schemas", a.listSchemas)
	mux.HandleFunc("GET /v1/schemas/$meta", a.getMetaSchema)
	mux.HandleFunc("GET /v1/schemas/{id}", a.getSchema)
	mux.HandleFunc("PATCH /v1/schemas/{id}", a.requireAdmin(a.updateSchema))
	mux.HandleFunc("POST /v1/schemas/{id}/promote", a.requireAdmin(a.promoteSchema))
	mux.HandleFunc("GET /v1/schemas/{id}/diff", a.diffSchema)
	mux.HandleFunc("POST /v1/schemas/{id}/preview", a.previewSchema)
	mux.HandleFunc("GET /v1/schemas/{id}/identity-count", a.schemaIdentityCount)

	// Session CRUD
	a.RegisterSessionRoutes(mux, a.requireAdmin)

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

	// Dynamic OpenAPI
	mux.HandleFunc("GET /openapi.json", a.openAPISpec)

	// Well-known discovery
	mux.HandleFunc("GET /.well-known/zitadel-identity-schema", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/v1/schemas/$meta", http.StatusPermanentRedirect)
	})

	// Schema-driven route aliases (e.g. /v1/users → entities?schema_type=human_user)
	a.registerAliasRoutes(mux)
}

// registerAliasRoutes reads the x-catalog from the meta schema and registers
// /v1/{path} aliases for entity types.
func (a *API) registerAliasRoutes(mux *http.ServeMux) {
	catalog, err := schema.Catalog()
	if err != nil {
		log.Printf("[alias] failed to load catalog: %v", err)
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

		log.Printf("[alias] registered /v1/%s → entities (type=%s)", entry.Path, st)
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
	ID           int64    `json:"id,string"`
	OrgID        int64    `json:"org_id,string"`
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
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Identifier == "" {
		writeError(w, http.StatusBadRequest, "identifier is required")
		return
	}

	identityID, err := id.New()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "generate id failed")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	profileJSON := "{}"
	if req.Profile != nil {
		b, err := json.Marshal(req.Profile)
		if err != nil {
			writeError(w, http.StatusBadRequest, "invalid profile field")
			return
		}
		profileJSON = string(b)
	}

	// TODO: validate data against schema if req.SchemaID is set

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, 'active', ?, '{}', ?, ?)`,
		identityID, req.Identifier, req.DisplayName, profileJSON, now, now,
	)
	if err != nil {
		writeError(w, http.StatusConflict, "identity already exists or database error")
		return
	}

	// Insert capabilities.
	for _, cap := range req.Capabilities {
		_, err = tx.ExecContext(r.Context(),
			`INSERT INTO entity_capabilities (entity_id, capability) VALUES (?, ?)`,
			identityID, cap)
		if err != nil {
			writeError(w, http.StatusInternalServerError, "failed to add capability")
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
		writeError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	resp := IdentityResponse{
		ID:           identityID,
		OrgID:        1,
		Identifier:   req.Identifier,
		DisplayName:  req.DisplayName,
		State:        "active",
		Profile:      req.Profile,
		Capabilities: req.Capabilities,
		CreatedAt:    now,
		UpdatedAt:    now,
	}
	writeJSON(w, http.StatusCreated, resp)
}

func (a *API) getIdentity(w http.ResponseWriter, r *http.Request) {
	identityID, err := parseID(r, "id")
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid id")
		return
	}

	resp, err := a.loadIdentity(r, identityID)
	if err != nil {
		writeError(w, http.StatusNotFound, "identity not found")
		return
	}

	writeJSON(w, http.StatusOK, resp)
}

func (a *API) listIdentities(w http.ResponseWriter, r *http.Request) {
	limit := 50
	if l := r.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 200 {
			limit = n
		}
	}
	var cursor int64
	if c := r.URL.Query().Get("cursor"); c != "" {
		cursor, _ = strconv.ParseInt(c, 10, 64)
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
		if oid, e := strconv.ParseInt(orgIDFilter, 10, 64); e == nil {
			where = append(where, `i.org_id = ?`)
			args = append(args, oid)
		}
	}
	where = append(where, `i.id > ?`)
	args = append(args, cursor)

	query := baseSelect + ` WHERE ` + strings.Join(where, " AND ") + ` ORDER BY i.id ASC LIMIT ?`
	args = append(args, limit+1)
	rows, err = a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
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
		nextCursor = strconv.FormatInt(identities[len(identities)-1].ID, 10)
	}

	writeJSON(w, http.StatusOK, ListResponse{
		Items:      identities,
		NextCursor: nextCursor,
	})
}

func (a *API) updateIdentity(w http.ResponseWriter, r *http.Request) {
	identityID, err := parseID(r, "id")
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var req IdentityRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "database error")
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
		writeError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		writeError(w, http.StatusNotFound, "identity not found")
		return
	}

	emitEvent(r.Context(), tx, "identity.updated", identityID, identityID, "identity", map[string]any{
		"state": req.State,
	})

	if err := tx.Commit(); err != nil {
		writeError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	resp, _ := a.loadIdentity(r, identityID)
	writeJSON(w, http.StatusOK, resp)
}

func (a *API) deleteIdentity(w http.ResponseWriter, r *http.Request) {
	identityID, err := parseID(r, "id")
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid id")
		return
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	// Clean up promoted indexes.
	_, _ = tx.ExecContext(r.Context(), `DELETE FROM entity_indexes WHERE entity_type = 'identity' AND entity_id = ?`, identityID)

	result, err := tx.ExecContext(r.Context(), `DELETE FROM entities WHERE id = ?`, identityID)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		writeError(w, http.StatusNotFound, "identity not found")
		return
	}

	emitEvent(r.Context(), tx, "identity.deleted", identityID, identityID, "identity", nil)

	if err := tx.Commit(); err != nil {
		writeError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()
	w.WriteHeader(http.StatusNoContent)
}

// --- Schema handlers ---

type SchemaRequest struct {
	ID      string `json:"id"`
	Type    string `json:"type"`
	OrgID   int64  `json:"org_id,omitempty"`
	Schema  any    `json:"schema"`  // JSON Schema document
	Message string `json:"message"` // Version commit message
}

type SchemaResponse struct {
	ID        string `json:"id"`
	Type      string `json:"type"`
	OrgID     int64  `json:"org_id"`
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
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Type == "" || req.Schema == nil {
		writeError(w, http.StatusBadRequest, "type and schema are required")
		return
	}
	if req.OrgID == 0 {
		req.OrgID = 1
	}

	schemaJSON, err := json.Marshal(req.Schema)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid schema")
		return
	}

	// Validate x-auth-methods keys.
	if validationErr := validateSchemaAnnotations(schemaJSON); validationErr != "" {
		writeError(w, http.StatusBadRequest, validationErr)
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
		writeError(w, http.StatusInternalServerError, "failed to save schema: "+err.Error())
		return
	}

	writeJSON(w, http.StatusCreated, SchemaResponse{
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
		writeError(w, http.StatusInternalServerError, "query failed")
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
		writeError(w, http.StatusInternalServerError, "rows error")
		return
	}

	writeJSON(w, http.StatusOK, ListResponse{Items: schemas})
}

func (a *API) getSchema(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	var s SchemaResponse
	var schemaStr string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, type, org_id, schema, version, COALESCE(is_default, false), COALESCE(message,''), COALESCE(created_by,''), created_at FROM schemas WHERE id = ?`, schemaID,
	).Scan(&s.ID, &s.Type, &s.OrgID, &schemaStr, &s.Version, &s.IsDefault, &s.Message, &s.CreatedBy, &s.CreatedAt)
	if err != nil {
		writeError(w, http.StatusNotFound, "schema not found")
		return
	}
	json.Unmarshal([]byte(schemaStr), &s.Schema)

	writeJSON(w, http.StatusOK, s)
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
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Schema == nil {
		writeError(w, http.StatusBadRequest, "schema is required")
		return
	}

	// Load existing schema to get type+org.
	var schemaType string
	var orgID int64
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT type, org_id FROM schemas WHERE id = ?`, schemaID,
	).Scan(&schemaType, &orgID)
	if err != nil {
		writeError(w, http.StatusNotFound, "schema not found")
		return
	}

	schemaJSON, err := json.Marshal(req.Schema)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid schema")
		return
	}

	// Validate x-auth-methods keys.
	if validationErr := validateSchemaAnnotations(schemaJSON); validationErr != "" {
		writeError(w, http.StatusBadRequest, validationErr)
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
		writeError(w, http.StatusInternalServerError, "failed to create version: "+err.Error())
		return
	}

	a.EmitAuthEvent(r.Context(), "schema.version_created", 0, map[string]any{
		"schema_id":  newID,
		"type":       schemaType,
		"version":    newVersion,
		"message":    req.Message,
		"from_version": schemaID,
	})

	writeJSON(w, http.StatusCreated, SchemaResponse{
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
	var orgID int64
	var version int
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT type, org_id, version FROM schemas WHERE id = ?`, schemaID,
	).Scan(&schemaType, &orgID, &version)
	if err != nil {
		writeError(w, http.StatusNotFound, "schema not found")
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

	a.EmitAuthEvent(r.Context(), "schema.promoted", 0, map[string]any{
		"schema_id": schemaID, "type": schemaType, "version": version,
	})

	writeJSON(w, http.StatusOK, map[string]any{
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
		writeError(w, http.StatusBadRequest, "compare query parameter required")
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
		writeError(w, http.StatusNotFound, "schema not found: "+schemaID)
		return
	}

	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT schema, version, COALESCE(message,'') FROM schemas WHERE id = ?`, compareID,
	).Scan(&rightStr, &rightVersion, &rightMsg)
	if err != nil {
		writeError(w, http.StatusNotFound, "schema not found: "+compareID)
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

	writeJSON(w, http.StatusOK, map[string]any{
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
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.EntityID == "" {
		writeError(w, http.StatusBadRequest, "entity_id is required")
		return
	}

	// Load the draft schema.
	var draftSchemaStr string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT schema FROM schemas WHERE id = ?`, schemaID,
	).Scan(&draftSchemaStr)
	if err != nil {
		writeError(w, http.StatusNotFound, "schema not found")
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
		writeError(w, http.StatusNotFound, "entity not found")
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

	writeJSON(w, http.StatusOK, map[string]any{
		"entity":         identifier,
		"current_claims": currentClaims,
		"draft_claims":   draftClaims,
		"changes":        claimChanges,
	})
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

	writeJSON(w, http.StatusOK, map[string]any{"count": count})
}

func (a *API) loadIdentity(r *http.Request, identityID int64) (IdentityResponse, error) {
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

func (a *API) loadCapabilities(r *http.Request, identityID int64) []string {
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

func parseID(r *http.Request, name string) (int64, error) {
	return strconv.ParseInt(r.PathValue(name), 10, 64)
}

func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(v)
}

func writeError(w http.ResponseWriter, status int, msg string) {
	writeJSON(w, status, ErrorResponse{Error: msg, Code: status})
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
		writeJSON(w, http.StatusOK, map[string]any{"results": []any{}, "query": ""})
		return
	}

	limit := 10
	if l := r.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 50 {
			limit = n
		}
	}

	pattern := "%" + q + "%"
	var results []SearchResult

	// Search entities by identifier, display_name
	idRows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, identifier, display_name, state FROM entities
		 WHERE identifier LIKE ? OR display_name LIKE ?
		 ORDER BY id DESC LIMIT ?`,
		pattern, pattern, limit)
	if err == nil {
		defer idRows.Close()
		for idRows.Next() {
			var id int64
			var ident, displayName, state string
			var dn sql.NullString
			if err := idRows.Scan(&id, &ident, &dn, &state); err != nil {
				continue
			}
			if dn.Valid {
				displayName = dn.String
			}
			results = append(results, SearchResult{
				ResourceType: "identity",
				ID:           strconv.FormatInt(id, 10),
				Title:        ident,
				Subtitle:     displayName + " · " + state,
				Link:         fmt.Sprintf("/console/identities/%d", id),
			})
		}
		if err := idRows.Err(); err == nil {
			idRows.Close()
		}
	}

	// Search entity_indexes (profile fields like email, display_name, etc.)
	eiRows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT DISTINCT ei.entity_id, i.identifier, i.display_name, ei.field, ei.value
		 FROM entity_indexes ei
		 JOIN entities e ON ei.entity_id = i.id AND ei.entity_type = 'identity'
		 WHERE ei.value LIKE ?
		 LIMIT ?`,
		pattern, limit)
	if err == nil {
		defer eiRows.Close()
		seen := map[string]bool{}
		for eiRows.Next() {
			var entityID int64
			var ident, field, value string
			var dn sql.NullString
			if err := eiRows.Scan(&entityID, &ident, &dn, &field, &value); err != nil {
				continue
			}
			idStr := strconv.FormatInt(entityID, 10)
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
				Link:         fmt.Sprintf("/console/identities/%d", entityID),
			})
		}
		if err := eiRows.Err(); err == nil {
			eiRows.Close()
		}
	}

	// Search schemas by type or id
	schRows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, type FROM schemas WHERE id LIKE ? OR type LIKE ? LIMIT ?`,
		pattern, pattern, limit)
	if err == nil {
		defer schRows.Close()
		for schRows.Next() {
			var schemaID, schemaType string
			if err := schRows.Scan(&schemaID, &schemaType); err != nil {
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
		if err := schRows.Err(); err == nil {
			schRows.Close()
		}
	}

	// Search events by event_type
	evtRows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, event_type, created_at FROM events WHERE event_type LIKE ? ORDER BY id DESC LIMIT ?`,
		pattern, limit)
	if err == nil {
		defer evtRows.Close()
		for evtRows.Next() {
			var evtID int64
			var evtType, createdAt string
			if err := evtRows.Scan(&evtID, &evtType, &createdAt); err != nil {
				continue
			}
			results = append(results, SearchResult{
				ResourceType: "event",
				ID:           strconv.FormatInt(evtID, 10),
				Title:        evtType,
				Subtitle:     createdAt,
				Link:         "/console/events",
			})
		}
		if err := evtRows.Err(); err == nil {
			evtRows.Close()
		}
	}

	// Search providers by name
	provRows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, name, protocol, template FROM providers
		 WHERE name LIKE ? OR template LIKE ?
		 ORDER BY name LIMIT ?`,
		pattern, pattern, limit)
	if err == nil {
		defer provRows.Close()
		for provRows.Next() {
			var provID, name, protocol, tmpl string
			if err := provRows.Scan(&provID, &name, &protocol, &tmpl); err != nil {
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
		if err := provRows.Err(); err == nil {
			provRows.Close()
		}
	}

	// Deduplicate entities (may appear from both direct + index search)
	seen := map[string]bool{}
	var deduped []SearchResult
	for _, r := range results {
		key := r.ResourceType + ":" + r.ID
		if !seen[key] {
			seen[key] = true
			deduped = append(deduped, r)
		}
	}

	writeJSON(w, http.StatusOK, map[string]any{
		"results": deduped,
		"query":   q,
		"count":   len(deduped),
	})
}

func promoteIndexes(ctx context.Context, tx *sql.Tx, entityType string, entityID int64, dataJSON string) {
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

func emitEvent(ctx context.Context, tx *sql.Tx, eventType string, actorID, aggregateID int64, aggregateType string, payload map[string]any) {
	eventID, err := id.New()
	if err != nil {
		return
	}
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	tx.ExecContext(ctx,
		`INSERT INTO events (id, event_type, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, session_id, created_at)
		 VALUES (?, ?, 0, ?, '', ?, ?, ?, '{}', '', 0, datetime('now'))`,
		eventID, eventType, actorID, aggregateID, aggregateType, payloadJSON)
}

func generateToken() (raw string, hash string, err error) {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return "", "", fmt.Errorf("generate token: %w", err)
	}
	raw = hex.EncodeToString(b)
	h := sha256.Sum256([]byte(raw))
	hash = hex.EncodeToString(h[:])
	return raw, hash, nil
}

// EmitAuthEvent is an exported helper for the UI to emit auth-related events.
func (a *API) EmitAuthEvent(ctx context.Context, eventType string, actorID int64, payload map[string]any) {
	eventID, err := id.New()
	if err != nil {
		return
	}
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	a.db.SQL().ExecContext(ctx,
		`INSERT INTO events (id, event_type, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, session_id, created_at)
		 VALUES (?, ?, 0, ?, '', ?, 'auth', ?, '{}', '', 0, datetime('now'))`,
		eventID, eventType, actorID, actorID, payloadJSON)
	a.bus.Signal()
}

// emitEventSimple is a package-level helper for event emission outside transactions.
func emitEventSimple(ctx context.Context, db interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}, eventType string, actorID int64, aggregateID, aggregateType string, payload map[string]any) {
	eventIDVal, err := id.New()
	if err != nil {
		return
	}
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	db.ExecContext(ctx, //nolint:errcheck // fire-and-forget audit event
		`INSERT INTO events (id, event_type, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, session_id, created_at)
		 VALUES (?, ?, 0, ?, '', ?, ?, ?, '{}', '', 0, datetime('now'))`,
		eventIDVal, eventType, actorID, aggregateID, aggregateType, payloadJSON)
}

// GetIdentityByID is an exported helper for the UI to get an identity (for edit form).
func (a *API) GetIdentityByID(r *http.Request, identityID int64) (IdentityResponse, error) {
	return a.loadIdentity(r, identityID)
}

// CreateIdentityInternal is an exported helper for the UI to create an identity.
func (a *API) CreateIdentityInternal(r *http.Request, req IdentityRequest) (IdentityResponse, error) {
	identityID, err := id.New()
	if err != nil {
		return IdentityResponse{}, fmt.Errorf("generate id: %w", err)
	}

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
		ID: identityID, OrgID: 1, Identifier: req.Identifier, DisplayName: req.DisplayName,
		State: "active", Profile: req.Profile, Capabilities: req.Capabilities,
		CreatedAt: now, UpdatedAt: now,
	}, nil
}

// UpdateIdentityInternal is an exported helper for the UI to update an identity.
func (a *API) UpdateIdentityInternal(r *http.Request, identityID int64, req IdentityRequest) (IdentityResponse, error) {
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
		return IdentityResponse{}, fmt.Errorf("identity %d not found", identityID)
	}

	emitEvent(r.Context(), tx, "identity.updated", identityID, identityID, "identity", nil)
	if err := tx.Commit(); err != nil {
		return IdentityResponse{}, fmt.Errorf("commit: %w", err)
	}
	a.bus.Signal()

	return a.loadIdentity(r, identityID)
}

// DeleteIdentityInternal is an exported helper for the UI to delete an identity.
func (a *API) DeleteIdentityInternal(r *http.Request, identityID int64) error {
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
		return fmt.Errorf("identity %d not found", identityID)
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
