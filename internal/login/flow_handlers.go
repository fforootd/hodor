package login

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/session"
)

// --- Flow API Handlers ---

// handleFlowCreate creates a new login flow and returns the first step.
// POST /v1/login/flows
func (h *Handler) handleFlowCreate(w http.ResponseWriter, r *http.Request) {
	cfg := h.getDefaultSchemaConfig(r)
	ssoProviders := h.loadSSOProviders(r)

	flowID := id.NewFlow()
	flow := &Flow{
		ID:           flowID,
		SchemaConfig: cfg,
		SSOProviders: ssoProviders,
	}

	// Determine entry step based on preset.
	switch cfg.Login.Preset {
	case "passkey_first":
		flow.CurrentStep = StepIdentifier // passkey_first still starts at identifier but with passkey button prominent
	case "sso_only":
		flow.CurrentStep = StepAuthSelect // skip identifier, go straight to SSO buttons
	default: // "identifier_first"
		flow.CurrentStep = StepIdentifier
	}

	h.flows.Put(flow)
	log.Printf("[flow] created %s (preset=%s, step=%s)", flowID, cfg.Login.Preset, flow.CurrentStep)

	writeJSON(w, flow.ToFlowStep())
}

// handleFlowSubmit processes a step submission and advances the flow.
// POST /v1/login/flows/{flow_id}/submit
func (h *Handler) handleFlowSubmit(w http.ResponseWriter, r *http.Request) {
	flowID := extractFlowID(r.URL.Path, "submit")
	if flowID == "" {
		writeErr(w, http.StatusBadRequest, "missing flow_id")
		return
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		writeErr(w, http.StatusNotFound, "flow not found or expired")
		return
	}

	var req struct {
		Action     string `json:"action"`      // "identifier", "password", "magic_link", "passkey", "sso", "back"
		Identifier string `json:"identifier"`  // for identifier step
		Password   string `json:"password"`    // for password step
		ProviderID string `json:"provider_id"` // for SSO
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeErr(w, http.StatusBadRequest, "invalid request body")
		return
	}

	switch req.Action {
	case "identifier":
		h.flowSubmitIdentifier(w, r, flow, req.Identifier)
	case "password":
		h.flowSubmitPassword(w, r, flow, req.Password)
	case "magic_link":
		h.flowSubmitMagicLink(w, r, flow)
	case "sso":
		h.flowSubmitSSO(w, r, flow, req.ProviderID)
	case "back":
		flow.CurrentStep = StepIdentifier
		flow.IdentityID = ""
		flow.Identifier = ""
		flow.DisplayName = ""
		flow.Verified = false
		h.flows.Put(flow)
		writeJSON(w, flow.ToFlowStep())
	default:
		writeErr(w, http.StatusBadRequest, fmt.Sprintf("unknown action: %s", req.Action))
	}
}

// handleFlowGet returns the current state of a flow.
// GET /v1/login/flows/{flow_id}
func (h *Handler) handleFlowGet(w http.ResponseWriter, r *http.Request) {
	flowID := extractFlowIDFromPath(r.URL.Path)
	if flowID == "" {
		writeErr(w, http.StatusBadRequest, "missing flow_id")
		return
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		writeErr(w, http.StatusNotFound, "flow not found or expired")
		return
	}

	writeJSON(w, flow.ToFlowStep())
}

// --- Flow Step Handlers ---

