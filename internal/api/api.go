// Package api provides REST+JSON handlers for the Zitadel v2 API.
// Schema-driven resource families and schema CRUD are served as plain JSON endpoints.
// OpenAPI spec is dynamically generated from the schema registry.
package api

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/catalog"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/notify"
	"github.com/zitadel/zitadel/internal/risk"

	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// API holds the REST handlers and their dependencies.
type API struct {
	db       *database.DB
	bus      *eventbus.Bus
	cookies  *session.CookieConfig
	spec     *OpenAPIRegistry
	catalog  *catalog.Service
	risk     risk.Evaluator
	notifier *notify.Service
}

// New creates a new API handler.
func New(db *database.DB, bus *eventbus.Bus, cookies *session.CookieConfig) *API {
	var riskEvaluator risk.Evaluator
	if db != nil {
		riskEvaluator = risk.NewEvaluator(db.SQL())
	}

	return &API{
		db:      db,
		bus:     bus,
		cookies: cookies,
		spec:    &OpenAPIRegistry{},
		risk:    riskEvaluator,
	}
}

func (a *API) SetCatalogService(svc *catalog.Service) {
	a.catalog = svc
}

func (a *API) SetNotificationService(svc *notify.Service) {
	a.notifier = svc
}

// RegisterRoutes mounts all REST API routes on the given mux.
// Authorization is handled by the FGA middleware (FGAGate) in the
// server middleware chain — individual routes no longer wrap requireAdmin.
func (a *API) RegisterRoutes(mux *http.ServeMux) {
	// Entity CRUD is exposed exclusively through schema-driven alias routes
	// (e.g. /v1/users, /v1/orgs, /v1/apps). See registerAliasRoutes().
	// The generic /v1/entities endpoint has been removed from the public API.

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
	a.RegisterNotificationRoutes(mux)

	// Mount generic Telemetry routes under /v1/telemetry
	a.RegisterTelemetryRoutes(mux)

	// Login Flow management (dedicated handlers with audience targeting)
	a.RegisterLoginFlowRoutes(mux)

	// Dynamic OpenAPI (generated from registry)
	a.registerOpenAPIOperations()
	mux.HandleFunc("GET /openapi.json", a.openAPISpecFromRegistry)

	// Well-known discovery
	mux.HandleFunc("GET /.well-known/zitadel-identity-schema", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/v1/schemas/$meta", http.StatusPermanentRedirect)
	})

	// Batch entity counts for sidebar badges
	mux.HandleFunc("GET /v1/counts", a.entityCounts)

	// Schema-driven entity routes (e.g. /v1/users, /v1/orgs, /v1/apps).
	// These are the primary public API — no generic /v1/entities.
	a.registerEntityRoutes(mux)

	// Groups & Projects — dedicated CRUD + member management (ADR-020)
	a.RegisterGroupRoutes(mux)
	a.RegisterProjectRoutes(mux)

	// Module management (enable/disable marketplace modules)
	a.RegisterModuleRoutes(mux)
}

// registerEntityRoutes reads the x-catalog from the meta schema and registers
// /v1/{path} routes for each entity type.
func (a *API) registerEntityRoutes(mux *http.ServeMux) {
	catalog, err := schema.Catalog()
	if err != nil {
		logging.Printf("[api] failed to load catalog: %v", err)
		return
	}

	// Canonical /v1/users family endpoint for human_user, service_user, and ai_agent.
	// Use schema_id on writes and schema_type for family filtering on reads.
	mux.HandleFunc("GET /v1/users", a.listUsers)
	mux.HandleFunc("POST /v1/users", a.createUser)
	mux.HandleFunc("GET /v1/users/{id}", a.getUser)
	mux.HandleFunc("PATCH /v1/users/{id}", a.updateUser)
	mux.HandleFunc("DELETE /v1/users/{id}", a.deleteUser)
	mux.HandleFunc("POST /v1/users/{id}/password", a.setEntityPassword)
	logging.Printf("[api] registered /v1/users (typed family)")

	// Dedicated Org CRUD routes.
	mux.HandleFunc("GET /v1/orgs", a.listOrgs)
	mux.HandleFunc("POST /v1/orgs", a.createOrg)
	mux.HandleFunc("GET /v1/orgs/{id}", a.getOrg)
	mux.HandleFunc("PATCH /v1/orgs/{id}", a.updateOrg)
	mux.HandleFunc("DELETE /v1/orgs/{id}", a.deleteOrg)
	mux.HandleFunc("GET /v1/orgs/{id}/members", a.listMembers("org"))
	mux.HandleFunc("POST /v1/orgs/{id}/members", a.addMember("org"))
	mux.HandleFunc("DELETE /v1/orgs/{id}/members/{userId}", a.removeMember("org"))
	logging.Printf("[api] registered /v1/orgs (full CRUD + members)")

	// Generic read-only routes for other dedicated resource tables.
	resourceTables := map[string]string{
		"action": "actions",
	}

	for typeName, tableName := range resourceTables {
		entry, ok := catalog[typeName]
		if !ok || entry.Path == "" {
			continue
		}
		prefix := "/v1/" + entry.Path
		tbl := tableName

		mux.HandleFunc("GET "+prefix, a.listResource(tbl))
		mux.HandleFunc("GET "+prefix+"/{id}", a.getResource(tbl))

		logging.Printf("[api] registered /v1/%s (table=%s)", entry.Path, tbl)
	}

	// Canonical /v1/apps family endpoint for application schemas.
	mux.HandleFunc("GET /v1/apps", a.listApps)
	mux.HandleFunc("POST /v1/apps", a.createApp)
	mux.HandleFunc("GET /v1/apps/{id}", a.getApp)
	mux.HandleFunc("PATCH /v1/apps/{id}", a.updateApp)
	mux.HandleFunc("DELETE /v1/apps/{id}", a.deleteApp)
	logging.Printf("[api] registered /v1/apps (typed family)")
}

// --- Org types ---

type OrgRequest struct {
	SchemaID string `json:"schema_id,omitempty"`
	Name     string `json:"name"`
	State    string `json:"state,omitempty"`
	Data     any    `json:"data,omitempty"`
	Metadata any    `json:"metadata,omitempty"`
}

type OrgResponse struct {
	ID         string `json:"id"`
	Name       string `json:"name"`
	State      string `json:"state"`
	SchemaID   string `json:"schema_id,omitempty"`
	SchemaType string `json:"schema_type,omitempty"`
	Metadata   any    `json:"metadata,omitempty"`
	Data       any    `json:"data,omitempty"`
	CreatedAt  string `json:"created_at"`
	UpdatedAt  string `json:"updated_at"`
}

// --- Org handlers ---

func (a *API) buildOrgResponse(ctx context.Context, row OrgResponse, metadataStr string) OrgResponse {
	metadata := decodeObjectString(metadataStr)
	row.SchemaType = "org"
	if row.SchemaID == "" {
		if rec, err := a.resolveResourceSchema(ctx, "org", ""); err == nil {
			row.SchemaID = rec.ID
		}
	}
	row.Data = orgCanonicalData(row.Name, metadata)
	if dataMap, ok := row.Data.(map[string]any); ok {
		row.Metadata = dataMap["metadata"]
	}
	return row
}

