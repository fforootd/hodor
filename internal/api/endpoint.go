package api

import (
	"encoding/json"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/instance"
	"github.com/zitadel/zitadel/internal/logging"
)

// ──────────────────────────────────────────────────────────────────
// Endpoint types
// ──────────────────────────────────────────────────────────────────

type EndpointRequest struct {
	Domain    string `json:"domain"`
	Path      string `json:"path,omitempty"`
	Component string `json:"component"`
	Enabled   *bool  `json:"enabled,omitempty"`
	TLSMode   string `json:"tls_mode,omitempty"`
}

type EndpointResponse struct {
	ID          string `json:"id"`
	InstanceID  string `json:"instance_id"`
	Domain      string `json:"domain"`
	Path        string `json:"path"`
	Component   string `json:"component"`
	Enabled     bool   `json:"enabled"`
	TLSMode     string `json:"tls_mode"`
	DNSVerified bool   `json:"dns_verified"`
	DNSMethod   string `json:"dns_method,omitempty"`
	DNSToken    string `json:"dns_token,omitempty"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

var validComponents = map[string]bool{
	"login":   true,
	"api":     true,
	"oidc":    true,
	"console": true,
	"account": true,
}

// ──────────────────────────────────────────────────────────────────
// Routes
// ──────────────────────────────────────────────────────────────────

func (a *API) RegisterEndpointRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/endpoints", a.listEndpoints)
	mux.HandleFunc("POST /v1/endpoints", a.createEndpoint)
	mux.HandleFunc("GET /v1/endpoints/{id}", a.getEndpoint)
	mux.HandleFunc("PATCH /v1/endpoints/{id}", a.updateEndpoint)
	mux.HandleFunc("DELETE /v1/endpoints/{id}", a.deleteEndpoint)
	mux.HandleFunc("POST /v1/endpoints/{id}/verify", a.verifyEndpoint)
	logging.Printf("[api] registered /v1/endpoints (full CRUD + verify)")
}

// ──────────────────────────────────────────────────────────────────
// Handlers
// ──────────────────────────────────────────────────────────────────

func (a *API) listEndpoints(w http.ResponseWriter, r *http.Request) {
	iid := instance.FromContext(r.Context())

	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, instance_id, domain, path, component, enabled, tls_mode,
		        dns_verified, COALESCE(dns_method,''), COALESCE(dns_token,''),
		        created_at, updated_at
		 FROM endpoints WHERE instance_id = ?
		 ORDER BY domain, path`, iid)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var items []EndpointResponse
	for rows.Next() {
		var ep EndpointResponse
		if err := rows.Scan(&ep.ID, &ep.InstanceID, &ep.Domain, &ep.Path,
			&ep.Component, &ep.Enabled, &ep.TLSMode,
			&ep.DNSVerified, &ep.DNSMethod, &ep.DNSToken,
			&ep.CreatedAt, &ep.UpdatedAt); err != nil {
			continue
		}
		items = append(items, ep)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: items})
}

func (a *API) createEndpoint(w http.ResponseWriter, r *http.Request) {
	var req EndpointRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Domain == "" {
		httputil.WriteError(w, http.StatusBadRequest, "domain is required")
		return
	}
	if !validComponents[req.Component] {
		httputil.WriteError(w, http.StatusBadRequest, "component must be one of: login, api, oidc, console, account")
		return
	}
	if req.Path == "" {
		req.Path = "/"
	}
	if req.TLSMode == "" {
		req.TLSMode = "auto"
	}

	iid := instance.FromContext(r.Context())
	epID := id.New()
	now := time.Now().UTC().Format(time.RFC3339)

	// Generate DNS verification token.
	dnsToken := crypto.MustRandomHex(16)

	enabled := true
	if req.Enabled != nil {
		enabled = *req.Enabled
	}

	_, err := a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO endpoints (id, instance_id, domain, path, component, enabled, tls_mode, dns_token, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		epID, iid, req.Domain, req.Path, req.Component, enabled, req.TLSMode, dnsToken, now, now,
	)
	if err != nil {
		if strings.Contains(err.Error(), "UNIQUE") {
			httputil.WriteError(w, http.StatusConflict, "endpoint already exists for this domain+path")
			return
		}
		httputil.WriteError(w, http.StatusInternalServerError, "create failed: "+err.Error())
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusCreated, EndpointResponse{
		ID:         epID,
		InstanceID: iid,
		Domain:     req.Domain,
		Path:       req.Path,
		Component:  req.Component,
		Enabled:    enabled,
		TLSMode:    req.TLSMode,
		DNSToken:   dnsToken,
		CreatedAt:  now,
		UpdatedAt:  now,
	})
}

