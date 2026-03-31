package login

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/telemetry"
)

// handleFlowCreate creates a new login flow and returns the first step.
// POST /v1/login/flows
func (h *Handler) handleFlowCreate(w http.ResponseWriter, r *http.Request) {
	var req struct {
		AuthRequestID string `json:"auth_request_id,omitempty"`
		RedirectURI   string `json:"redirect_uri,omitempty"`
		State         string `json:"state,omitempty"`
		Fingerprint   string `json:"fingerprint,omitempty"`
		FlowID        string `json:"flow_id,omitempty"`
		ClientID      string `json:"client_id,omitempty"`
	}
	_ = json.NewDecoder(r.Body).Decode(&req)

	if req.FlowID == "" {
		req.FlowID = r.URL.Query().Get("flow")
	}
	if req.AuthRequestID == "" {
		req.AuthRequestID = strings.TrimSpace(r.URL.Query().Get("auth_request_id"))
	}

	var oidcAuthReq *oidcAuthRequestContext
	if req.AuthRequestID != "" {
		authReq, err := h.lookupOIDCAuthRequest(r.Context(), req.AuthRequestID)
		if err != nil {
			writeLoginError(w, http.StatusBadRequest, loginBadRequest("OIDC authentication request was not found or has expired."))
			return
		}
		oidcAuthReq = authReq
		if req.RedirectURI == "" {
			req.RedirectURI = authReq.RedirectURI
		}
		if req.State == "" {
			req.State = authReq.State
		}
	}

	cfg, meta, err := h.getResolvedConfigStrict(r, req.FlowID)
	if err != nil {
		status, apiErr := h.classifyInitError(r.Context(), err)
		logging.Printf(
			"[login] init failed host=%s org=%s mode=%s resolved_flow=%s used_default_schema=%t err=%v code=%s kind=%s retryable=%t",
			meta.Host, meta.OrgID, meta.ResolutionMode, meta.ResolvedFlowID, meta.UsedDefaultSchema, err, apiErr.Code, apiErr.Kind, apiErr.Retryable,
		)
		writeLoginError(w, status, apiErr)
		return
	}
	ssoProviders := h.loadSSOProviders(r, cfg)

	flowID := id.NewFlow()
	flow := &Flow{
		ID:              flowID,
		SchemaConfig:    cfg,
		RevealMode:      IdentityRevealModeAnonymous,
		SSOProviders:    ssoProviders,
		AuthRequestID:   req.AuthRequestID,
		RedirectURI:     req.RedirectURI,
		OIDCState:       req.State,
		VisitorID:       req.Fingerprint,
		FingerprintHash: req.Fingerprint,
		AuthMethod:      "password",
	}
	if oidcAuthReq != nil {
		flow.OIDCPrompts = oidcAuthReq.Prompt
		flow.OIDCLoginHint = oidcAuthReq.LoginHint
		if flow.Identifier == "" && oidcAuthReq.LoginHint != "" {
			flow.Identifier = oidcAuthReq.LoginHint
		}
	}
	if allowTrustedSessionReuse(flow.OIDCPrompts) {
		if trustedUserID, ok := h.resolveTrustedUserID(r, req.State); ok {
			flow.TrustedUserID = trustedUserID
			flow.RevealMode = IdentityRevealModeKnownUser
			flow.Identifier, flow.DisplayName = h.loadTrustedIdentitySummary(r.Context(), trustedUserID)
		}
	}

	if flow.AuthRequestID != "" && requireSilentTrustedSession(flow.OIDCPrompts) {
		if flow.TrustedUserID != "" {
			if err := h.completeOIDCAuthRequest(r.Context(), flow.AuthRequestID, flow.TrustedUserID); err != nil {
				writeLoginError(w, http.StatusInternalServerError, loginInternalError("OIDC login could not continue. Please try again."))
				return
			}
		}
		h.completeFlowWithTrustedSession(w, flow)
		return
	}

	if flow.AuthRequestID != "" && flow.TrustedUserID != "" {
		flow.CurrentStep = StepSessionReuse
		h.flows.Put(flow)
		h.renderFlowStep(w, r, flow)
		return
	}

	switch cfg.Login.Strategy {
	case "passkey_first":
		flow.CurrentStep = StepIdentifier
	case "sso_only":
		flow.CurrentStep = StepAuthSelect
	default:
		flow.CurrentStep = StepIdentifier
	}

	h.flows.Put(flow)
	logging.Printf("[flow] created %s (strategy=%s, step=%s, login_flow=%s, host=%s, org=%s)", flowID, cfg.Login.Strategy, flow.CurrentStep, cfg.LoginFlowID, meta.Host, meta.OrgID)
	h.renderFlowStep(w, r, flow)
}

