package login

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"strings"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/captcha"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/risk"
	"github.com/zitadel/zitadel/internal/schema"
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
		AuthRequestID string `json:"auth_request_id,omitempty"`
		RedirectURI   string `json:"redirect_uri,omitempty"`
		State         string `json:"state,omitempty"`
		Fingerprint   string `json:"fingerprint,omitempty"`
		FlowID        string `json:"flow_id,omitempty"`   // preview: load a specific login flow
		ClientID      string `json:"client_id,omitempty"` // OIDC: resolve flow by app
	}
	_ = json.NewDecoder(r.Body).Decode(&req)

	// Also check query param for preview: /v1/login/flows?flow=xxx
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
	h.renderFlowStep(w, r, flow)
}

type oidcAuthRequestContext struct {
	RedirectURI string
	State       string
	Prompt      []string
	LoginHint   string
}

func (h *Handler) lookupOIDCAuthRequest(ctx context.Context, requestID string) (*oidcAuthRequestContext, error) {
	var authReq oidcAuthRequestContext
	var dataJSON string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT redirect_uri, state, COALESCE(data, '{}')
		 FROM auth_states
		 WHERE id = ?
			   AND type = 'oidc_auth'
			   AND expires_at > datetime('now')`,
		requestID,
	).Scan(&authReq.RedirectURI, &authReq.State, &dataJSON)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, err
		}
		return nil, err
	}
	if dataJSON != "" && dataJSON != "{}" {
		var data map[string]any
		if err := json.Unmarshal([]byte(dataJSON), &data); err == nil {
			authReq.Prompt = stringSliceFromAny(data["prompt"])
			if loginHint, ok := data["login_hint"].(string); ok {
				authReq.LoginHint = loginHint
			}
		}
	}
	return &authReq, nil
}

func (h *Handler) completeOIDCAuthRequest(ctx context.Context, requestID, userID string) error {
	result, err := h.db.SQL().ExecContext(ctx,
		`UPDATE auth_states
		 SET user_id = ?, done = 1, auth_time = datetime('now')
		 WHERE id = ? AND type = 'oidc_auth'`,
		userID, requestID,
	)
	if err != nil {
		return err
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (h *Handler) oidcAuthorizeCallbackURL(requestID string) string {
	return "/authorize/callback?id=" + url.QueryEscape(requestID)
}

func stringSliceFromAny(value any) []string {
	values, ok := value.([]any)
	if !ok {
		return nil
	}
	result := make([]string, 0, len(values))
	for _, value := range values {
		if s, ok := value.(string); ok {
			result = append(result, s)
		}
	}
	return result
}

func hasOIDCPrompt(prompts []string, want string) bool {
	for _, prompt := range prompts {
		if prompt == want {
			return true
		}
	}
	return false
}

func allowTrustedSessionReuse(prompts []string) bool {
	return !hasOIDCPrompt(prompts, "login") && !hasOIDCPrompt(prompts, "select_account")
}

func requireSilentTrustedSession(prompts []string) bool {
	return hasOIDCPrompt(prompts, "none")
}

func (h *Handler) loadTrustedIdentitySummary(ctx context.Context, userID string) (identifier, displayName string) {
	_ = h.db.SQL().QueryRowContext(ctx,
		`SELECT COALESCE(identifier, ''), COALESCE(display_name, '')
		 FROM users
		 WHERE id = ?`,
		userID,
	).Scan(&identifier, &displayName)
	return identifier, displayName
}

func (h *Handler) completeFlowWithTrustedSession(w http.ResponseWriter, flow *Flow) {
	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"flow_id":      flow.ID,
		"step":         "complete",
		"session_id":   "",
		"redirect_uri": h.oidcAuthorizeCallbackURL(flow.AuthRequestID),
	})
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
		flow.transitionToStep(StepForgotPassword)
		flow.Errors = nil
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

func (h *Handler) evaluatePreAuthRisk(r *http.Request, flow *Flow) {
	if flow == nil || flow.SchemaConfig == nil || flow.SchemaConfig.Captcha == nil {
		return
	}
	if flow.SchemaConfig.Captcha.Mode != "risk_based" || !captchaActiveForStep(flow.SchemaConfig.Captcha, flow.CurrentStep) {
		flow.PreAuthRisk = nil
		flow.AdaptiveCaptcha = false
		return
	}

	result := risk.FailureResult(risk.StagePreAuth, risk.RecommendationRequireCaptcha)
	if h.risk != nil {
		evaluatedRisk, err := h.risk.Evaluate(r.Context(), risk.Input{
			Stage:          risk.StagePreAuth,
			UserID:         flow.IduserID,
			LoginFlowID:    flow.ID,
			IPAddress:      remoteIPFromAddr(r.RemoteAddr),
			UserAgent:      r.UserAgent(),
			TrustedSession: flow.TrustedUserID != "",
			Reauth:         flow.TrustedUserID != "" && flow.TrustedUserID == flow.IduserID,
			Signals: risk.Signals{
				CaptchaProvider: flow.CaptchaProvider,
				CaptchaVerified: flow.CaptchaVerified,
				CaptchaScore:    flow.CaptchaScore,
				PoWCompleted:    flow.PoWCompleted,
				PoWDurationMs:   flow.PoWDurationMs,
				VisitorID:       firstNonEmptyString(flow.VisitorID, telemetry.FingerprintFromContext(r.Context())),
				FingerprintHash: firstNonEmptyString(flow.FingerprintHash, flow.VisitorID, telemetry.FingerprintFromContext(r.Context())),
				RequestID:       telemetry.RequestIDFromContext(r.Context()),
			},
		})
		if err != nil {
			logging.Printf("[risk] pre-auth evaluation failed flow=%s step=%s: %v", flow.ID, flow.CurrentStep, err)
		} else {
			result = evaluatedRisk
		}
	}

	flow.PreAuthRisk = result
	flow.AdaptiveCaptcha = result.RecommendedNextStep == risk.RecommendationRequireCaptcha || result.RecommendedNextStep == risk.RecommendationBlock
	h.api.EmitEvent(r.Context(), "signal.risk_evaluated", flow.IduserID, flow.ID, "login_flow", map[string]any{
		"score":                 result.Score,
		"level":                 string(result.Level),
		"reasons":               result.Reasons,
		"recommended_next_step": string(result.RecommendedNextStep),
		"stage":                 string(result.Stage),
		"evaluator_version":     result.EvaluatorVersion,
		"policy_name":           "builtin_adaptive_captcha_v1",
		"policy_version":        "v1",
	})
}

func configuredCaptchaForStep(flow *Flow) (*CaptchaConfig, string, bool) {
	if flow == nil || flow.SchemaConfig == nil {
		return nil, "", false
	}
	scope := captchaScopeForStep(flow.CurrentStep)
	if scope == "" || !flow.captchaRequiredForCurrentStep() {
		return nil, "", false
	}
	return flow.SchemaConfig.Captcha, scope, true
}

func protectedCaptchaAction(action string) bool {
	switch action {
	case "identifier", "password", "magic_link", "sso", "register_submit", "send_reset":
		return true
	default:
		return false
	}
}

func (h *Handler) ensureCaptchaVerifiedForAction(w http.ResponseWriter, r *http.Request, flow *Flow, action string) bool {
	if !protectedCaptchaAction(action) {
		return true
	}
	h.evaluatePreAuthRisk(r, flow)
	_, scope, active := configuredCaptchaForStep(flow)
	if !active {
		return true
	}
	if flow.CaptchaVerified && flow.CaptchaVerifiedScope == scope {
		return true
	}

	flow.Errors = []FlowError{{Code: "captcha_required", Message: "Complete captcha verification to continue."}}
	h.renderFlowStep(w, r, flow)
	return false
}

// --- Flow Step Handlers ---

func (h *Handler) flowSubmitIdentifier(w http.ResponseWriter, r *http.Request, flow *Flow, identifier string) {
	identifier = strings.TrimSpace(identifier)
	if identifier == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "identifier_required", Message: "Email or username is required"})
		h.renderFlowStep(w, r, flow)
		return
	}

	// Resolve identifier via unique_fields (ADR-016).
	orgID := httputil.ResolveOrgID(r, "")

	resolved, err := uniqueness.ResolveIdentifier(r.Context(), h.db.SQL(), identifier, orgID)
	if errors.Is(err, uniqueness.ErrIdentityNotFound) {
		flow.Identifier = identifier // preserve for registration
		flow.Errors = append(flow.Errors, FlowError{Code: "not_found", Message: "Account not found"})
		h.renderFlowStep(w, r, flow)
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
	flow.transitionToStep(StepAuthSelect)
	flow.Errors = nil

	logging.Printf("[flow] %s identifier resolved: %s (identity=%s)", flow.ID, identifier, resolved.UserID)
	h.renderFlowStep(w, r, flow)
}

func (h *Handler) flowSubmitUseSession(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if flow.TrustedUserID == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "session_unavailable", Message: "Your existing session is no longer available. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}
	currentTrustedUserID, ok := h.resolveTrustedUserIDFromRequest(r)
	if !ok || currentTrustedUserID != flow.TrustedUserID {
		flow.Errors = append(flow.Errors, FlowError{Code: "session_unavailable", Message: "Your existing session is no longer available. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}
	if flow.AuthRequestID == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "oidc_request_missing", Message: "OIDC login could not continue. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}
	if err := h.completeOIDCAuthRequest(r.Context(), flow.AuthRequestID, flow.TrustedUserID); err != nil {
		flow.Errors = append(flow.Errors, FlowError{Code: "oidc_complete_failed", Message: "OIDC login could not continue. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.IduserID = flow.TrustedUserID
	flow.AuthMethod = "session_reuse"
	flow.transitionToStep(StepComplete)
	h.completeFlowWithTrustedSession(w, flow)
	h.flows.Delete(flow.ID)
}

func (h *Handler) flowSubmitPassword(w http.ResponseWriter, r *http.Request, flow *Flow, password string) {
	if password == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "password_required", Message: "Password is required"})
		h.renderFlowStep(w, r, flow)
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
		h.renderFlowStep(w, r, flow)
		return
	}

	// Extract hash from data JSON: {"hash":"..."}.
	hash := auth.DecodeCredentialJSON(credData)
	if hash == "" {
		logging.Printf("[flow] %s invalid credential data for identity=%s", flow.ID, flow.IduserID)
		flow.Errors = append(flow.Errors, FlowError{Code: "internal", Message: "Something went wrong. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	ok, _, err := h.passwords.Verify(hash, password)
	if err != nil || !ok {
		h.api.EmitAuthEvent(r.Context(), "auth.login_failed", flow.IduserID, map[string]any{
			"reason":  "invalid_password",
			"flow_id": flow.ID,
		})
		flow.Errors = append(flow.Errors, FlowError{Code: "invalid_password", Message: "Invalid password. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.Verified = true
	flow.AuthMethod = "password"
	flow.Errors = nil

	// Check if MFA is required.
	if flow.SchemaConfig.Login.MFARequired {
		flow.transitionToStep(StepMFA)
		h.renderFlowStep(w, r, flow)
		return
	}

	// Complete the flow.
	h.flowComplete(w, r, flow)
}

func (h *Handler) flowSubmitMagicLink(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if flow.Identifier == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "no_identifier", Message: "No identifier set"})
		h.renderFlowStep(w, r, flow)
		return
	}

	if _, _, err := h.queueMagicLink(r.Context(), flow.Identifier, "login"); err != nil {
		logging.Printf("[flow] %s failed to queue magic link to %s: %v", flow.ID, flow.Identifier, err)
		flow.Errors = append(flow.Errors, FlowError{Code: "magic_link_failed", Message: "We couldn't send a sign-in link right now. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	logging.Printf("[flow] %s queued magic link to %s", flow.ID, flow.Identifier)
	flow.transitionToStep(StepMagicLink)
	flow.Errors = nil
	flow.Messages = append(flow.Messages, FlowMessage{Type: "success", Text: "Sign-in link sent!"})
	h.renderFlowStep(w, r, flow)
}

func (h *Handler) flowSubmitSSO(w http.ResponseWriter, r *http.Request, flow *Flow, providerID string) {
	if providerID == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "provider_required", Message: "Provider is required"})
		h.renderFlowStep(w, r, flow)
		return
	}

	// Return redirect URL for SSO.
	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"flow_id":      flow.ID,
		"action":       "redirect",
		"redirect_url": fmt.Sprintf("/v1/auth/sso/%s/start?flow_id=%s", providerID, flow.ID),
	})
}

// flowTransitionToRegister moves the flow to the registration step.
func (h *Handler) flowTransitionToRegister(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if !flow.SchemaConfig.Login.RegistrationAllowed {
		flow.Errors = append(flow.Errors, FlowError{Code: "registration_disabled", Message: "Registration is not available"})
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.transitionToStep(StepRegister)
	flow.Errors = nil
	if flow.RegData == nil {
		flow.RegData = make(map[string]string)
	}
	h.renderFlowStep(w, r, flow)
}

// flowSubmitRegister handles the registration form submission.
func (h *Handler) flowSubmitRegister(w http.ResponseWriter, r *http.Request, flow *Flow, formData map[string]string) {
	h.mergeRegistrationData(flow, formData)

	if validationErrors := validateFlowRegistrationFields(flow.SchemaConfig.SchemaProps, flow.RegData); len(validationErrors) > 0 {
		h.respondWithFlowErrors(w, r, flow, validationErrors)
		return
	}

	identifier := resolveFlowRegistrationIdentifier(flow.SchemaConfig.SchemaProps, flow.RegData)
	displayName := flow.RegData["display_name"]
	if displayName == "" {
		displayName = identifier
	}

	schemaRec, ok := h.resolveFlowRegistrationSchema(w, r, flow)
	if !ok {
		return
	}

	payload, profileJSON, ok := h.buildFlowRegistrationPayload(w, r, flow, schemaRec.Schema, identifier, displayName)
	if !ok {
		return
	}

	newID := id.New()
	schemaID := schemaRec.ID
	orgID := httputil.ResolveOrgID(r, "1") // fallback to "1" for single-org mode

	tx, err := h.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		logging.Printf("[flow] %s registration tx failed: %v", flow.ID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration failed. Please try again."}})
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO users (id, org_id, identifier, display_name, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, 'active', ?, ?, datetime('now'), datetime('now'))`,
		newID, orgID, identifier, displayName, schemaID, profileJSON,
	)
	if err != nil {
		logging.Printf("[flow] %s registration failed: %v", flow.ID, err)
		if strings.Contains(err.Error(), "UNIQUE") || strings.Contains(err.Error(), "unique") {
			h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "already_exists", Message: "An account with this identifier already exists"}})
		} else {
			h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration failed. Please try again."}})
		}
		return
	}
	if err := uniqueness.EnforceFromIdentifier(r.Context(), tx, newID, orgID, identifier); err != nil {
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "already_exists", Message: "An account with this identifier already exists"}})
		return
	}
	if err := uniqueness.Enforce(r.Context(), tx, newID, orgID, uniqueness.ExtractConstraints(schemaRec.Schema), payload); err != nil {
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "already_exists", Message: "An account with this identifier already exists"}})
		return
	}
	if err := tx.Commit(); err != nil {
		logging.Printf("[flow] %s registration commit failed: %v", flow.ID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration failed. Please try again."}})
		return
	}

	logging.Printf("[flow] %s registered new identity %s (%s)", flow.ID, newID, identifier)

	// Set flow state to the new identity and complete.
	flow.IduserID = newID
	flow.Identifier = identifier
	flow.DisplayName = displayName
	flow.Verified = true
	flow.AuthMethod = "registration"
	flow.Errors = nil

	h.api.EmitAuthEvent(r.Context(), "auth.registration_completed", newID, map[string]any{
		"flow_id":    flow.ID,
		"identifier": identifier,
	})

	// Complete the flow (creates session, sets cookie).
	h.flowComplete(w, r, flow)
}