func (a *API) getEndpoint(w http.ResponseWriter, r *http.Request) {
	epID := r.PathValue("id")
	iid := instance.FromContext(r.Context())

	var ep EndpointResponse
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, instance_id, domain, path, component, enabled, tls_mode,
		        dns_verified, COALESCE(dns_method,''), COALESCE(dns_token,''),
		        created_at, updated_at
		 FROM endpoints WHERE id = ? AND instance_id = ?`, epID, iid,
	).Scan(&ep.ID, &ep.InstanceID, &ep.Domain, &ep.Path,
		&ep.Component, &ep.Enabled, &ep.TLSMode,
		&ep.DNSVerified, &ep.DNSMethod, &ep.DNSToken,
		&ep.CreatedAt, &ep.UpdatedAt)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "endpoint not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, ep)
}

func (a *API) updateEndpoint(w http.ResponseWriter, r *http.Request) {
	epID := r.PathValue("id")
	iid := instance.FromContext(r.Context())

	var req EndpointRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	setClauses := []string{"updated_at = ?"}
	args := []any{now}

	if req.Domain != "" {
		setClauses = append(setClauses, "domain = ?")
		args = append(args, req.Domain)
	}
	if req.Path != "" {
		setClauses = append(setClauses, "path = ?")
		args = append(args, req.Path)
	}
	if req.Component != "" {
		if !validComponents[req.Component] {
			httputil.WriteError(w, http.StatusBadRequest, "invalid component")
			return
		}
		setClauses = append(setClauses, "component = ?")
		args = append(args, req.Component)
	}
	if req.Enabled != nil {
		setClauses = append(setClauses, "enabled = ?")
		args = append(args, *req.Enabled)
	}
	if req.TLSMode != "" {
		setClauses = append(setClauses, "tls_mode = ?")
		args = append(args, req.TLSMode)
	}

	args = append(args, epID, iid)

	query := "UPDATE endpoints SET " + strings.Join(setClauses, ", ") + " WHERE id = ? AND instance_id = ?"
	result, err := a.db.SQL().ExecContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "endpoint not found")
		return
	}

	a.bus.Signal()

	a.getEndpoint(w, r)
}

func (a *API) deleteEndpoint(w http.ResponseWriter, r *http.Request) {
	epID := r.PathValue("id")
	iid := instance.FromContext(r.Context())

	result, err := a.db.SQL().ExecContext(r.Context(),
		`DELETE FROM endpoints WHERE id = ? AND instance_id = ?`, epID, iid)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "delete failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "endpoint not found")
		return
	}

	a.bus.Signal()
	w.WriteHeader(http.StatusNoContent)
}

// verifyEndpoint marks an endpoint as DNS-verified.
// In a production system this would do a real TXT/CNAME lookup.
// For the POC, we accept both methods and auto-verify.
func (a *API) verifyEndpoint(w http.ResponseWriter, r *http.Request) {
	epID := r.PathValue("id")
	iid := instance.FromContext(r.Context())

	var req struct {
		Method string `json:"method"` // "txt" or "cname"
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.Method != "txt" && req.Method != "cname" {
		httputil.WriteError(w, http.StatusBadRequest, "method must be 'txt' or 'cname'")
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)
	result, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE endpoints SET dns_verified = 1, dns_method = ?, updated_at = ? WHERE id = ? AND instance_id = ?`,
		req.Method, now, epID, iid)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "verify failed")
		return
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		httputil.WriteError(w, http.StatusNotFound, "endpoint not found")
		return
	}

	a.bus.Signal()

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"status":       "verified",
		"method":       req.Method,
		"dns_verified": true,
	})
}
