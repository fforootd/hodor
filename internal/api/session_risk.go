package api

import (
	"context"

	"github.com/zitadel/zitadel/internal/login"
	"github.com/zitadel/zitadel/internal/risk"
	"github.com/zitadel/zitadel/internal/telemetry"
)

func hydrateSignalsFromContext(ctx context.Context, signals *risk.Signals) *risk.Signals {
	if signals == nil {
		signals = &risk.Signals{}
	} else {
		cloned := *signals
		signals = &cloned
	}

	if signals.RequestID == "" {
		signals.RequestID = telemetry.RequestIDFromContext(ctx)
	}
	if signals.VisitorID == "" {
		signals.VisitorID = telemetry.FingerprintFromContext(ctx)
	}
	if signals.FingerprintHash == "" {
		signals.FingerprintHash = signals.VisitorID
	}

	return signals
}

func buildRiskInput(stage risk.Stage, userID, userAgent, ipAddress string, signals *risk.Signals, provenance *login.SessionProvenance) risk.Input {
	input := risk.Input{
		Stage:     stage,
		UserID:    userID,
		UserAgent: userAgent,
		IPAddress: ipAddress,
	}
	if signals != nil {
		input.Signals = *signals
	}
	if provenance == nil {
		return input
	}

	input.AuthMethod = provenance.AuthMethod
	input.ProviderID = provenance.ProviderID
	input.ProviderKind = provenance.ProviderKind
	input.LoginFlowID = provenance.LoginFlowID
	if provenance.AuthContext != nil {
		input.TrustedSession = boolOr(provenance.AuthContext["trusted_session"])
		input.Reauth = boolOr(provenance.AuthContext["trusted_reauth"])
	}

	return input
}

func riskMetadata(result *risk.Result, signals *risk.Signals) map[string]any {
	if result == nil {
		return map[string]any{
			"level":                 string(risk.LevelUnknown),
			"recommended_next_step": string(risk.RecommendationAllowAndLog),
			"stage":                 string(risk.StagePostAuth),
			"evaluator_version":     risk.EvaluatorVersion,
		}
	}

	return map[string]any{
		"score":                 result.Score,
		"level":                 string(result.Level),
		"reasons":               result.Reasons,
		"recommended_next_step": string(result.RecommendedNextStep),
		"stage":                 string(result.Stage),
		"evaluator_version":     result.EvaluatorVersion,
		"signals":               riskSignalSummary(signals),
	}
}

func riskSignalSummary(signals *risk.Signals) map[string]any {
	if signals == nil {
		return map[string]any{}
	}

	return map[string]any{
		"captcha_verified": signals.CaptchaVerified,
		"captcha_provider": signals.CaptchaProvider,
		"pow_completed":    signals.PoWCompleted,
		"pow_duration_ms":  signals.PoWDurationMs,
		"request_id":       signals.RequestID,
		"visitor_id":       signals.VisitorID,
	}
}

func riskEventPayload(result *risk.Result, policyName, policyVersion string) map[string]any {
	payload := map[string]any{
		"policy_name":    policyName,
		"policy_version": policyVersion,
	}
	if result == nil {
		return payload
	}

	payload["score"] = result.Score
	payload["level"] = string(result.Level)
	payload["reasons"] = result.Reasons
	payload["recommended_next_step"] = string(result.RecommendedNextStep)
	payload["stage"] = string(result.Stage)
	payload["evaluator_version"] = result.EvaluatorVersion
	return payload
}

func provenanceValue(provenance *login.SessionProvenance, key string) any {
	if provenance == nil {
		return nil
	}
	switch key {
	case "login_flow_id":
		return provenance.LoginFlowID
	default:
		return nil
	}
}

func boolOr(value any) bool {
	b, _ := value.(bool)
	return b
}