func (h *Handler) mergeRegistrationData(flow *Flow, formData map[string]string) {
	if flow.RegData == nil {
		flow.RegData = make(map[string]string)
	}
	for k, v := range formData {
		if k != "action" {
			flow.RegData[k] = v
		}
	}
}

func validateFlowRegistrationFields(fields []SchemaFieldDef, values map[string]string) []FlowError {
	var validationErrors []FlowError
	for _, field := range fields {
		if !field.Required {
			continue
		}
		if strings.TrimSpace(values[field.Name]) != "" {
			continue
		}
		label := field.Title
		if label == "" {
			label = humanize(field.Name)
		}
		validationErrors = append(validationErrors, FlowError{
			Code:    "field_required",
			Message: fmt.Sprintf("%s is required", label),
		})
	}
	return validationErrors
}

func resolveFlowRegistrationIdentifier(fields []SchemaFieldDef, values map[string]string) string {
	for _, field := range fields {
		if !field.Identifier {
			continue
		}
		if value := strings.TrimSpace(values[field.Name]); value != "" {
			return value
		}
	}
	if value := strings.TrimSpace(values["email"]); value != "" {
		return value
	}
	for _, value := range values {
		if trimmed := strings.TrimSpace(value); trimmed != "" {
			return trimmed
		}
	}
	return ""
}

