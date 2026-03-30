package api

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/id"
)

// ImportRequest is the body for POST /v1/import.
type ImportRequest struct {
	Providers      []ImportProvider      `json:"providers,omitempty"`
	Entities       []ImportEntity        `json:"users,omitempty"`
	LinkedAccounts []ImportLinkedAccount `json:"linked_identities,omitempty"`
	OnConflict     string                `json:"on_conflict"` // skip (default), fail, update
}

// ImportProvider is a provider to import.
type ImportProvider struct {
	ID             string            `json:"id,omitempty"`
	Name           string            `json:"name"`
	Protocol       string            `json:"protocol,omitempty"`
	Template       string            `json:"template,omitempty"`
	Config         map[string]any    `json:"config,omitempty"`
	ClaimOverrides map[string]string `json:"claim_overrides,omitempty"`
	AutoRegister   *bool             `json:"auto_register,omitempty"`
}

// ImportEntity is an identity to import.
type ImportEntity struct {
	Identifier  string         `json:"identifier"`
	DisplayName string         `json:"display_name"`
	SchemaID    string         `json:"schema_id,omitempty"`
	State       string         `json:"state,omitempty"`
	Password    string         `json:"password,omitempty"`
	Profile     map[string]any `json:"profile,omitempty"`
}

// ImportLinkedAccount links an identity to a provider.
type ImportLinkedAccount struct {
	IdentityIdentifier string `json:"user_identifier"` // resolves to identity ID
	ProviderName       string `json:"provider_name"`   // resolves to provider ID
	ExternalSub        string `json:"external_sub"`
	ExternalEmail      string `json:"external_email,omitempty"`
}

// ImportResult tracks per-item results.
type ImportResult struct {
	Index    int    `json:"index"`
	Resource string `json:"resource"`
	Status   string `json:"status"` // created, skipped, error
	ID       any    `json:"id,omitempty"`
	Reason   string `json:"reason,omitempty"`
}

// RegisterBulkRoutes mounts import and bulk endpoints.
func (a *API) RegisterBulkRoutes(mux *http.ServeMux) {
	mux.HandleFunc("POST /v1/import", a.requireAdmin(a.handleImport))
	mux.HandleFunc("POST /v1/admin/bulk", a.requireAdmin(a.handleEntitiesBulk))
}

// --- Global Import ---

func (a *API) handleImport(w http.ResponseWriter, r *http.Request) {
	var req ImportRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.OnConflict == "" {
		req.OnConflict = "skip"
	}

	var results []ImportResult
	var created, skipped, errors int

	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "begin transaction: "+err.Error())
		return
	}
	defer tx.Rollback()

	// Phase 1: Providers (no dependencies).
	for i, p := range req.Providers {
		res := a.importProvider(r, tx, p, i, req.OnConflict)
		results = append(results, res)
		switch res.Status {
		case "created":
			created++
		case "skipped":
			skipped++
		case "error":
			errors++
			if req.OnConflict == "fail" {
				httputil.WriteJSON(w, http.StatusConflict, importResponse(results, created, skipped, errors))
				return
			}
		}
	}

	// Phase 2: Identities (may reference schemas).
	for i, ident := range req.Entities {
		res := a.importIdentity(r, tx, ident, len(req.Providers)+i, req.OnConflict)
		results = append(results, res)
		switch res.Status {
		case "created":
			created++
		case "skipped":
			skipped++
		case "error":
			errors++
			if req.OnConflict == "fail" {
				httputil.WriteJSON(w, http.StatusConflict, importResponse(results, created, skipped, errors))
				return
			}
		}
	}

	// Phase 3: Linked accounts (depends on entities + providers).
	for i, la := range req.LinkedAccounts {
		res := a.importLinkedAccount(r, tx, la, len(req.Providers)+len(req.Entities)+i, req.OnConflict)
		results = append(results, res)
		switch res.Status {
		case "created":
			created++
		case "skipped":
			skipped++
		case "error":
			errors++
			if req.OnConflict == "fail" {
				httputil.WriteJSON(w, http.StatusConflict, importResponse(results, created, skipped, errors))
				return
			}
		}
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit: "+err.Error())
		return
	}

	a.bus.Signal()
	httputil.WriteJSON(w, http.StatusOK, importResponse(results, created, skipped, errors))
}

