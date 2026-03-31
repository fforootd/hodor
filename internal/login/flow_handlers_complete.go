package login

import (
	"net/http"

	"github.com/zitadel/zitadel/internal/captcha"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/risk"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
)

func (h *Handler) flowComplete(w http.ResponseWriter, r *http.Request, flow *Flow) {
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

	h.flows.Delete(flow.ID)
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
func (h *Handler) flowSubmitFingerprint(w http.ResponseWriter, r *http.Request, flow *Flow, req map[string]string) {
	flow.VisitorID = req["visitor_id"]
	flow.FingerprintHash = req["fingerprint_hash"]
	h.renderFlowStep(w, r, flow)
}
