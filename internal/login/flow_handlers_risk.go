package login

import (
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/risk"
	"github.com/zitadel/zitadel/internal/telemetry"
)

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

func firstNonEmptyString(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return value
		}
	}
	return ""
}
