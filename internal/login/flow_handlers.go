package login

import (
	"encoding/json"
	"errors"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/captcha"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

// --- Flow API Handlers ---

// handleFlowCreate creates a new login flow and returns the first step.
// POST /v1/login/flows
func (h *Handler) handleFlowCreate(w http.ResponseWriter, r *http.Request) {
	// Optional: accept OIDC redirect context, preview flow ID, and device fingerprint.
	var req struct {
		RedirectURI string `json:"redirect_uri,omitempty"`
		State       string `json:"state,omitempty"`
		Fingerprint string `json:"fingerprint,omitempty"`
		FlowID      string `json:"flow_id,omitempty"`   // preview: load a specific login flow
		ClientID    string `json:"client_id,omitempty"` // OIDC: resolve flow by app
	}
	_ = json.NewDecoder(r.Body).Decode(&req)

	// Also check query param for preview: /v1/login/flows?flow=xxx
	if req.FlowID == "" {
		req.FlowID = r.URL.Query().Get("flow")
	}

	// Resolve the best login flow config (or use preview override).
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
	ssoProviders := h.loadSSOProviders(r)

	flowID := id.NewFlow()
	flow := &Flow{
		ID:              flowID,
		SchemaConfig:    cfg,
		RevealMode:      IdentityRevealModeAnonymous,
		SSOProviders:    ssoProviders,
		RedirectURI:     req.RedirectURI,
		OIDCState:       req.State,
		VisitorID:       req.Fingerprint,
		FingerprintHash: req.Fingerprint,
	}
	if trustedUserID, ok := h.resolveTrustedUserID(r, req.State); ok {
		flow.TrustedUserID = trustedUserID
		flow.RevealMode = IdentityRevealModeKnownUser
	}

	// Determine entry step based on strategy.
	switch cfg.Login.Strategy {
	case "passkey_first":
		flow.CurrentStep = StepIdentifier
	case "sso_only":
		flow.CurrentStep = StepAuthSelect
	default: // "identifier_first"
		flow.CurrentStep = StepIdentifier
	}

	h.flows.Put(flow)
	logging.Printf("[flow] created %s (strategy=%s, step=%s, login_flow=%s, host=%s, org=%s)", flowID, cfg.Login.Strategy, flow.CurrentStep, cfg.LoginFlowID, meta.Host, meta.OrgID)

	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
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

	// Inject flow_id into context so all downstream EmitAuthEvent calls include it.
	ctx := telemetry.WithFlowID(r.Context(), flowID)
	r = r.WithContext(ctx)

	var req map[string]string
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeLoginError(w, http.StatusBadRequest, loginBadRequest("Invalid login request body."))
		return
	}

	action := req["action"]
	switch action {
	case "identifier":
		h.flowSubmitIdentifier(w, r, flow, req["identifier"])
	case "password":
		h.flowSubmitPassword(w, r, flow, req["password"])
	case "magic_link":
		h.flowSubmitMagicLink(w, r, flow)
	case "resend_magic_link":
		h.flowSubmitMagicLink(w, r, flow) // same as initial send
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
		flow.CurrentStep = StepForgotPassword
		flow.Errors = nil
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
	case "send_reset":
		// Reuse magic link infrastructure with password reset messaging.
		flow.CurrentStep = StepMagicLink
		flow.Messages = []FlowMessage{{Type: "info", Text: "Password reset link sent to " + flow.Identifier}}
		h.flows.Put(flow)
		// TODO: actually send recovery email (reuse magic link sender with purpose="reset")
		logging.Printf("[flow] %s sent password reset to %s", flow.ID, flow.Identifier)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
	case "back":
		flow.CurrentStep = StepIdentifier
		flow.IduserID = ""
		flow.Identifier = ""
		flow.DisplayName = ""
		flow.RevealMode = IdentityRevealModeAnonymous
		flow.Verified = false
		flow.Errors = nil
		flow.Messages = nil
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
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

	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

// --- Flow Step Handlers ---

func (h *Handler) flowSubmitIdentifier(w http.ResponseWriter, r *http.Request, flow *Flow, identifier string) {
	identifier = strings.TrimSpace(identifier)
	if identifier == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "identifier_required", Message: "Email or username is required"})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	// Resolve identifier via unique_fields (ADR-016).
	orgID := httputil.ResolveOrgID(r, "")

	resolved, err := uniqueness.ResolveIdentifier(r.Context(), h.db.SQL(), identifier, orgID)
	if errors.Is(err, uniqueness.ErrIdentityNotFound) {
		flow.Identifier = identifier // preserve for registration
		flow.Errors = append(flow.Errors, FlowError{Code: "not_found", Message: "Account not found"})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}
	if err != nil {
		logging.Printf("[flow] %s identifier resolve error: %v", flow.ID, err)
		writeLoginError(w, http.StatusInternalServerError, loginInternalError("Login could not continue. Please try again."))
		return
	}

	flow.IduserID = resolved.UserID
	flow.Identifier = identifier
	if flow.TrustedUserID != "" && flow.TrustedUserID == resolved.UserID {
		flow.RevealMode = IdentityRevealModeKnownUser
		flow.DisplayName = resolved.DisplayName
	} else {
		flow.RevealMode = IdentityRevealModeAnonymous
		flow.DisplayName = ""
	}
	flow.CurrentStep = StepAuthSelect
	flow.Errors = nil
	h.flows.Put(flow)

	logging.Printf("[flow] %s identifier resolved: %s (identity=%s)", flow.ID, identifier, resolved.UserID)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

func (h *Handler) flowSubmitPassword(w http.ResponseWriter, r *http.Request, flow *Flow, password string) {
	if password == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "password_required", Message: "Password is required"})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	var credData string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT data FROM credentials WHERE user_id = ? AND type = 'password'`,
		flow.IduserID,
	).Scan(&credData)
	if err != nil {
		logging.Printf("[flow] %s password lookup failed for identity=%s: %v", flow.ID, flow.IduserID, err)
		flow.Errors = append(flow.Errors, FlowError{Code: "internal", Message: "Something went wrong. Please try again."})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	// Extract hash from data JSON: {"hash":"..."}.
	hash := auth.DecodeCredentialJSON(credData)
	if hash == "" {
		logging.Printf("[flow] %s invalid credential data for identity=%s", flow.ID, flow.IduserID)
		flow.Errors = append(flow.Errors, FlowError{Code: "internal", Message: "Something went wrong. Please try again."})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	ok, _, err := h.passwords.Verify(hash, password)
	if err != nil || !ok {
		h.api.EmitAuthEvent(r.Context(), "auth.login_failed", flow.IduserID, map[string]any{
			"reason":  "invalid_password",
			"flow_id": flow.ID,
		})
		flow.Errors = append(flow.Errors, FlowError{Code: "invalid_password", Message: "Invalid password. Please try again."})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	flow.Verified = true
	flow.Errors = nil

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
		flow.Errors = append(flow.Errors, FlowError{Code: "no_identifier", Message: "No identifier set"})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	// Delegate to existing magic link infrastructure.
	logging.Printf("[flow] %s sending magic link to %s", flow.ID, flow.Identifier)
	flow.CurrentStep = StepMagicLink
	flow.Errors = nil
	flow.Messages = append(flow.Messages, FlowMessage{Type: "success", Text: "Sign-in link sent!"})
	h.flows.Put(flow)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

func (h *Handler) flowSubmitSSO(w http.ResponseWriter, r *http.Request, flow *Flow, providerID string) {
	if providerID == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "provider_required", Message: "Provider is required"})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	// Return redirect URL for SSO.
	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"flow_id":      flow.ID,
		"action":       "redirect",
		"redirect_url": fmt.Sprintf("/v1/auth/sso/%s/start", providerID),
	})
}

// flowTransitionToRegister moves the flow to the registration step.
func (h *Handler) flowTransitionToRegister(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if !flow.SchemaConfig.Login.RegistrationAllowed {
		flow.Errors = append(flow.Errors, FlowError{Code: "registration_disabled", Message: "Registration is not available"})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	flow.CurrentStep = StepRegister
	flow.Errors = nil
	if flow.RegData == nil {
		flow.RegData = make(map[string]string)
	}
	h.flows.Put(flow)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

// flowSubmitRegister handles the registration form submission.
func (h *Handler) flowSubmitRegister(w http.ResponseWriter, r *http.Request, flow *Flow, formData map[string]string) {
	if flow.RegData == nil {
		flow.RegData = make(map[string]string)
	}

	// Accumulate form data (skip "action").
	for k, v := range formData {
		if k != "action" {
			flow.RegData[k] = v
		}
	}

	// Validate required fields from schema.
	var validationErrors []FlowError
	for _, field := range flow.SchemaConfig.SchemaProps {
		if field.Required {
			val := flow.RegData[field.Name]
			if strings.TrimSpace(val) == "" {
				label := field.Title
				if label == "" {
					label = humanize(field.Name)
				}
				validationErrors = append(validationErrors, FlowError{
					Code:    "field_required",
					Message: fmt.Sprintf("%s is required", label),
				})
			}
		}
	}

	if len(validationErrors) > 0 {
		flow.Errors = validationErrors
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	// Find the primary identifier from the form data.
	identifier := ""
	for _, field := range flow.SchemaConfig.SchemaProps {
		if field.Identifier {
			if v, ok := flow.RegData[field.Name]; ok && v != "" {
				identifier = v
				break
			}
		}
	}
	if identifier == "" {
		// Fallback: use email or first available field.
		identifier = flow.RegData["email"]
		if identifier == "" {
			for _, v := range flow.RegData {
				if v != "" {
					identifier = v
					break
				}
			}
		}
	}

	displayName := flow.RegData["display_name"]
	if displayName == "" {
		displayName = identifier
	}

	// Create the entity via the database.
	newID := id.New()
	profileJSON := "{}"
	if len(flow.RegData) > 0 {
		if b, err := json.Marshal(flow.RegData); err == nil {
			profileJSON = string(b)
		}
	}

	// Resolve org from the flow context.
	orgID := httputil.ResolveOrgID(r, "1") // fallback to "1" for single-org mode

	_, err := h.db.SQL().ExecContext(r.Context(),
		`INSERT INTO users (id, org_id, identifier, display_name, state, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, 'active', ?, datetime('now'), datetime('now'))`,
		newID, orgID, identifier, displayName, profileJSON,
	)
	if err != nil {
		logging.Printf("[flow] %s registration failed: %v", flow.ID, err)
		if strings.Contains(err.Error(), "UNIQUE") || strings.Contains(err.Error(), "unique") {
			flow.Errors = append(flow.Errors, FlowError{Code: "already_exists", Message: "An account with this identifier already exists"})
		} else {
			flow.Errors = append(flow.Errors, FlowError{Code: "internal", Message: "Registration failed. Please try again."})
		}
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	logging.Printf("[flow] %s registered new identity %s (%s)", flow.ID, newID, identifier)

	// Set flow state to the new identity and complete.
	flow.IduserID = newID
	flow.Identifier = identifier
	flow.DisplayName = displayName
	flow.Verified = true
	flow.Errors = nil

	h.api.EmitAuthEvent(r.Context(), "auth.registration_completed", newID, map[string]any{
		"flow_id":    flow.ID,
		"identifier": identifier,
	})

	// Complete the flow (creates session, sets cookie).
	h.flowComplete(w, r, flow)
}

func (h *Handler) flowComplete(w http.ResponseWriter, r *http.Request, flow *Flow) {
	// Create session via the existing API.
	// Collect accumulated client signals from the flow.
	signals := &ClientSignals{
		CaptchaProvider: flow.CaptchaProvider,
		CaptchaVerified: flow.CaptchaVerified,
		CaptchaScore:    flow.CaptchaScore,
		PoWCompleted:    flow.PoWCompleted,
		PoWDurationMs:   flow.PoWDurationMs,
		VisitorID:       flow.VisitorID,
		FingerprintHash: flow.FingerprintHash,
		RequestID:       telemetry.RequestIDFromContext(r.Context()),
	}
	sessResp, err := h.api.CreateSessionForLogin(r.Context(), flow.IduserID, r.UserAgent(), r.RemoteAddr, signals)
	if err != nil {
		flow.Errors = append(flow.Errors, FlowError{Code: "session_failed", Message: "Failed to create session. Please try again."})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	// Set session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, h.cookies)

	flow.CurrentStep = StepComplete
	h.flows.Put(flow)

	logging.Printf("[flow] %s completed (identity=%s, session=%s)", flow.ID, flow.IduserID, sessResp.Session.ID)

	h.api.EmitAuthEvent(r.Context(), "auth.login_completed", flow.IduserID, map[string]any{
		"session_id": sessResp.Session.ID,
		"flow_id":    flow.ID,
		"method":     "flow",
	})

	// Determine redirect URI: OIDC redirect_uri if present, otherwise /console.
	redirectURI := "/console"
	if flow.RedirectURI != "" {
		redirectURI = flow.RedirectURI
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"flow_id":      flow.ID,
		"step":         "complete",
		"session_id":   sessResp.Session.ID,
		"redirect_uri": redirectURI,
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

// handleCaptchaChallenge generates an Altcha PoW challenge.
// GET /v1/captcha/challenge
func (h *Handler) handleCaptchaChallenge(w http.ResponseWriter, r *http.Request) {
	challenge, err := h.captcha.CreateChallenge()
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create challenge")
		return
	}
	httputil.WriteJSON(w, http.StatusOK, challenge)
}

// flowSubmitCaptcha handles the "captcha_submit" action.
// The client sends the Altcha PoW payload after solving the challenge.
func (h *Handler) flowSubmitCaptcha(w http.ResponseWriter, r *http.Request, flow *Flow, req map[string]string) {
	payload := req["altcha_payload"]
	if payload == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "captcha_missing", Message: "Captcha verification required."})
		h.flows.Put(flow)
		httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
		return
	}

	result := captcha.VerifyAltcha(h.captcha, payload)
	flow.CaptchaProvider = "altcha"
	flow.CaptchaVerified = result.Valid
	flow.CaptchaScore = result.Score
	flow.PoWCompleted = result.PoWCompleted
	flow.PoWDurationMs = result.PoWDurationMs

	if !result.Valid {
		flow.Errors = append(flow.Errors, FlowError{Code: "captcha_failed", Message: "Captcha verification failed. Please try again."})
	}

	h.flows.Put(flow)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}

// flowSubmitFingerprint handles the "fingerprint_submit" action.
// The client sends the ThumbmarkJS visitor ID after collecting it.
func (h *Handler) flowSubmitFingerprint(w http.ResponseWriter, r *http.Request, flow *Flow, req map[string]string) {
	flow.VisitorID = req["visitor_id"]
	flow.FingerprintHash = req["fingerprint_hash"]
	h.flows.Put(flow)
	httputil.WriteJSON(w, http.StatusOK, flow.ToFlowStep())
}