func (a *API) listOrgs(w http.ResponseWriter, r *http.Request) {
	limit, cursor := parsePagination(r)

	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, name, state, COALESCE(schema_id,''), COALESCE(metadata,'{}'), created_at, updated_at
		 FROM orgs
		 WHERE id > ?
		 ORDER BY id ASC
		 LIMIT ?`,
		cursor, limit+1,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var items []OrgResponse
	for rows.Next() {
		var row OrgResponse
		var metadataStr string
		if err := rows.Scan(&row.ID, &row.Name, &row.State, &row.SchemaID, &metadataStr, &row.CreatedAt, &row.UpdatedAt); err != nil {
			continue
		}
		items = append(items, a.buildOrgResponse(r.Context(), row, metadataStr))
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}

	var nextCursor string
	if len(items) > limit {
		items = items[:limit]
		nextCursor = items[len(items)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: items, NextCursor: nextCursor})
}

func (a *API) getOrg(w http.ResponseWriter, r *http.Request) {
	orgID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var row OrgResponse
	var metadataStr string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, name, state, COALESCE(schema_id,''), COALESCE(metadata,'{}'), created_at, updated_at
		 FROM orgs
		 WHERE id = ?`,
		orgID,
	).Scan(&row.ID, &row.Name, &row.State, &row.SchemaID, &metadataStr, &row.CreatedAt, &row.UpdatedAt)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "organization not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, a.buildOrgResponse(r.Context(), row, metadataStr))
}

func (a *API) createOrg(w http.ResponseWriter, r *http.Request) {
	var req OrgRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	schemaRec, err := a.resolveResourceSchema(r.Context(), "org", req.SchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(data) == 0 {
		data = orgCanonicalData(req.Name, map[string]any{"metadata": req.Metadata})
	}
	name := stringFromAny(data["display_name"])
	if name == "" {
		name = strings.TrimSpace(req.Name)
	}
	if name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "display_name is required")
		return
	}
	data["display_name"] = name
	if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	orgID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)
	storedMetadata := stripKeys(data, "display_name")
	metadataJSON := encodeObjectString(storedMetadata)

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO orgs (id, name, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, 'active', ?, ?, ?, ?)`,
		orgID, name, schemaRec.ID, metadataJSON, now, now,
	)
	if err != nil {
		logging.Printf("[createOrg] DB insert failed: %v", err)
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error":   "database error",
			"code":    409,
			"details": err.Error(),
		})
		return
	}

	emitEvent(r.Context(), tx, "org.created", orgID, orgID, "org", map[string]any{
		"name": name,
	})

	creatorID := creatorFromRequest(r)
	if svc := FGAService; svc != nil {
		fgaAsync("org created", func() { //nolint:contextcheck
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			if err := svc.OnOrgCreated(ctx, orgID, creatorID); err != nil {
				logging.Printf("[fga] org created: %v", err)
			}
		})
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, OrgResponse{
		ID:         orgID,
		Name:       name,
		State:      "active",
		SchemaID:   schemaRec.ID,
		SchemaType: schemaRec.Type,
		Metadata:   data["metadata"],
		Data:       data,
		CreatedAt:  now,
		UpdatedAt:  now,
	})
}

func (a *API) updateOrg(w http.ResponseWriter, r *http.Request) {
	orgID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var req OrgRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	var currentName, currentState, currentSchemaID, currentMetadata string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT name, state, COALESCE(schema_id,''), COALESCE(metadata,'{}')
		 FROM orgs
		 WHERE id = ?`,
		orgID,
	).Scan(&currentName, &currentState, &currentSchemaID, &currentMetadata)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "organization not found")
		return
	}

	schemaRec, err := a.resolveResourceSchema(r.Context(), "org", currentSchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(data) == 0 {
		existing := orgCanonicalData(currentName, decodeObjectString(currentMetadata))
		data = existing
		if strings.TrimSpace(req.Name) != "" {
			data["display_name"] = strings.TrimSpace(req.Name)
		}
		if req.Metadata != nil {
			data["metadata"] = req.Metadata
		}
	}
	name := stringFromAny(data["display_name"])
	if name == "" {
		name = currentName
		data["display_name"] = name
	}
	if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	nextState := currentState
	if strings.TrimSpace(req.State) != "" {
		nextState = strings.TrimSpace(req.State)
	}

	result, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE orgs
		 SET name = ?, state = ?, metadata = ?, updated_at = ?
		 WHERE id = ?`,
		name, nextState, encodeObjectString(stripKeys(data, "display_name")), timeNow(), orgID,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "organization not found")
		return
	}

	a.bus.Signal()
	a.getOrg(w, r)
}

func (a *API) deleteOrg(w http.ResponseWriter, r *http.Request) {
	orgID, err := parseID(r, "id")
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

	result, err := tx.ExecContext(r.Context(), `DELETE FROM orgs WHERE id = ?`, orgID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "organization not found")
		return
	}

	// FGA: clean up tuples (best-effort — orphan tuples on deleted resources are harmless).
	if svc := FGAService; svc != nil {
		if err := svc.OnResourceDeleted(r.Context(), orgID); err != nil {
			logging.Printf("[fga] warn: failed to delete org tuples (will be cleaned by reconciler): %v", err)
		}
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()
	w.WriteHeader(http.StatusNoContent)
}

// ── Org member management ──

// --- App types ---

type AppRequest struct {
	SchemaID               string   `json:"schema_id,omitempty"`
	Name                   string   `json:"name"`
	Description            string   `json:"description,omitempty"`
	AppType                string   `json:"app_type,omitempty"`
	ClientID               string   `json:"client_id,omitempty"`
	ClientSecret           string   `json:"client_secret,omitempty"`
	RedirectURIs           []string `json:"redirect_uris,omitempty"`
	PostLogoutRedirectURIs []string `json:"post_logout_redirect_uris,omitempty"`
	GrantTypes             []string `json:"grant_types,omitempty"`
	ResponseTypes          []string `json:"response_types,omitempty"`
	LogoURI                string   `json:"logo_uri,omitempty"`
	State                  string   `json:"state,omitempty"`
	Data                   any      `json:"data,omitempty"`
	Metadata               any      `json:"metadata,omitempty"`
}

type AppResponse struct {
	ID                     string   `json:"id"`
	OrgID                  string   `json:"org_id"`
	Name                   string   `json:"name"`
	Description            string   `json:"description,omitempty"`
	AppType                string   `json:"app_type"`
	ClientID               string   `json:"client_id"`
	RedirectURIs           []string `json:"redirect_uris,omitempty"`
	PostLogoutRedirectURIs []string `json:"post_logout_redirect_uris,omitempty"`
	GrantTypes             []string `json:"grant_types,omitempty"`
	ResponseTypes          []string `json:"response_types,omitempty"`
	LogoURI                string   `json:"logo_uri,omitempty"`
	State                  string   `json:"state"`
	SchemaID               string   `json:"schema_id,omitempty"`
	SchemaType             string   `json:"schema_type,omitempty"`
	Metadata               any      `json:"metadata,omitempty"`
	Data                   any      `json:"data,omitempty"`
	CreatedAt              string   `json:"created_at"`
	UpdatedAt              string   `json:"updated_at"`
}

// --- App handlers ---

func (a *API) buildAppResponse(ctx context.Context, row AppResponse, metadataStr string) AppResponse {
	metadata := decodeObjectString(metadataStr)
	if row.SchemaID != "" && row.SchemaType != "" {
		// The family list endpoint already resolved the schema context.
	} else if rec, err := a.resolveResourceSchema(ctx, "app", row.SchemaID); err == nil {
		row.SchemaID = rec.ID
		row.SchemaType = rec.Type
	}

	row.Data = appCanonicalData(
		row.Name,
		row.Description,
		row.AppType,
		row.RedirectURIs,
		row.PostLogoutRedirectURIs,
		row.GrantTypes,
		row.ResponseTypes,
		row.LogoURI,
		metadata,
	)
	if dataMap, ok := row.Data.(map[string]any); ok {
		if row.Description == "" {
			row.Description = stringFromAny(dataMap["description"])
		}
		if len(row.PostLogoutRedirectURIs) == 0 {
			row.PostLogoutRedirectURIs = stringSliceFromAny(dataMap["post_logout_redirect_uris"])
		}
		if row.LogoURI == "" {
			row.LogoURI = stringFromAny(dataMap["logo_uri"])
		}
		row.Metadata = dataMap["metadata"]
	}
	return row
}

func (a *API) listApps(w http.ResponseWriter, r *http.Request) {
	limit, cursor := parsePagination(r)
	orgID := r.URL.Query().Get("org_id")
	state := r.URL.Query().Get("state")
	schemaType := r.URL.Query().Get("schema_type")

	var where []string
	var args []any
	where = append(where, "a.id > ?")
	args = append(args, cursor)
	if orgID != "" {
		where = append(where, "a.org_id = ?")
		args = append(args, orgID)
	}
	if state != "" {
		where = append(where, "a.state = ?")
		args = append(args, state)
	}
	if schemaType != "" {
		where = append(where, "s.type = ?")
		args = append(args, schemaType)
	}

	query := `SELECT a.id, a.org_id, a.name, a.app_type, a.client_id,
	                 COALESCE(a.redirect_uris,'[]'), COALESCE(a.grant_types,'[]'), COALESCE(a.response_types,'[]'),
	                 a.state, COALESCE(a.schema_id,''), COALESCE(s.type,''), COALESCE(a.metadata,'{}'), a.created_at, a.updated_at
	          FROM apps a
	          LEFT JOIN schemas s ON a.schema_id = s.id
	          WHERE ` + strings.Join(where, " AND ") + `
	          ORDER BY a.id ASC
	          LIMIT ?`
	args = append(args, limit+1)

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var items []AppResponse
	for rows.Next() {
		var row AppResponse
		var redirectURIs, grantTypes, responseTypes, metadataStr string
		if err := rows.Scan(
			&row.ID,
			&row.OrgID,
			&row.Name,
			&row.AppType,
			&row.ClientID,
			&redirectURIs,
			&grantTypes,
			&responseTypes,
			&row.State,
			&row.SchemaID,
			&row.SchemaType,
			&metadataStr,
			&row.CreatedAt,
			&row.UpdatedAt,
		); err != nil {
			continue
		}
		row.RedirectURIs = stringSliceFromAny(redirectURIs)
		row.GrantTypes = stringSliceFromAny(grantTypes)
		row.ResponseTypes = stringSliceFromAny(responseTypes)
		items = append(items, a.buildAppResponse(r.Context(), row, metadataStr))
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}

	var nextCursor string
	if len(items) > limit {
		items = items[:limit]
		nextCursor = items[len(items)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: items, NextCursor: nextCursor})
}

func (a *API) getApp(w http.ResponseWriter, r *http.Request) {
	appID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var row AppResponse
	var redirectURIs, grantTypes, responseTypes, metadataStr string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, org_id, name, app_type, client_id,
		        COALESCE(redirect_uris,'[]'), COALESCE(grant_types,'[]'), COALESCE(response_types,'[]'),
		        state, COALESCE(schema_id,''), COALESCE(metadata,'{}'), created_at, updated_at
		 FROM apps
		 WHERE id = ?`,
		appID,
	).Scan(
		&row.ID,
		&row.OrgID,
		&row.Name,
		&row.AppType,
		&row.ClientID,
		&redirectURIs,
		&grantTypes,
		&responseTypes,
		&row.State,
		&row.SchemaID,
		&metadataStr,
		&row.CreatedAt,
		&row.UpdatedAt,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "application not found")
		return
	}

	row.RedirectURIs = stringSliceFromAny(redirectURIs)
	row.GrantTypes = stringSliceFromAny(grantTypes)
	row.ResponseTypes = stringSliceFromAny(responseTypes)

	httputil.WriteJSON(w, http.StatusOK, a.buildAppResponse(r.Context(), row, metadataStr))
}