func importResponse(results []ImportResult, created, skipped, errors int) map[string]any {
	return map[string]any{
		"total":   len(results),
		"created": created,
		"skipped": skipped,
		"errors":  errors,
		"results": results,
	}
}

// --- Import Helpers ---

func (a *API) importProvider(r *http.Request, tx *sql.Tx, p ImportProvider, idx int, onConflict string) ImportResult {
	if p.Name == "" {
		return ImportResult{Index: idx, Resource: "provider", Status: "error", Reason: "name required"}
	}

	// Check conflict in providers table.
	var existing string
	err := tx.QueryRowContext(r.Context(),
		`SELECT id FROM providers WHERE name = ?`, p.Name).Scan(&existing)
	if err == nil {
		if onConflict == "skip" {
			return ImportResult{Index: idx, Resource: "provider", Status: "skipped", ID: existing, Reason: "name exists"}
		}
		return ImportResult{Index: idx, Resource: "provider", Status: "error", Reason: "provider exists"}
	}

	provID := p.ID
	if provID == "" {
		provID = fmt.Sprintf("prov_%s", generateShortID())
	}
	if p.Protocol == "" {
		p.Protocol = "oidc"
	}
	if p.Template == "" {
		p.Template = "custom"
	}

	configMap := map[string]any{}
	if p.Config != nil {
		configMap = p.Config
	}
	overrideMap := map[string]string{}
	if p.ClaimOverrides != nil {
		overrideMap = p.ClaimOverrides
	}
	autoReg := true
	if p.AutoRegister != nil {
		autoReg = *p.AutoRegister
	}

	configJSON, _ := json.Marshal(configMap)
	overrideJSON, _ := json.Marshal(overrideMap)

	orgID := "_global" // Bulk import defaults to global org for now.
	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, 0, 'provider_v1', '{}', datetime('now'), datetime('now'))`,
		provID, orgID, p.Name, p.Protocol, p.Template,
		string(configJSON), string(overrideJSON), autoReg)
	if err != nil {
		return ImportResult{Index: idx, Resource: "provider", Status: "error", Reason: err.Error()}
	}

	return ImportResult{Index: idx, Resource: "provider", Status: "created", ID: provID}
}

func (a *API) importIdentity(r *http.Request, tx *sql.Tx, ident ImportEntity, idx int, onConflict string) ImportResult {
	if ident.Identifier == "" {
		return ImportResult{Index: idx, Resource: "identity", Status: "error", Reason: "identifier required"}
	}

	// Check conflict.
	var existingID string
	err := tx.QueryRowContext(r.Context(), `SELECT id FROM users WHERE identifier = ?`, ident.Identifier).Scan(&existingID)
	if err == nil {
		if onConflict == "update" {
			// Upsert: update display_name and metadata.
			metadataJSON := "{}"
			if ident.Profile != nil {
				b, _ := json.Marshal(ident.Profile)
				metadataJSON = string(b)
			}
			tx.ExecContext(r.Context(),
				`UPDATE users SET display_name = ?, metadata = ?, updated_at = datetime('now') WHERE id = ?`,
				ident.DisplayName, metadataJSON, existingID)
			return ImportResult{Index: idx, Resource: "identity", Status: "updated", ID: existingID}
		}
		if onConflict == "skip" {
			return ImportResult{Index: idx, Resource: "identity", Status: "skipped", ID: existingID, Reason: "identifier exists"}
		}
		return ImportResult{Index: idx, Resource: "identity", Status: "error", Reason: "identifier exists"}
	}

	newID := id.New()

	state := ident.State
	if state == "" {
		state = "active"
	}
	schemaID := ident.SchemaID
	if schemaID == "" {
		schemaID = "human_user_v1"
	}

	metadataJSON := "{}"
	if ident.Profile != nil {
		b, _ := json.Marshal(ident.Profile)
		metadataJSON = string(b)
	}

	orgID := "_global"
	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, 'human', ?, ?, ?, datetime('now'), datetime('now'))`,
		newID, orgID, ident.Identifier, ident.DisplayName, state, schemaID, metadataJSON)
	if err != nil {
		return ImportResult{Index: idx, Resource: "identity", Status: "error", Reason: err.Error()}
	}

	// Hash password and store as entity_credential.
	if ident.Password != "" {
		pw := auth.NewPasswords(nil)
		hash, err := pw.Hash(ident.Password)
		if err == nil {
			credID := id.New()
			credJSON := auth.EncodeCredentialJSON(hash)
			tx.ExecContext(r.Context(),
				`INSERT INTO credentials (id, user_id, type, data) VALUES (?, ?, 'password', ?)`,
				credID, newID, credJSON)
		}
	}

	return ImportResult{Index: idx, Resource: "identity", Status: "created", ID: newID}
}