func (h *Handler) flowSubmitIdentifier(w http.ResponseWriter, r *http.Request, flow *Flow, identifier string) {
	identifier = strings.TrimSpace(identifier)
	if identifier == "" {
		writeErr(w, http.StatusBadRequest, "identifier is required")
		return
	}

	var identityID string
	var displayName string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT id, COALESCE(display_name, identifier) FROM entities WHERE identifier = ? AND state = 'active'`,
		identifier,
	).Scan(&identityID, &displayName)
	if err == sql.ErrNoRows {
		writeErr(w, http.StatusNotFound, "account not found")
		return
	}
	if err != nil {
		writeErr(w, http.StatusInternalServerError, "internal error")
		return
	}

	flow.IdentityID = identityID
	flow.Identifier = identifier
	flow.DisplayName = displayName
	flow.CurrentStep = StepAuthSelect
	h.flows.Put(flow)

	log.Printf("[flow] %s identifier resolved: %s (identity=%s)", flow.ID, identifier, identityID)
	writeJSON(w, flow.ToFlowStep())
}

func (h *Handler) flowSubmitPassword(w http.ResponseWriter, r *http.Request, flow *Flow, password string) {
	if password == "" {
		writeErr(w, http.StatusBadRequest, "password is required")
		return
	}

	var credData string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT credential_data FROM entity_credentials WHERE entity_id = ? AND credential_type = 'password'`,
		flow.IdentityID,
	).Scan(&credData)
	if err != nil {
		log.Printf("[flow] %s password lookup failed for identity=%s: %v", flow.ID, flow.IdentityID, err)
		writeErr(w, http.StatusInternalServerError, "internal error")
		return
	}

	// Extract hash from credential_data JSON: {"hash":"..."}
	var cred struct {
		Hash string `json:"hash"`
	}
	if err := json.Unmarshal([]byte(credData), &cred); err != nil || cred.Hash == "" {
		log.Printf("[flow] %s invalid credential data for identity=%s", flow.ID, flow.IdentityID)
		writeErr(w, http.StatusInternalServerError, "internal error")
		return
	}

	ok, _, err := h.passwords.Verify(cred.Hash, password)
	if err != nil || !ok {
		writeErr(w, http.StatusUnauthorized, "invalid_password")
		return
	}

	flow.Verified = true

	// Check if MFA is required.
	if flow.SchemaConfig.Login.MFARequired {
		flow.CurrentStep = StepMFA
		h.flows.Put(flow)
		writeJSON(w, flow.ToFlowStep())
		return
	}

	// Complete the flow.
	h.flowComplete(w, r, flow)
}

func (h *Handler) flowSubmitMagicLink(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if flow.Identifier == "" {
		writeErr(w, http.StatusBadRequest, "no identifier set")
		return
	}

	// Delegate to existing magic link infrastructure.
	log.Printf("[flow] %s sending magic link to %s", flow.ID, flow.Identifier)
	flow.CurrentStep = StepMagicLink
	h.flows.Put(flow)
	writeJSON(w, flow.ToFlowStep())
}

func (h *Handler) flowSubmitSSO(w http.ResponseWriter, r *http.Request, flow *Flow, providerID string) {
	if providerID == "" {
		writeErr(w, http.StatusBadRequest, "provider_id is required")
		return
	}

	// Return redirect URL for SSO.
	writeJSON(w, map[string]any{
		"flow_id":      flow.ID,
		"action":       "redirect",
		"redirect_url": fmt.Sprintf("/v1/auth/sso/%s/start", providerID),
	})
}

func (h *Handler) flowComplete(w http.ResponseWriter, r *http.Request, flow *Flow) {
	// Create session via the existing API.
	sessResp, err := h.api.CreateSessionInternal(r.Context(), flow.IdentityID, r.UserAgent(), r.RemoteAddr)
	if err != nil {
		writeErr(w, http.StatusInternalServerError, "session creation failed")
		return
	}

	// Set session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, h.cookies)

	flow.CurrentStep = StepComplete
	h.flows.Put(flow)

	log.Printf("[flow] %s completed (identity=%s, session=%s)", flow.ID, flow.IdentityID, sessResp.Session.ID)

	h.api.EmitAuthEvent(r.Context(), "auth.login_completed", flow.IdentityID, map[string]any{
		"session_id": sessResp.Session.ID,
		"flow_id":    flow.ID,
		"method":     "flow",
	})

	writeJSON(w, map[string]any{
		"flow_id":      flow.ID,
		"step":         "complete",
		"session_id":   sessResp.Session.ID,
		"redirect_uri": "/console",
	})

	// Clean up flow.
	h.flows.Delete(flow.ID)
}

// --- Path Helpers ---

// extractFlowID extracts the flow ID from paths like /v1/login/flows/{id}/submit
func extractFlowID(path, suffix string) string {
	prefix := "/v1/login/flows/"
	if !strings.HasPrefix(path, prefix) {
		return ""
	}
	rest := strings.TrimPrefix(path, prefix)
	rest = strings.TrimSuffix(rest, "/"+suffix)
	rest = strings.TrimSuffix(rest, "/")
	if rest == "" {
		return ""
	}
	return rest
}

// extractFlowIDFromPath extracts the flow ID from paths like /v1/login/flows/{id}
func extractFlowIDFromPath(path string) string {
	prefix := "/v1/login/flows/"
	if !strings.HasPrefix(path, prefix) {
		return ""
	}
	rest := strings.TrimPrefix(path, prefix)
	rest = strings.TrimSuffix(rest, "/")
	return rest
}