func (a *API) createApp(w http.ResponseWriter, r *http.Request) {
	var req AppRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	schemaRec, err := a.resolveResourceSchema(r.Context(), "app", req.SchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(data) == 0 {
		data = appCanonicalData(
			req.Name,
			req.Description,
			req.AppType,
			req.RedirectURIs,
			req.PostLogoutRedirectURIs,
			req.GrantTypes,
			req.ResponseTypes,
			req.LogoURI,
			map[string]any{"metadata": req.Metadata},
		)
	}
	data, err = schema.ObjectMap(data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	name := stringFromAny(data["client_name"])
	if name == "" {
		name = strings.TrimSpace(req.Name)
		data["client_name"] = name
	}
	if name == "" {
		httputil.WriteError(w, http.StatusBadRequest, "client_name is required")
		return
	}

	appType := normalizeAppType(stringFromAny(data["app_type"]))
	if appType == "" {
		appType = normalizeAppType(req.AppType)
	}
	if appType == "" {
		appType = "web"
		data["app_type"] = appType
	}

	if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	appID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)

	clientID := req.ClientID
	if clientID == "" {
		clientID = id.New()
	}

	clientSecret := ""
	if req.ClientSecret != "" {
		clientSecret, err = auth.HashSecret(req.ClientSecret)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "hash client secret failed")
			return
		}
	}

	redirectList := stringSliceFromAny(data["redirect_uris"])
	grantList := stringSliceFromAny(data["grant_types"])
	responseList := stringSliceFromAny(data["response_types"])
	redirectBytes, err := json.Marshal(redirectList)
	if err != nil {
		redirectBytes = []byte("[]")
	}
	grantBytes, err := json.Marshal(grantList)
	if err != nil || len(grantList) == 0 {
		grantBytes = []byte(`["authorization_code"]`)
		grantList = []string{"authorization_code"}
	}
	responseBytes, err := json.Marshal(responseList)
	if err != nil || len(responseList) == 0 {
		responseBytes = []byte(`["code"]`)
		responseList = []string{"code"}
	}

	description := stringFromAny(data["description"])
	postLogoutRedirectURIs := stringSliceFromAny(data["post_logout_redirect_uris"])
	logoURI := stringFromAny(data["logo_uri"])
	metadataJSON := encodeObjectString(stripKeys(data,
		"client_name",
		"app_type",
		"redirect_uris",
		"grant_types",
		"response_types",
	))

	orgID := r.Header.Get("X-Org-Id")

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO apps (id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?)`,
		appID, orgID, name, appType, clientID, clientSecret,
		string(redirectBytes), string(grantBytes), string(responseBytes), schemaRec.ID, metadataJSON, now, now,
	)
	if err != nil {
		logging.Printf("[createApp] DB insert failed: %v", err)
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error":   "database error",
			"code":    409,
			"details": err.Error(),
		})
		return
	}

	emitEvent(r.Context(), tx, "app.created", appID, appID, "app", map[string]any{
		"name":      name,
		"client_id": clientID,
	})

	creatorID := creatorFromRequest(r)
	if svc := FGAService; svc != nil {
		fgaAsync("app created", func() { //nolint:contextcheck
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			if err := svc.OnAppCreated(ctx, appID, creatorID, orgID); err != nil {
				logging.Printf("[fga] app created: %v", err)
			}
		})
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, AppResponse{
		ID:                     appID,
		OrgID:                  orgID,
		Name:                   name,
		Description:            description,
		AppType:                appType,
		ClientID:               clientID,
		RedirectURIs:           redirectList,
		PostLogoutRedirectURIs: postLogoutRedirectURIs,
		GrantTypes:             grantList,
		ResponseTypes:          responseList,
		LogoURI:                logoURI,
		State:                  "active",
		SchemaID:               schemaRec.ID,
		SchemaType:             schemaRec.Type,
		Metadata:               data["metadata"],
		Data:                   data,
		CreatedAt:              now,
		UpdatedAt:              now,
	})
}

func (a *API) updateApp(w http.ResponseWriter, r *http.Request) {
	appID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var req AppRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	var current AppResponse
	var redirectURIs, grantTypes, responseTypes, metadataStr, currentClientSecret string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, org_id, name, app_type, client_id,
		        COALESCE(redirect_uris,'[]'), COALESCE(grant_types,'[]'), COALESCE(response_types,'[]'),
		        state, COALESCE(schema_id,''), COALESCE(metadata,'{}'), COALESCE(client_secret,''), created_at, updated_at
		 FROM apps
		 WHERE id = ?`,
		appID,
	).Scan(
		&current.ID,
		&current.OrgID,
		&current.Name,
		&current.AppType,
		&current.ClientID,
		&redirectURIs,
		&grantTypes,
		&responseTypes,
		&current.State,
		&current.SchemaID,
		&metadataStr,
		&currentClientSecret,
		&current.CreatedAt,
		&current.UpdatedAt,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "application not found")
		return
	}
	current.RedirectURIs = stringSliceFromAny(redirectURIs)
	current.GrantTypes = stringSliceFromAny(grantTypes)
	current.ResponseTypes = stringSliceFromAny(responseTypes)
	current = a.buildAppResponse(r.Context(), current, metadataStr)

	schemaRec, err := a.resolveResourceSchema(r.Context(), "app", current.SchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(data) == 0 {
		data = appCanonicalData(
			current.Name,
			current.Description,
			current.AppType,
			current.RedirectURIs,
			current.PostLogoutRedirectURIs,
			current.GrantTypes,
			current.ResponseTypes,
			current.LogoURI,
			decodeObjectString(metadataStr),
		)
		if strings.TrimSpace(req.Name) != "" {
			data["client_name"] = strings.TrimSpace(req.Name)
		}
		if strings.TrimSpace(req.Description) != "" {
			data["description"] = strings.TrimSpace(req.Description)
		}
		if strings.TrimSpace(req.AppType) != "" {
			data["app_type"] = normalizeAppType(req.AppType)
		}
		if len(req.RedirectURIs) > 0 {
			data["redirect_uris"] = req.RedirectURIs
		}
		if len(req.PostLogoutRedirectURIs) > 0 {
			data["post_logout_redirect_uris"] = req.PostLogoutRedirectURIs
		}
		if len(req.GrantTypes) > 0 {
			data["grant_types"] = req.GrantTypes
		}
		if len(req.ResponseTypes) > 0 {
			data["response_types"] = req.ResponseTypes
		}
		if strings.TrimSpace(req.LogoURI) != "" {
			data["logo_uri"] = strings.TrimSpace(req.LogoURI)
		}
		if req.Metadata != nil {
			data["metadata"] = req.Metadata
		}
	}
	data, err = schema.ObjectMap(data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}
	if stringFromAny(data["client_name"]) == "" {
		data["client_name"] = current.Name
	}
	if stringFromAny(data["app_type"]) == "" {
		data["app_type"] = current.AppType
	}
	if err := schema.ValidateData(schemaRec.Schema, data); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
		return
	}

	nextState := current.State
	if strings.TrimSpace(req.State) != "" {
		nextState = strings.TrimSpace(req.State)
	}

	redirectJSON, err := json.Marshal(stringSliceFromAny(data["redirect_uris"]))
	if err != nil {
		redirectJSON = []byte("[]")
	}
	grantJSON, err := json.Marshal(stringSliceFromAny(data["grant_types"]))
	if err != nil {
		grantJSON = []byte(`["authorization_code"]`)
	}
	responseJSON, err := json.Marshal(stringSliceFromAny(data["response_types"]))
	if err != nil {
		responseJSON = []byte(`["code"]`)
	}

	nextClientSecret := currentClientSecret
	if req.ClientSecret != "" {
		nextClientSecret, err = auth.HashSecret(req.ClientSecret)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "hash client secret failed")
			return
		}
	}

	result, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE apps
		 SET name = ?, app_type = ?, client_secret = ?, redirect_uris = ?, grant_types = ?, response_types = ?, state = ?, metadata = ?, updated_at = ?
		 WHERE id = ?`,
		stringFromAny(data["client_name"]),
		normalizeAppType(stringFromAny(data["app_type"])),
		nextClientSecret,
		string(redirectJSON),
		string(grantJSON),
		string(responseJSON),
		nextState,
		encodeObjectString(stripKeys(data,
			"client_name",
			"app_type",
			"redirect_uris",
			"grant_types",
			"response_types",
		)),
		timeNow(),
		appID,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "application not found")
		return
	}

	a.bus.Signal()

	a.getApp(w, r)
}

func (a *API) deleteApp(w http.ResponseWriter, r *http.Request) {
	appID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	result, err := a.db.SQL().ExecContext(r.Context(), `DELETE FROM apps WHERE id = ?`, appID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "application not found")
		return
	}

	a.bus.Signal()

	w.WriteHeader(http.StatusNoContent)
}

// --- User types ---

type UserRequest struct {
	SchemaID     string   `json:"schema_id,omitempty"`
	Identifier   string   `json:"identifier"`
	DisplayName  string   `json:"display_name,omitempty"`
	Data         any      `json:"data,omitempty"`
	Profile      any      `json:"profile,omitempty"`
	Metadata     any      `json:"metadata,omitempty"`
	State        string   `json:"state,omitempty"`
	Capabilities []string `json:"capabilities,omitempty"`
}

type UserResponse struct {
	ID           string             `json:"id"`
	OrgID        string             `json:"org_id"`
	Identifier   string             `json:"identifier"`
	DisplayName  string             `json:"display_name,omitempty"`
	UserType     string             `json:"user_type"`
	State        string             `json:"state"`
	SchemaID     string             `json:"schema_id,omitempty"`
	SchemaType   string             `json:"schema_type,omitempty"`
	Profile      any                `json:"profile,omitempty"`
	Metadata     any                `json:"metadata,omitempty"`
	Data         any                `json:"data,omitempty"`
	Capabilities []string           `json:"capabilities,omitempty"`
	Orgs         []OrgMembershipDTO `json:"orgs,omitempty"`
	CreatedAt    string             `json:"created_at"`
	UpdatedAt    string             `json:"updated_at"`
}

type OrgMembershipDTO struct {
	OrgID   string `json:"org_id"`
	OrgName string `json:"org_name"`
	Role    string `json:"role"`
	AddedAt string `json:"added_at"`
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

func (a *API) createUser(w http.ResponseWriter, r *http.Request) {
	var req UserRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	schemaRec, err := schema.ResolveUserSchemaForWrite(r.Context(), a.db.SQL(), req.SchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
		return
	}

	identifier := strings.TrimSpace(req.Identifier)
	displayName := strings.TrimSpace(req.DisplayName)
	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
		return
	}

	var write *validatedUserWrite
	if len(data) > 0 {
		if identifier == "" {
			identifier = identifierFromUserData(schemaRec.Schema, data, "")
		}
		if identifier == "" {
			httputil.WriteError(w, http.StatusBadRequest, "identifier is required")
			return
		}
		if displayName == "" {
			displayName = stringFromAny(data["display_name"])
		}
		if displayName == "" {
			displayName = identifier
		}
		write, err = validatedUserWriteFromData(schemaRec, identifier, displayName, data)
	} else {
		if identifier == "" {
			httputil.WriteError(w, http.StatusBadRequest, "identifier is required")
			return
		}
		if displayName == "" {
			displayName = identifier
		}
		write, err = a.prepareUserWrite(r.Context(), req.SchemaID, identifier, displayName, req.Metadata, req.Profile)
	}
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
		return
	}

	userID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "database error")
		return
	}
	defer tx.Rollback()

	userType := "human"
	if write.Schema.Type == "service_user" || write.Schema.Type == "ai_agent" {
		userType = write.Schema.Type
	}

	orgID := r.Header.Get("X-Org-Id")
	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?)`,
		userID, orgID, identifier, displayName, userType, write.Schema.ID, write.MetadataJSON, now, now,
	)
	if err != nil {
		// Do not swallow the actual SQL error message!
		logging.Printf("[createIdentity] DB insert failed: %v", err)
		httputil.WriteJSON(w, http.StatusConflict, map[string]any{
			"error":   "database error",
			"code":    409,
			"details": err.Error(),
		})
		return
	}

	// Enforce uniqueness via unique_fields table (ADR-016).
	if err := enforceUserUniqueness(r.Context(), tx, userID, orgID, identifier, write); err != nil {
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

	// Capabilities handled by FGA — no-op for user_capabilities table.

	// Insert org membership (structural — same transaction as user creation).
	// Only insert if orgID is set (wizard creates users without X-Org-Id header).
	if orgID != "" {
		if _, err := tx.ExecContext(r.Context(),
			`INSERT OR IGNORE INTO memberships (resource_type, resource_id, user_id, role, added_at) VALUES ('org', ?, ?, 'member', ?)`,
			orgID, userID, now); err != nil {
			logging.Printf("[createUser] membership insert failed: %v", err)
		}
	}

	// Emit event.
	emitEvent(r.Context(), tx, "identity.created", userID, userID, "identity", map[string]any{
		"identifier": identifier,
	})

	if orgID != "" {
		creatorID := creatorFromRequest(r)
		if svc := FGAService; svc != nil {
			fgaAsync("user created", func() { //nolint:contextcheck
				ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
				defer cancel()
				if err := svc.OnResourceCreated(ctx, userID, creatorID, orgID); err != nil {
					logging.Printf("[fga] user created: %v", err)
				}
			})
		}
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	resp := UserResponse{
		ID:           userID,
		OrgID:        orgID,
		Identifier:   identifier,
		DisplayName:  displayName,
		UserType:     userType,
		State:        "active",
		SchemaID:     write.Schema.ID,
		SchemaType:   write.Schema.Type,
		Profile:      stripKeys(write.Payload, "metadata"),
		Metadata:     write.Payload["metadata"],
		Data:         write.Payload,
		Capabilities: req.Capabilities,
		CreatedAt:    now,
		UpdatedAt:    now,
	}
	httputil.WriteJSON(w, http.StatusCreated, resp)
}

func (a *API) getUser(w http.ResponseWriter, r *http.Request) {
	userID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	resp, err := a.loadUser(r, userID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "user not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, resp)
}

func (a *API) listUsers(w http.ResponseWriter, r *http.Request) {
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

	// Optional schema_type filter for a specific user-family schema.
	schemaType := r.URL.Query().Get("schema_type")
	// Optional org_id filter for org context scoping.
	orgIDFilter := r.URL.Query().Get("org_id")
	stateFilter := r.URL.Query().Get("state")

	var rows *sql.Rows
	var err error

	// Build query dynamically based on filters.
	var where []string
	var args []any
	baseSelect := `SELECT i.id, i.org_id, i.identifier, i.display_name, i.user_type, i.state,
		COALESCE(i.schema_id,''), COALESCE(s.type,''), i.metadata, i.created_at, i.updated_at
		 FROM users i
		 LEFT JOIN schemas s ON i.schema_id = s.id`
	if schemaType != "" {
		where = append(where, `s.type = ?`)
		args = append(args, schemaType)
	}

	if orgIDFilter != "" {
		where = append(where, `i.id IN (SELECT user_id FROM memberships WHERE resource_type='org' AND resource_id=?)`)
		args = append(args, orgIDFilter)
	}
	if stateFilter != "" {
		where = append(where, `i.state = ?`)
		args = append(args, stateFilter)
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

	var users []UserResponse
	for rows.Next() {
		user, err := scanUserRow(rows)
		if err != nil {
			continue
		}
		user.Capabilities = a.loadCapabilities(r, user.ID)
		users = append(users, user)
	}

	var nextCursor string
	if len(users) > limit {
		users = users[:limit]
		nextCursor = users[len(users)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{
		Items:      users,
		NextCursor: nextCursor,
	})
}

func (a *API) updateUser(w http.ResponseWriter, r *http.Request) {
	userID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	var req UserRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	var currentIdentifier, currentDisplayName, currentSchemaID, currentMetadata, currentOrgID string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT identifier, COALESCE(display_name,''), COALESCE(schema_id,''), COALESCE(metadata,'{}'), COALESCE(org_id,'')
		 FROM users WHERE id = ?`,
		userID,
	).Scan(&currentIdentifier, &currentDisplayName, &currentSchemaID, &currentMetadata, &currentOrgID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
		return
	}

	currentMetadataMap := decodeObjectString(currentMetadata)
	schemaRec, err := schema.ResolveUserSchemaForWrite(r.Context(), a.db.SQL(), currentSchemaID)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
		return
	}

	data, err := objectMapOrEmpty(req.Data)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
		return
	}

	nextIdentifier := strings.TrimSpace(req.Identifier)
	nextDisplayName := strings.TrimSpace(req.DisplayName)

	var write *validatedUserWrite
	if len(data) > 0 {
		if nextIdentifier == "" {
			nextIdentifier = identifierFromUserData(schemaRec.Schema, data, currentIdentifier)
		}
		if nextIdentifier == "" {
			nextIdentifier = currentIdentifier
		}
		if nextDisplayName == "" {
			nextDisplayName = stringFromAny(data["display_name"])
		}
		if nextDisplayName == "" {
			nextDisplayName = currentDisplayName
		}
		write, err = validatedUserWriteFromData(schemaRec, nextIdentifier, nextDisplayName, data)
	} else {
		mergedMetadata := cloneObjectMap(currentMetadataMap)
		profile, err := schema.ObjectMap(req.Profile)
		if err != nil {
			httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
			return
		}
		extraMetadata, err := schema.ObjectMap(req.Metadata)
		if err != nil {
			httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
			return
		}
		for key, value := range extraMetadata {
			mergedMetadata[key] = value
		}
		for key, value := range profile {
			mergedMetadata[key] = value
		}
		if nextIdentifier == "" {
			nextIdentifier = currentIdentifier
		}
		if nextDisplayName == "" {
			nextDisplayName = currentDisplayName
		}
		existingWrite, prepErr := a.prepareExistingUserWrite(r.Context(), currentSchemaID, nextIdentifier, nextDisplayName, mergedMetadata)
		if prepErr != nil {
			httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(prepErr))
			return
		}
		write = existingWrite
	}
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
		return
	}

	if nextDisplayName == "" {
		nextDisplayName = currentDisplayName
	}
	if nextIdentifier == "" {
		nextIdentifier = currentIdentifier
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	defer tx.Rollback()
	result, err := tx.ExecContext(r.Context(),
		`UPDATE users
		 SET identifier = ?, state = COALESCE(NULLIF(?, ''), state), display_name = ?, metadata = ?, updated_at = ?
		 WHERE id = ?`,
		nextIdentifier, req.State, nextDisplayName, write.MetadataJSON, timeNow(), userID,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
		return
	}
	if err := reindexUserUniqueness(r.Context(), tx, userID, currentOrgID, nextIdentifier, write); err != nil {
		if v, ok := err.(*uniqueness.ViolationError); ok {
			httputil.WriteJSON(w, http.StatusConflict, map[string]any{
				"error": "uniqueness_violation",
				"field": v.Field,
				"value": v.Value,
				"scope": v.Scope,
			})
			return
		}
		httputil.WriteError(w, http.StatusConflict, "identity already exists")
		return
	}
	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}

	a.EmitAuthEvent(r.Context(), "identity.updated", userID, map[string]any{
		"state": req.State,
	})

	resp, _ := a.loadUser(r, userID)
	httputil.WriteJSON(w, http.StatusOK, resp)
}

func (a *API) deleteUser(w http.ResponseWriter, r *http.Request) {
	userID, err := parseID(r, "id")
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

	// Release unique_fields before deleting the user row (ADR-016).
	// This is the primary cleanup — the FK CASCADE is a safety net.
	if err := uniqueness.Release(r.Context(), tx, userID); err != nil {
		logging.Printf("[deleteUser] warn: failed to release unique fields: %v", err)
	}

	result, err := tx.ExecContext(r.Context(), `DELETE FROM users WHERE id = ?`, userID)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
		return
	}

	emitEvent(r.Context(), tx, "identity.deleted", userID, userID, "identity", nil)

	// FGA: clean up all tuples (best-effort — orphan tuples on deleted users are harmless).
	if svc := FGAService; svc != nil {
		if err := svc.OnResourceDeleted(r.Context(), userID); err != nil {
			logging.Printf("[fga] warn: failed to delete user tuples (will be cleaned by reconciler): %v", err)
		}
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit failed")
		return
	}

	a.bus.Signal()

	w.WriteHeader(http.StatusNoContent)
}

func (a *API) setEntityPassword(w http.ResponseWriter, r *http.Request) {
	userID := r.PathValue("id")
	if userID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "invalid identity id")
		return
	}

	var req SetUserPasswordRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Password == "" {
		httputil.WriteError(w, http.StatusBadRequest, "password is required")
		return
	}

	pwd := auth.NewPasswords(a.db)
	if err := pwd.SetPassword(r.Context(), userID, req.Password); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to set password")
		return
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
		req.OrgID = ""
	}

	schemaJSON, err := json.Marshal(req.Schema)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid schema")
		return
	}

	if err := schema.ValidateSchemaDocument(schemaJSON); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
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

	if err := schema.ValidateSchemaDocument(schemaJSON); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, err.Error())
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
		`SELECT COUNT(*) FROM users i
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
		UserID string `json:"user_id"` // identity ID or identifier
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.UserID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "user_id is required")
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
		`SELECT i.identifier, COALESCE(i.metadata, '{}'), COALESCE(sc.schema, '{}')
		 FROM users i
		 LEFT JOIN schemas sc ON i.schema_id = sc.id
		 WHERE i.identifier = ? OR i.id = ?`,
		req.UserID, req.UserID,
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
		`SELECT s.type, COUNT(*) FROM users i
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

	// Providers are now entities — their count appears via the schema type join above.
	// No separate provider count needed.

	// Count orgs from the dedicated orgs table.
	var orgCount int
	if err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM orgs `).Scan(&orgCount); err == nil {
		counts["org"] = orgCount
	}

	// Count apps from the apps table.
	var appCount int
	if err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM apps `).Scan(&appCount); err == nil {
		counts["apps"] = appCount
	}

	// Total user count (all types).
	var userCount int
	if err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM users `).Scan(&userCount); err == nil {
		counts["users"] = userCount
	}

	httputil.WriteJSON(w, http.StatusOK, counts)
}

// getMetaSchema returns the canonical Zitadel identity schema meta-schema.
func (a *API) getMetaSchema(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/schema+json")
	w.WriteHeader(http.StatusOK)
	w.Write([]byte(schema.MetaSchema))
}

// validateSchemaAnnotations validates x-* annotation keys against allowed sets.
func validateSchemaAnnotations(schemaJSON []byte) string {
	var raw map[string]json.RawMessage
	if json.Unmarshal(schemaJSON, &raw) != nil {
		return "invalid JSON"
	}

	if msg := validateAuthMethods(raw); msg != "" {
		return msg
	}
	if msg := validateCaptcha(raw); msg != "" {
		return msg
	}
	if msg := validateFingerprint(raw); msg != "" {
		return msg
	}
	if msg := validateRateLimit(raw); msg != "" {
		return msg
	}
	if msg := validateLoginFlow(raw); msg != "" {
		return msg
	}
	return ""
}

func validateAuthMethods(raw map[string]json.RawMessage) string {
	authMethodsRaw, ok := raw["x-auth-methods"]
	if !ok {
		return ""
	}
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
	return ""
}

func validateCaptcha(raw map[string]json.RawMessage) string {
	captchaRaw, ok := raw["x-captcha"]
	if !ok {
		return ""
	}
	var captcha map[string]any
	if json.Unmarshal(captchaRaw, &captcha) != nil {
		return "x-captcha must be an object"
	}
	allowed := map[string]bool{
		"provider": true, "mode": true, "difficulty": true,
		"algorithm": true, "steps": true, "site_key": true, "secret_key": true,
	}
	for key := range captcha {
		if !allowed[key] {
			return fmt.Sprintf("unknown key %q in x-captcha; allowed: provider, mode, difficulty, algorithm, steps, site_key, secret_key", key)
		}
	}
	if p, ok := captcha["provider"].(string); ok {
		validProviders := map[string]bool{
			"altcha": true, "hcaptcha": true, "recaptcha": true, "turnstile": true, "none": true,
		}
		if !validProviders[p] {
			return fmt.Sprintf("unknown captcha provider %q; allowed: altcha, hcaptcha, recaptcha, turnstile, none", p)
		}
	}
	if m, ok := captcha["mode"].(string); ok {
		validModes := map[string]bool{"always": true, "risk_based": true, "never": true}
		if !validModes[m] {
			return fmt.Sprintf("unknown captcha mode %q; allowed: always, risk_based, never", m)
		}
	}
	return ""
}

func validateFingerprint(raw map[string]json.RawMessage) string {
	fpRaw, ok := raw["x-fingerprint"]
	if !ok {
		return ""
	}
	var fp map[string]any
	if json.Unmarshal(fpRaw, &fp) != nil {
		return "x-fingerprint must be an object"
	}
	allowed := map[string]bool{
		"enabled": true, "provider": true, "persist": true, "steps": true,
	}
	for key := range fp {
		if !allowed[key] {
			return fmt.Sprintf("unknown key %q in x-fingerprint; allowed: enabled, provider, persist, steps", key)
		}
	}
	if p, ok := fp["provider"].(string); ok {
		if p != "thumbmarkjs" && p != "built_in" {
			return fmt.Sprintf("unknown fingerprint provider %q; allowed: thumbmarkjs, built_in", p)
		}
	}
	return ""
}

func validateRateLimit(raw map[string]json.RawMessage) string {
	rlRaw, ok := raw["x-rate-limit"]
	if !ok {
		return ""
	}
	var rl map[string]any
	if json.Unmarshal(rlRaw, &rl) != nil {
		return "x-rate-limit must be an object"
	}
	allowed := map[string]bool{
		"max_attempts": true, "window_seconds": true, "lockout_seconds": true, "scope": true,
	}
	for key := range rl {
		if !allowed[key] {
			return fmt.Sprintf("unknown key %q in x-rate-limit; allowed: max_attempts, window_seconds, lockout_seconds, scope", key)
		}
	}
	if s, ok := rl["scope"].(string); ok {
		validScopes := map[string]bool{"ip": true, "identifier": true, "fingerprint": true}
		if !validScopes[s] {
			return fmt.Sprintf("unknown rate limit scope %q; allowed: ip, identifier, fingerprint", s)
		}
	}
	return ""
}

func validateLoginFlow(raw map[string]json.RawMessage) string {
	lfRaw, ok := raw["x-login-flow"]
	if !ok {
		return ""
	}
	var lf map[string]any
	if json.Unmarshal(lfRaw, &lf) != nil {
		return "x-login-flow must be an object"
	}
	allowed := map[string]bool{
		"flow_id": true, "inherit": true, "override": true,
	}
	for key := range lf {
		if !allowed[key] {
			return fmt.Sprintf("unknown key %q in x-login-flow; allowed: flow_id, inherit, override", key)
		}
	}
	return ""
}

func (a *API) schemaIdentityCount(w http.ResponseWriter, r *http.Request) {
	schemaID := r.PathValue("id")

	var count int
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COUNT(*) FROM users WHERE schema_id = ?`, schemaID,
	).Scan(&count)
	if err != nil {
		count = 0
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"count": count})
}

func (a *API) loadUser(r *http.Request, userID string) (UserResponse, error) {
	var resp UserResponse
	var displayName, metaStr, schemaID sql.NullString
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, org_id, identifier, display_name, user_type, state, COALESCE(schema_id,''), metadata, created_at, updated_at
		 FROM users WHERE id = ?`, userID,
	).Scan(&resp.ID, &resp.OrgID, &resp.Identifier, &displayName, &resp.UserType, &resp.State, &schemaID,
		&metaStr, &resp.CreatedAt, &resp.UpdatedAt)
	if err != nil {
		return resp, err
	}
	if displayName.Valid {
		resp.DisplayName = displayName.String
	}
	if schemaID.Valid {
		resp.SchemaID = schemaID.String
	}
	resp.Capabilities = a.loadCapabilities(r, userID)

	metadata := map[string]any{}
	if metaStr.Valid {
		_ = json.Unmarshal([]byte(metaStr.String), &metadata)
	}
	if resp.SchemaID != "" {
		if schemaRec, err := schema.LoadSchemaRecord(r.Context(), a.db.SQL(), resp.SchemaID); err == nil {
			resp.SchemaType = schemaRec.Type
			resp.Data = schema.MaterializeUserData(schemaRec.Schema, resp.Identifier, resp.DisplayName, metadata)
		}
	}
	if resp.Data == nil {
		resp.Data = schema.MaterializeUserData("", resp.Identifier, resp.DisplayName, metadata)
	}
	if dataMap, ok := resp.Data.(map[string]any); ok {
		resp.Profile = stripKeys(dataMap, "metadata")
		resp.Metadata = dataMap["metadata"]
	}

	// Enrich: org memberships from the memberships table.
	orgRows, err2 := a.db.SQL().QueryContext(r.Context(),
		`SELECT m.resource_id, COALESCE(o.name,''), m.role, m.added_at
		 FROM memberships m
		 LEFT JOIN orgs o ON o.id = m.resource_id
		 WHERE m.user_id = ? AND m.resource_type = 'org'
		 ORDER BY m.added_at ASC`, userID)
	if err2 == nil {
		defer orgRows.Close()
		for orgRows.Next() {
			var om OrgMembershipDTO
			if err := orgRows.Scan(&om.OrgID, &om.OrgName, &om.Role, &om.AddedAt); err != nil {
				continue
			}
			resp.Orgs = append(resp.Orgs, om)
		}
		_ = orgRows.Err() // non-fatal enrichment; ignore iteration errors
	}

	return resp, nil
}