func (h *Handler) resolveFlowRegistrationSchema(w http.ResponseWriter, r *http.Request, flow *Flow) (*schema.SchemaRecord, bool) {
	schemaID := flow.SchemaConfig.SchemaID
	if schemaID == "" {
		schemaRec, err := schema.ResolveDefaultHumanUserSchema(r.Context(), h.db.SQL())
		if err != nil {
			logging.Printf("[flow] %s default human user schema unavailable: %v", flow.ID, err)
			h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration is not available right now."}})
			return nil, false
		}
		return schemaRec, true
	}

	schemaRec, err := schema.LoadSchemaRecord(r.Context(), h.db.SQL(), schemaID)
	if err != nil {
		logging.Printf("[flow] %s failed to load schema %s: %v", flow.ID, schemaID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration is not available right now."}})
		return nil, false
	}
	return schemaRec, true
}

func (h *Handler) buildFlowRegistrationPayload(w http.ResponseWriter, r *http.Request, flow *Flow, schemaJSON, identifier, displayName string) (map[string]any, string, bool) {
	registrationData := make(map[string]any, len(flow.RegData))
	for key, value := range flow.RegData {
		registrationData[key] = value
	}
	payload := schema.MaterializeUserData(schemaJSON, identifier, displayName, registrationData)
	if err := schema.ValidateData(schemaJSON, payload); err != nil {
		logging.Printf("[flow] %s registration validation failed: %v", flow.ID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "invalid_registration", Message: "Please complete the highlighted fields and try again."}})
		return nil, "", false
	}

	profileJSON := "{}"
	if len(flow.RegData) > 0 {
		if b, err := json.Marshal(flow.RegData); err == nil {
			profileJSON = string(b)
		}
	}

	return payload, profileJSON, true
}