func (a *API) importLinkedAccount(r *http.Request, tx *sql.Tx, la ImportLinkedAccount, idx int, onConflict string) ImportResult {
	// Resolve identity by identifier.
	var userID string
	err := tx.QueryRowContext(r.Context(), `SELECT id FROM users WHERE identifier = ?`, la.IdentityIdentifier).Scan(&userID)
	if err != nil {
		return ImportResult{Index: idx, Resource: "linked_account", Status: "error", Reason: "identity not found: " + la.IdentityIdentifier}
	}

	// Resolve provider by name from providers table.
	var providerID string
	err = tx.QueryRowContext(r.Context(),
		`SELECT id FROM providers WHERE name = ?`, la.ProviderName).Scan(&providerID)
	if err != nil {
		return ImportResult{Index: idx, Resource: "linked_account", Status: "error", Reason: "provider not found: " + la.ProviderName}
	}

	// Check conflict.
	var existingLinkID string
	err = tx.QueryRowContext(r.Context(),
		`SELECT id FROM linked_identities WHERE provider_id = ? AND external_sub = ?`,
		providerID, la.ExternalSub).Scan(&existingLinkID)
	if err == nil {
		if onConflict == "skip" {
			return ImportResult{Index: idx, Resource: "linked_account", Status: "skipped", ID: existingLinkID, Reason: "already linked"}
		}
		return ImportResult{Index: idx, Resource: "linked_account", Status: "error", Reason: "already linked"}
	}

	linkID := id.New()
	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
		 VALUES (?, ?, ?, ?, ?, '{}', datetime('now'))`,
		linkID, userID, providerID, la.ExternalSub, la.ExternalEmail)
	if err != nil {
		return ImportResult{Index: idx, Resource: "linked_account", Status: "error", Reason: err.Error()}
	}

	return ImportResult{Index: idx, Resource: "linked_account", Status: "created", ID: linkID}
}

// --- Per-Resource Bulk: Identities ---

func (a *API) handleEntitiesBulk(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Entities   []ImportEntity `json:"users"`
		OnConflict string         `json:"on_conflict"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.OnConflict == "" {
		req.OnConflict = "skip"
	}

	// Delegate to import handler logic.
	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "begin transaction: "+err.Error())
		return
	}
	defer tx.Rollback()

	var results []ImportResult
	var created, skipped, errors int
	for i, ident := range req.Entities {
		res := a.importIdentity(r, tx, ident, i, req.OnConflict)
		results = append(results, res)
		switch res.Status {
		case "created", "updated":
			created++
		case "skipped":
			skipped++
		case "error":
			errors++
			if req.OnConflict == "fail" {
				httputil.WriteJSON(w, http.StatusConflict, importResponse(results, created, skipped, errors))
				return
			}
		}
	}

	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "commit: "+err.Error())
		return
	}

	a.bus.Signal()
	httputil.WriteJSON(w, http.StatusOK, importResponse(results, created, skipped, errors))
}