func (a *API) loadCapabilities(_ *http.Request, _ string) []string {
	// POC: capabilities are derived from FGA, not a table.
	// Return ["admin"] for all authenticated users for backward compat.
	return []string{"admin"}
}

func scanUserRow(rows *sql.Rows) (UserResponse, error) {
	var resp UserResponse
	var displayName, metaStr, schemaID, schemaType sql.NullString
	err := rows.Scan(&resp.ID, &resp.OrgID, &resp.Identifier, &displayName, &resp.UserType, &resp.State,
		&schemaID, &schemaType, &metaStr, &resp.CreatedAt, &resp.UpdatedAt)
	if err != nil {
		return resp, err
	}
	if displayName.Valid {
		resp.DisplayName = displayName.String
	}
	if schemaID.Valid {
		resp.SchemaID = schemaID.String
	}
	if schemaType.Valid {
		resp.SchemaType = schemaType.String
	}
	if metaStr.Valid {
		json.Unmarshal([]byte(metaStr.String), &resp.Metadata)
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

// --- Generic Resource Handlers ---

// listResource returns a handler that lists rows from a dedicated table.
// Returns all columns as JSON objects.
func (a *API) listResource(table string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		limit := 50
		if l := r.URL.Query().Get("limit"); l != "" {
			if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 200 {
				limit = n
			}
		}
		cursor := r.URL.Query().Get("cursor")

		// Discover columns dynamically so we don't need per-table column lists.
		colsQuery := fmt.Sprintf(`SELECT name FROM pragma_table_info('%s')`, table)
		colRows, err := a.db.SQL().QueryContext(r.Context(), colsQuery)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}
		var colNames []string
		for colRows.Next() {
			var c string
			if err := colRows.Scan(&c); err == nil {
				colNames = append(colNames, c)
			}
		}
		defer colRows.Close()
		if err := colRows.Err(); err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}
		if len(colNames) == 0 {
			httputil.WriteError(w, http.StatusInternalServerError, "no columns found")
			return
		}

		query := fmt.Sprintf(`SELECT %s FROM %s WHERE id > ? ORDER BY id ASC LIMIT ?`,
			strings.Join(colNames, ", "), table)
		args := []any{cursor, limit + 1}
		rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}
		defer rows.Close()

		var items []map[string]any
		for rows.Next() {
			values := make([]any, len(colNames))
			ptrs := make([]any, len(colNames))
			for i := range values {
				ptrs[i] = &values[i]
			}
			if err := rows.Scan(ptrs...); err != nil {
				continue
			}
			row := make(map[string]any, len(colNames))
			for i, col := range colNames {
				row[col] = values[i]
			}
			items = append(items, row)
		}
		if err := rows.Err(); err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}

		var nextCursor string
		if len(items) > limit {
			items = items[:limit]
			nextCursor = fmt.Sprint(items[len(items)-1]["id"])
		}

		httputil.WriteJSON(w, http.StatusOK, ListResponse{
			Items:      items,
			NextCursor: nextCursor,
		})
	}
}