// handleFlowSubmit processes a step submission and advances the flow.
// POST /v1/login/flows/{flow_id}/submit
func (h *Handler) handleFlowSubmit(w http.ResponseWriter, r *http.Request) {
	flowID := extractFlowID(r.URL.Path, "submit")
	if flowID == "" {
		writeLoginError(w, http.StatusBadRequest, loginBadRequest("Missing flow ID."))
		return
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		writeLoginError(w, http.StatusNotFound, loginFlowNotFound("Login flow was not found or has expired."))
		return
	}

	ctx := telemetry.WithFlowID(r.Context(), flowID)
	r = r.WithContext(ctx)

	var req map[string]string
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeLoginError(w, http.StatusBadRequest, loginBadRequest("Invalid login request body."))
		return
	}

	action := req["action"]
	if !h.ensureCaptchaVerifiedForAction(w, r, flow, action) {
		return
	}
	switch action {
	case "identifier":
		h.flowSubmitIdentifier(w, r, flow, req["identifier"])
	case "use_session":
		h.flowSubmitUseSession(w, r, flow)
	case "password":
		h.flowSubmitPassword(w, r, flow, req["password"])
	case "magic_link":
		h.flowSubmitMagicLink(w, r, flow)
	case "resend_magic_link":
		h.flowSubmitMagicLink(w, r, flow)
	case "sso":
		h.flowSubmitSSO(w, r, flow, req["provider_id"])
	case "register":
		h.flowTransitionToRegister(w, r, flow)
	case "register_submit":
		h.flowSubmitRegister(w, r, flow, req)
	case "captcha_submit":
		h.flowSubmitCaptcha(w, r, flow, req)
	case "fingerprint_submit":
		h.flowSubmitFingerprint(w, r, flow, req)
	case "forgot_password":
		flow.transitionToStep(StepForgotPassword)
		flow.Errors = nil
		flow.Messages = nil
		h.renderFlowStep(w, r, flow)
	case "send_reset":
		if flow.Identifier == "" {
			flow.Errors = append(flow.Errors, FlowError{Code: "no_identifier", Message: "No identifier set"})
			h.renderFlowStep(w, r, flow)
			return
		}
		if _, _, err := h.queueMagicLink(ctx, flow.Identifier, "reset"); err != nil {
			logging.Printf("[flow] %s failed to queue password reset for %s: %v", flow.ID, flow.Identifier, err)
			flow.Errors = append(flow.Errors, FlowError{Code: "reset_failed", Message: "We couldn't send a reset link right now. Please try again."})
			h.renderFlowStep(w, r, flow)
			return
		}
		flow.transitionToStep(StepMagicLink)
		flow.Errors = nil
		flow.Messages = []FlowMessage{{Type: "info", Text: "Password reset link sent to " + flow.Identifier}}
		logging.Printf("[flow] %s queued password reset to %s", flow.ID, flow.Identifier)
		h.renderFlowStep(w, r, flow)
	case "back":
		flow.clearCaptchaState()
		flow.transitionToStep(StepIdentifier)
		flow.IduserID = ""
		flow.Identifier = ""
		flow.DisplayName = ""
		flow.RevealMode = IdentityRevealModeAnonymous
		flow.Verified = false
		flow.Errors = nil
		flow.Messages = nil
		h.renderFlowStep(w, r, flow)
	default:
		writeLoginError(w, http.StatusBadRequest, loginBadRequest(fmt.Sprintf("Unknown login action %q.", action)))
	}
}

// handleFlowGet returns the current state of a flow.
// GET /v1/login/flows/{flow_id}
func (h *Handler) handleFlowGet(w http.ResponseWriter, r *http.Request) {
	flowID := extractFlowIDFromPath(r.URL.Path)
	if flowID == "" {
		writeLoginError(w, http.StatusBadRequest, loginBadRequest("Missing flow ID."))
		return
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		writeLoginError(w, http.StatusNotFound, loginFlowNotFound("Login flow was not found or has expired."))
		return
	}

	h.renderFlowStep(w, r, flow)
}

func (h *Handler) renderFlowStep(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if flow == nil {
		writeLoginError(w, http.StatusBadRequest, loginBadRequest("Login flow was not found or has expired."))
		return
	}

	ctx := telemetry.WithFlowID(r.Context(), flow.ID)
	r = r.WithContext(ctx)
	h.evaluatePreAuthRisk(r, flow)
	h.flows.Put(flow)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

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