func (h *Handler) respondWithFlowErrors(w http.ResponseWriter, r *http.Request, flow *Flow, errs []FlowError) {
	flow.Errors = errs
	h.renderFlowStep(w, r, flow)
}

func (h *Handler) flowComplete(w http.ResponseWriter, r *http.Request, flow *Flow) {
	// Create session via the existing API.
	// Collect accumulated client signals from the flow.
	signals := &risk.Signals{
		CaptchaProvider: flow.CaptchaProvider,
		CaptchaVerified: flow.CaptchaVerified,
		CaptchaScore:    flow.CaptchaScore,
		PoWCompleted:    flow.PoWCompleted,
		PoWDurationMs:   flow.PoWDurationMs,
		VisitorID:       flow.VisitorID,
		FingerprintHash: flow.FingerprintHash,
		RequestID:       telemetry.RequestIDFromContext(r.Context()),
	}
	sessResp, err := h.api.CreateSessionForLogin(r.Context(), flow.IduserID, r.UserAgent(), r.RemoteAddr, signals, &SessionProvenance{
		AuthMethod:  flow.AuthMethod,
		LoginFlowID: flow.ID,
		AuthContext: map[string]any{
			"flow_id":         flow.ID,
			"trusted_session": flow.TrustedUserID != "",
			"trusted_reauth":  flow.TrustedUserID != "" && flow.TrustedUserID == flow.IduserID,
		},
	})
	if err != nil {
		flow.Errors = append(flow.Errors, FlowError{Code: "session_failed", Message: "Failed to create session. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	// Set session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, h.cookies)

	flow.transitionToStep(StepComplete)
	h.flows.Put(flow)

	logging.Printf("[flow] %s completed (identity=%s, session=%s)", flow.ID, flow.IduserID, sessResp.Session.ID)

	authCtx := telemetry.WithSessionID(r.Context(), sessResp.Session.ID)
	h.api.EmitAuthEvent(authCtx, "auth.login_completed", flow.IduserID, map[string]any{
		"session_id": sessResp.Session.ID,
		"flow_id":    flow.ID,
		"method":     flow.AuthMethod,
	})

	// Determine redirect URI: hand OIDC flows back to the provider callback,
	// otherwise fall back to the requested redirect or the console.
	redirectURI := "/console"
	if flow.AuthRequestID != "" {
		if err := h.completeOIDCAuthRequest(r.Context(), flow.AuthRequestID, flow.IduserID); err != nil {
			flow.Errors = append(flow.Errors, FlowError{Code: "oidc_complete_failed", Message: "OIDC login could not continue. Please try again."})
			h.renderFlowStep(w, r, flow)
			return
		}
		redirectURI = h.oidcAuthorizeCallbackURL(flow.AuthRequestID)
	} else if flow.RedirectURI != "" {
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

// handleFlowCaptchaChallenge generates an Altcha PoW challenge for the active flow.
// GET /v1/login/flows/{id}/captcha/challenge
func (h *Handler) handleFlowCaptchaChallenge(w http.ResponseWriter, r *http.Request) {
	flowID := r.PathValue("id")
	if flowID == "" {
		writeLoginError(w, http.StatusBadRequest, loginBadRequest("Missing flow ID."))
		return
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		writeLoginError(w, http.StatusNotFound, loginFlowNotFound("Login flow was not found or has expired."))
		return
	}

	r = r.WithContext(telemetry.WithFlowID(r.Context(), flowID))
	h.evaluatePreAuthRisk(r, flow)
	cc, _, active := configuredCaptchaForStep(flow)
	if !active || cc.Provider != "altcha" {
		httputil.WriteError(w, http.StatusBadRequest, "altcha captcha is not active for this step")
		return
	}

	challenge, err := h.altchaVerifierForConfig(flow.SchemaConfig).CreateChallenge()
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "failed to create challenge")
		return
	}
	httputil.WriteJSON(w, http.StatusOK, challenge)
}

// flowSubmitCaptcha handles the "captcha_submit" action.
func (h *Handler) flowSubmitCaptcha(w http.ResponseWriter, r *http.Request, flow *Flow, req map[string]string) {
	h.evaluatePreAuthRisk(r, flow)
	cc, scope, active := configuredCaptchaForStep(flow)
	if !active {
		flow.Errors = []FlowError{{Code: "captcha_not_required", Message: "Captcha is not required for this step."}}
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.Errors = nil
	flow.CaptchaProvider = cc.Provider

	var result *captcha.VerifyResult
	switch cc.Provider {
	case "altcha":
		payload := req["altcha_payload"]
		if payload == "" {
			flow.Errors = []FlowError{{Code: "captcha_missing", Message: "Captcha verification required."}}
			h.renderFlowStep(w, r, flow)
			return
		}
		result = captcha.VerifyAltcha(h.altchaVerifierForConfig(flow.SchemaConfig), payload)
	case "hcaptcha", "recaptcha", "turnstile":
		if cc.SiteKey == "" || cc.SecretKey == "" {
			logging.Printf("[flow] %s captcha provider %s is missing site_key or secret_key", flow.ID, cc.Provider)
			flow.clearCaptchaState()
			flow.Errors = []FlowError{{Code: "captcha_failed", Message: "Captcha is not configured correctly. Contact your administrator."}}
			h.renderFlowStep(w, r, flow)
			return
		}
		token := req["captcha_token"]
		if token == "" {
			flow.Errors = []FlowError{{Code: "captcha_missing", Message: "Captcha verification required."}}
			h.renderFlowStep(w, r, flow)
			return
		}
		var err error
		result, err = captcha.VerifyProviderToken(r.Context(), h.captchaHTTP, cc.Provider, cc.SecretKey, token, remoteIPFromAddr(r.RemoteAddr))
		if err != nil {
			logging.Printf("[flow] %s captcha verify failed for provider=%s: %v", flow.ID, cc.Provider, err)
			flow.clearCaptchaState()
			flow.Errors = []FlowError{{Code: "captcha_failed", Message: "Captcha verification failed. Please try again."}}
			h.renderFlowStep(w, r, flow)
			return
		}
	default:
		flow.clearCaptchaState()
		flow.Errors = []FlowError{{Code: "captcha_failed", Message: "Captcha provider is not supported."}}
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.CaptchaVerified = result.Valid
	if result.Valid {
		flow.CaptchaVerifiedScope = scope
	} else {
		flow.CaptchaVerifiedScope = ""
	}
	flow.CaptchaScore = result.Score
	flow.PoWCompleted = result.PoWCompleted
	flow.PoWDurationMs = result.PoWDurationMs

	if !result.Valid {
		flow.Errors = []FlowError{{Code: "captcha_failed", Message: "Captcha verification failed. Please try again."}}
	}

	h.renderFlowStep(w, r, flow)
}

// flowSubmitFingerprint handles the "fingerprint_submit" action.
// The client sends the ThumbmarkJS visitor ID after collecting it.
func (h *Handler) flowSubmitFingerprint(w http.ResponseWriter, r *http.Request, flow *Flow, req map[string]string) {
	flow.VisitorID = req["visitor_id"]
	flow.FingerprintHash = req["fingerprint_hash"]
	h.renderFlowStep(w, r, flow)
}

func firstNonEmptyString(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return value
		}
	}
	return ""
}