// getResource returns a handler that fetches a single row by ID from a dedicated table.
func (a *API) getResource(table string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		id, err := parseID(r, "id")
		if err != nil {
			httputil.WriteError(w, http.StatusBadRequest, "invalid id")
			return
		}

		// Discover columns dynamically.
		colsQuery := fmt.Sprintf(`SELECT name FROM pragma_table_info('%s')`, table)
		colRows, err := a.db.SQL().QueryContext(r.Context(), colsQuery)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}
		var colNames []string
		for colRows.Next() {
			var c string
			if err := colRows.Scan(&c); err == nil {
				colNames = append(colNames, c)
			}
		}
		defer colRows.Close()
		if err := colRows.Err(); err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}

		query := fmt.Sprintf(`SELECT %s FROM %s WHERE id = ?`,
			strings.Join(colNames, ", "), table)
		args := []any{id}
		rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
		if err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}
		defer rows.Close()

		if !rows.Next() {
			httputil.WriteError(w, http.StatusNotFound, "not found")
			return
		}

		values := make([]any, len(colNames))
		ptrs := make([]any, len(colNames))
		for i := range values {
			ptrs[i] = &values[i]
		}
		if err := rows.Scan(ptrs...); err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "scan failed")
			return
		}

		if err := rows.Err(); err != nil {
			httputil.WriteError(w, http.StatusInternalServerError, "query failed")
			return
		}

		row := make(map[string]any, len(colNames))
		for i, col := range colNames {
			row[col] = values[i]
		}

		httputil.WriteJSON(w, http.StatusOK, row)
	}
}

