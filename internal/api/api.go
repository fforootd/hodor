// Package api provides REST+JSON handlers for the ZITADEL v2 API.
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
	mux.HandleFunc("POST /v1/identities", a.createIdentity)
	mux.HandleFunc("GET /v1/identities", a.listIdentities)
	mux.HandleFunc("GET /v1/identities/{id}", a.getIdentity)
	mux.HandleFunc("PATCH /v1/identities/{id}", a.updateIdentity)
	mux.HandleFunc("DELETE /v1/identities/{id}", a.deleteIdentity)

	// Schema CRUD (write = admin-only, read = public)
	mux.HandleFunc("POST /v1/schemas", a.requireAdmin(a.createSchema))
	mux.HandleFunc("GET /v1/schemas", a.listSchemas)
	mux.HandleFunc("GET /v1/schemas/$meta", a.getMetaSchema)
	mux.HandleFunc("GET /v1/schemas/{id}", a.getSchema)
	mux.HandleFunc("PATCH /v1/schemas/{id}", a.requireAdmin(a.updateSchema))
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
		`INSERT INTO identities (id, org_id, identifier, display_name, state, profile, metadata, created_at, updated_at)
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
			`INSERT INTO identity_capabilities (identity_id, capability) VALUES (?, ?)`,
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

	var rows *sql.Rows
	var err error
	if schemaType != "" {
		rows, err = a.db.SQL().QueryContext(r.Context(),
			`SELECT i.id, i.org_id, i.identifier, i.display_name, i.state, i.profile, i.metadata, i.data, i.created_at, i.updated_at
			 FROM identities i
			 JOIN schemas s ON i.schema_id = s.id
			 WHERE s.type = ? AND i.id > ? ORDER BY i.id ASC LIMIT ?`,
			schemaType, cursor, limit+1)
	} else {
		rows, err = a.db.SQL().QueryContext(r.Context(),
			`SELECT id, org_id, identifier, display_name, state, profile, metadata, data, created_at, updated_at
			 FROM identities WHERE id > ? ORDER BY id ASC LIMIT ?`,
			cursor, limit+1)
	}
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

	query := "UPDATE identities SET " + strings.Join(setClauses, ", ") + " WHERE id = ?" //nolint:gosec // G202: setClauses are hardcoded column names, not user input.
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

	result, err := tx.ExecContext(r.Context(), `DELETE FROM identities WHERE id = ?`, identityID)
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
	ID     string `json:"id"`
	Type   string `json:"type"`
	OrgID  int64  `json:"org_id,omitempty"`
	Schema any    `json:"schema"` // JSON Schema document
}

type SchemaResponse struct {
	ID        string `json:"id"`
	Type      string `json:"type"`
	OrgID     int64  `json:"org_id"`
	Schema    any    `json:"schema"`
	Version   int    `json:"version"`
	IsDefault bool   `json:"is_default"`
	CreatedAt string `json:"created_at"`
}

func (a *API) createSchema(w http.ResponseWriter, r *http.Request) {
	var req SchemaRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.ID == "" || req.Type == "" || req.Schema == nil {
		writeError(w, http.StatusBadRequest, "id, type, and schema are required")
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

	// Check if there's already a default for this type+org.
	var existingDefault int
	a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM schemas WHERE type = ? AND org_id = ? AND is_default = true`,
		req.Type, req.OrgID).Scan(&existingDefault)
	isDefault := existingDefault == 0 // First schema of this type becomes default.

	_, err = a.db.SQL().ExecContext(r.Context(),
		`INSERT OR REPLACE INTO schemas (id, type, org_id, schema, version, is_default, created_at)
		 VALUES (?, ?, ?, ?, 1, ?, ?)`,
		req.ID, req.Type, req.OrgID, string(schemaJSON), isDefault, now)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to save schema")
		return
	}

	writeJSON(w, http.StatusCreated, SchemaResponse{
		ID:        req.ID,
		Type:      req.Type,
		OrgID:     req.OrgID,
		Schema:    req.Schema,
		Version:   1,
		IsDefault: isDefault,
		CreatedAt: now,
	})
}

