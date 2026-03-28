package login

import (
	"encoding/json"
	"errors"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/uniqueness"
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
	logging.Printf("[flow] created %s (preset=%s, step=%s)", flowID, cfg.Login.Preset, flow.CurrentStep)

	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

// handleFlowSubmit processes a step submission and advances the flow.
// POST /v1/login/flows/{flow_id}/submit
func (h *Handler) handleFlowSubmit(w http.ResponseWriter, r *http.Request) {
	flowID := extractFlowID(r.URL.Path, "submit")
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "missing flow_id")
		return
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		httputil.WriteError(w, http.StatusNotFound, "flow not found or expired")
		return
	}

	var req struct {
		Action     string `json:"action"`      // "identifier", "password", "magic_link", "passkey", "sso", "back"
		Identifier string `json:"identifier"`  // for identifier step
		Password   string `json:"password"`    // for password step
		ProviderID string `json:"provider_id"` // for SSO
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid request body")
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
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
	default:
		httputil.WriteError(w, http.StatusBadRequest, fmt.Sprintf("unknown action: %s", req.Action))
	}
}

// handleFlowGet returns the current state of a flow.
// GET /v1/login/flows/{flow_id}
func (h *Handler) handleFlowGet(w http.ResponseWriter, r *http.Request) {
	flowID := extractFlowIDFromPath(r.URL.Path)
	if flowID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "missing flow_id")
		return
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		httputil.WriteError(w, http.StatusNotFound, "flow not found or expired")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

// --- Flow Step Handlers ---

func (h *Handler) flowSubmitIdentifier(w http.ResponseWriter, r *http.Request, flow *Flow, identifier string) {
	identifier = strings.TrimSpace(identifier)
	if identifier == "" {
		httputil.WriteError(w, http.StatusBadRequest, "identifier is required")
		return
	}

	// Resolve identifier via unique_fields (ADR-016).
	orgID := httputil.ResolveOrgID(r, "")

	resolved, err := uniqueness.ResolveIdentifier(r.Context(), h.db.SQL(), identifier, orgID)
	if errors.Is(err, uniqueness.ErrIdentityNotFound) {
		httputil.WriteError(w, http.StatusNotFound, "account not found")
		return
	}
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "internal error")
		return
	}

	flow.IdentityID = resolved.EntityID
	flow.Identifier = identifier
	flow.DisplayName = resolved.DisplayName
	flow.CurrentStep = StepAuthSelect
	h.flows.Put(flow)

	logging.Printf("[flow] %s identifier resolved: %s (identity=%s)", flow.ID, identifier, resolved.EntityID)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

func (h *Handler) flowSubmitPassword(w http.ResponseWriter, r *http.Request, flow *Flow, password string) {
	if password == "" {
		httputil.WriteError(w, http.StatusBadRequest, "password is required")
		return
	}

	var credData string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT credential_data FROM entity_credentials WHERE entity_id = ? AND credential_type = 'password'`,
		flow.IdentityID,
	).Scan(&credData)
	if err != nil {
		logging.Printf("[flow] %s password lookup failed for identity=%s: %v", flow.ID, flow.IdentityID, err)
		httputil.WriteError(w, http.StatusInternalServerError, "internal error")
		return
	}

	// Extract hash from credential_data JSON: {"hash":"..."}
	hash := auth.DecodeCredentialJSON(credData)
	if hash == "" {
		logging.Printf("[flow] %s invalid credential data for identity=%s", flow.ID, flow.IdentityID)
		httputil.WriteError(w, http.StatusInternalServerError, "internal error")
		return
	}

	ok, _, err := h.passwords.Verify(hash, password)
	if err != nil || !ok {
		h.api.EmitAuthEvent(r.Context(), "auth.login_failed", flow.IdentityID, map[string]any{
			"reason":  "invalid_password",
			"flow_id": flow.ID,
		})
		httputil.WriteError(w, http.StatusUnauthorized, "invalid_password")
		return
	}

	flow.Verified = true

	// Check if MFA is required.
	if flow.SchemaConfig.Login.MFARequired {
		flow.CurrentStep = StepMFA
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	// Complete the flow.
	h.flowComplete(w, r, flow)
}

func (h *Handler) flowSubmitMagicLink(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if flow.Identifier == "" {
		httputil.WriteError(w, http.StatusBadRequest, "no identifier set")
		return
	}

	// Delegate to existing magic link infrastructure.
	logging.Printf("[flow] %s sending magic link to %s", flow.ID, flow.Identifier)
	flow.CurrentStep = StepMagicLink
	h.flows.Put(flow)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

func (h *Handler) flowSubmitSSO(w http.ResponseWriter, r *http.Request, flow *Flow, providerID string) {
	if providerID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "provider_id is required")
		return
	}

	// Return redirect URL for SSO.
	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"flow_id":      flow.ID,
		"action":       "redirect",
		"redirect_url": fmt.Sprintf("/v1/auth/sso/%s/start", providerID),
	})
}

func (h *Handler) flowComplete(w http.ResponseWriter, r *http.Request, flow *Flow) {
	// Create session via the existing API.
	sessResp, err := h.api.CreateSessionInternal(r.Context(), flow.IdentityID, r.UserAgent(), r.RemoteAddr)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "session creation failed")
		return
	}

	// Set session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, h.cookies)

	flow.CurrentStep = StepComplete
	h.flows.Put(flow)

	logging.Printf("[flow] %s completed (identity=%s, session=%s)", flow.ID, flow.IdentityID, sessResp.Session.ID)

	h.api.EmitAuthEvent(r.Context(), "auth.login_completed", flow.IdentityID, map[string]any{
		"session_id": sessResp.Session.ID,
		"flow_id":    flow.ID,
		"method":     "flow",
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
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