// execer abstracts *sql.Tx and *sql.DB for event insertion.
type execer interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}

// emitEventTo is the single implementation for audit event emission.
// All other emit* functions delegate here.
func emitEventTo(ctx context.Context, db execer, eventType, actorID, aggregateID, aggregateType string, payload map[string]any) {
	eventID := id.New()
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	requestID := telemetry.RequestIDFromContext(ctx)
	sessionID := telemetry.SessionIDFromContext(ctx)
	flowID := telemetry.FlowIDFromContext(ctx)
	fingerprint := telemetry.FingerprintFromContext(ctx)
	clientID := telemetry.ClientIDFromContext(ctx)
	tokenID := telemetry.TokenIDFromContext(ctx)
	delegationType := telemetry.DelegationTypeFromContext(ctx)
	sdkName := telemetry.SDKNameFromContext(ctx)
	sdkVersion := telemetry.SDKVersionFromContext(ctx)

	db.ExecContext(ctx, //nolint:errcheck // fire-and-forget audit event
		`INSERT INTO events (id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, request_id, session_id, flow_id, fingerprint, client_id, token_id, delegation_type, sdk_name, sdk_version, created_at)
		 VALUES (?, ?, ?, '0', ?, '', ?, ?, ?, '{}', ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))`,
		eventID, eventType, eventCategory(eventType), actorID, aggregateID, aggregateType, payloadJSON, requestID, sessionID, flowID, fingerprint, clientID, tokenID, delegationType, sdkName, sdkVersion)
}