func (a *API) listSchemas(w http.ResponseWriter, r *http.Request) {
	typeFilter := r.URL.Query().Get("type")

	query := `SELECT id, type, org_id, schema, version, COALESCE(is_default, false), created_at FROM schemas ORDER BY id`
	var args []any
	if typeFilter != "" {
		query = `SELECT id, type, org_id, schema, version, COALESCE(is_default, false), created_at FROM schemas WHERE type = ? ORDER BY id`
		args = []any{typeFilter}
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
		if err := rows.Scan(&s.ID, &s.Type, &s.OrgID, &schemaStr, &s.Version, &s.IsDefault, &s.CreatedAt); err != nil {
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
		`SELECT id, type, org_id, schema, version, COALESCE(is_default, false), created_at FROM schemas WHERE id = ?`, schemaID,
	).Scan(&s.ID, &s.Type, &s.OrgID, &schemaStr, &s.Version, &s.IsDefault, &s.CreatedAt)
	if err != nil {
		writeError(w, http.StatusNotFound, "schema not found")
		return
	}
	json.Unmarshal([]byte(schemaStr), &s.Schema)

	writeJSON(w, http.StatusOK, s)
}

func (a *API) updateSchema(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	var req struct {
		Schema    any  `json:"schema"`
		IsDefault *bool `json:"is_default,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	// Handle is_default toggle.
	if req.IsDefault != nil {
		if *req.IsDefault {
			// Unset previous default for this type+org.
			var schemaType string
			var orgID int64
			a.db.SQL().QueryRowContext(r.Context(),
				`SELECT type, org_id FROM schemas WHERE id = ?`, schemaID,
			).Scan(&schemaType, &orgID)
			if schemaType != "" {
				a.db.SQL().ExecContext(r.Context(),
					`UPDATE schemas SET is_default = false WHERE type = ? AND org_id = ? AND id != ?`,
					schemaType, orgID, schemaID)
			}
		}
		a.db.SQL().ExecContext(r.Context(),
			`UPDATE schemas SET is_default = ? WHERE id = ?`,
			*req.IsDefault, schemaID)
	}

	// Handle schema body update.
	if req.Schema != nil {
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

		result, err := a.db.SQL().ExecContext(r.Context(),
			`UPDATE schemas SET schema = ?, version = version + 1 WHERE id = ?`,
			string(schemaJSON), schemaID)
		if err != nil {
			writeError(w, http.StatusInternalServerError, "failed to update schema")
			return
		}
		rows, _ := result.RowsAffected()
		if rows == 0 {
			writeError(w, http.StatusNotFound, "schema not found")
			return
		}
	} else if req.IsDefault == nil {
		writeError(w, http.StatusBadRequest, "schema or is_default is required")
		return
	}

	// Return updated schema.
	var s SchemaResponse
	var updatedStr string
	a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, type, org_id, schema, version, COALESCE(is_default, false), created_at FROM schemas WHERE id = ?`, schemaID,
	).Scan(&s.ID, &s.Type, &s.OrgID, &updatedStr, &s.Version, &s.IsDefault, &s.CreatedAt)
	json.Unmarshal([]byte(updatedStr), &s.Schema)

	a.EmitAuthEvent(r.Context(), "schema.updated", 0, map[string]any{
		"schema_id": schemaID,
		"version":   s.Version,
	})

	writeJSON(w, http.StatusOK, s)
}

// getMetaSchema returns the canonical ZITADEL identity schema meta-schema.
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
		`SELECT COUNT(*) FROM identities WHERE schema_id = ?`, schemaID,
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
		 FROM identities WHERE id = ?`, identityID,
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
		`SELECT capability FROM identity_capabilities WHERE identity_id = ?`, identityID)
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
	ResourceType string `json:"resource_type"` // identity, schema, event, session
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

	// Search identities by identifier, display_name
	idRows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, identifier, display_name, state FROM identities
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
		 JOIN identities i ON ei.entity_id = i.id AND ei.entity_type = 'identity'
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

	// Deduplicate identities (may appear from both direct + index search)
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
		`INSERT INTO identities (id, org_id, identifier, display_name, state, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, 'active', ?, '{}', ?, ?)`,
		identityID, req.Identifier, req.DisplayName, profileJSON, now, now)
	if err != nil {
		return IdentityResponse{}, fmt.Errorf("insert: %w", err)
	}

	for _, cap := range req.Capabilities {
		tx.ExecContext(r.Context(),
			`INSERT INTO identity_capabilities (identity_id, capability) VALUES (?, ?)`,
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

	query := "UPDATE identities SET " + strings.Join(setClauses, ", ") + " WHERE id = ?" //nolint:gosec // G202: setClauses are hardcoded column names, not user input.
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
	result, err := tx.ExecContext(r.Context(), `DELETE FROM identities WHERE id = ?`, identityID)
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