// emitEvent emits an audit event within a transaction.
func emitEvent(ctx context.Context, tx *sql.Tx, eventType string, actorID, aggregateID, aggregateType string, payload map[string]any) {
	emitEventTo(ctx, tx, eventType, actorID, aggregateID, aggregateType, payload)
}

// EmitAuthEvent emits an auth-category event outside a transaction and signals the bus.
func (a *API) EmitAuthEvent(ctx context.Context, eventType string, actorID string, payload map[string]any) {
	emitEventTo(ctx, a.db.SQL(), eventType, actorID, actorID, "auth", payload)
	a.bus.Signal()
}

// EmitEvent emits a generic event outside a transaction and signals the bus.
func (a *API) EmitEvent(ctx context.Context, eventType, actorID, aggregateID, aggregateType string, payload map[string]any) {
	emitEventTo(ctx, a.db.SQL(), eventType, actorID, aggregateID, aggregateType, payload)
	a.bus.Signal()
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
func (a *API) GetIdentityByID(r *http.Request, userID string) (UserResponse, error) {
	return a.loadUser(r, userID)
}

// CreateUserInternal is an exported helper for the UI to create an identity.
func (a *API) CreateUserInternal(r *http.Request, req UserRequest) (UserResponse, error) {
	userID := id.New()

	now := time.Now().UTC().Format(time.RFC3339)
	write, err := a.prepareUserWrite(r.Context(), req.SchemaID, req.Identifier, req.DisplayName, req.Metadata, req.Profile)
	if err != nil {
		return UserResponse{}, err
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		return UserResponse{}, fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, '_global', ?, ?, ?, 'active', ?, ?, ?, ?)`,
		userID, req.Identifier, req.DisplayName, func() string {
			if write.Schema.Type == "service_user" || write.Schema.Type == "ai_agent" {
				return write.Schema.Type
			}
			return "human"
		}(), write.Schema.ID, write.MetadataJSON, now, now)
	if err != nil {
		return UserResponse{}, fmt.Errorf("insert: %w", err)
	}
	if err := enforceUserUniqueness(r.Context(), tx, userID, "_global", req.Identifier, write); err != nil {
		return UserResponse{}, err
	}

	emitEvent(r.Context(), tx, "identity.created", userID, userID, "identity", map[string]any{
		"identifier": req.Identifier,
	})

	if err := tx.Commit(); err != nil {
		return UserResponse{}, fmt.Errorf("commit: %w", err)
	}
	a.bus.Signal()

	return UserResponse{
		ID: userID, OrgID: "", Identifier: req.Identifier, DisplayName: req.DisplayName,
		State: "active", Profile: req.Profile, Capabilities: req.Capabilities,
		CreatedAt: now, UpdatedAt: now,
	}, nil
}

// UpdateUserInternal is an exported helper for the UI to update an identity.
func (a *API) UpdateUserInternal(r *http.Request, userID string, req UserRequest) (UserResponse, error) {
	var currentIdentifier, currentDisplayName, currentSchemaID, currentMetadata, orgID string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT identifier, COALESCE(display_name,''), COALESCE(schema_id,''), COALESCE(metadata,'{}'), COALESCE(org_id,'')
		 FROM users WHERE id = ?`,
		userID,
	).Scan(&currentIdentifier, &currentDisplayName, &currentSchemaID, &currentMetadata, &orgID)
	if err != nil {
		return UserResponse{}, fmt.Errorf("identity %s", userID)
	}

	metadata, err := schema.ObjectMap(json.RawMessage(currentMetadata))
	if err != nil {
		return UserResponse{}, err
	}
	profile, err := schema.ObjectMap(req.Profile)
	if err != nil {
		return UserResponse{}, err
	}
	for k, v := range profile {
		metadata[k] = v
	}

	nextDisplayName := currentDisplayName
	if strings.TrimSpace(req.DisplayName) != "" {
		nextDisplayName = strings.TrimSpace(req.DisplayName)
	}

	write, err := a.prepareExistingUserWrite(r.Context(), currentSchemaID, currentIdentifier, nextDisplayName, metadata)
	if err != nil {
		return UserResponse{}, err
	}

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		return UserResponse{}, fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`UPDATE users
		 SET state = COALESCE(NULLIF(?, ''), state),
		     display_name = ?,
		     metadata = ?,
		     updated_at = ?
		 WHERE id = ?`,
		req.State, nextDisplayName, write.MetadataJSON, time.Now().UTC().Format(time.RFC3339), userID,
	)
	if err != nil {
		return UserResponse{}, fmt.Errorf("update: %w", err)
	}
	if err := reindexUserUniqueness(r.Context(), tx, userID, orgID, currentIdentifier, write); err != nil {
		return UserResponse{}, err
	}
	if err := tx.Commit(); err != nil {
		return UserResponse{}, fmt.Errorf("commit: %w", err)
	}

	a.EmitAuthEvent(r.Context(), "identity.updated", userID, nil)

	return a.loadUser(r, userID)
}

// DeleteIdentityInternal is an exported helper for the UI to delete an identity.
func (a *API) DeleteIdentityInternal(r *http.Request, userID string) error {
	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	result, err := tx.ExecContext(r.Context(), `DELETE FROM users WHERE id = ?`, userID)
	if err != nil {
		return fmt.Errorf("delete: %w", err)
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		return fmt.Errorf("identity %s", userID)
	}

	emitEvent(r.Context(), tx, "identity.deleted", userID, userID, "identity", nil)
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}
	a.bus.Signal()
	return nil
}

// DB returns the database instance for direct queries (used by UI session handling).
func (a *API) DB() *database.DB { return a.db }
