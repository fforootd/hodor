package api

import (
	"context"
	"encoding/json"
	"net/http"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/login"
	"github.com/zitadel/zitadel/internal/risk"
	sessionsvc "github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/telemetry"
)

func (a *API) createSession(w http.ResponseWriter, r *http.Request) {
	var req CreateSessionRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.IduserID == "" {
		httputil.WriteError(w, http.StatusBadRequest, "user_id is required")
		return
	}

	resp, err := a.CreateSessionInternal(r.Context(), req.IduserID, req.UserAgent, req.IPAddress, nil, nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, err.Error())
		return
	}

	httputil.WriteJSON(w, http.StatusCreated, resp)
}

// CreateSessionInternal creates a session programmatically (used by UI login).
// signals may be nil for legacy callers.
func (a *API) CreateSessionInternal(ctx context.Context, userID string, userAgent, ipAddress string, signals *risk.Signals, provenance *login.SessionProvenance) (*CreateSessionResponse, error) {
	sessionID := id.New()

	rawToken, tokenHash, err := generatePrefixedToken(PrefixSession)
	if err != nil {
		return nil, err
	}

	now := time.Now().UTC()
	expiresAt := now.Add(24 * time.Hour)

	effectiveSignals := hydrateSignalsFromContext(ctx, signals)
	riskResult := risk.FailureResult(risk.StagePostAuth, risk.RecommendationAllowAndLog)
	if a.risk != nil {
		evaluatedRisk, evalErr := a.risk.Evaluate(ctx, buildRiskInput(risk.StagePostAuth, userID, userAgent, ipAddress, effectiveSignals, provenance))
		if evalErr != nil {
			logging.Printf("[risk] post-auth evaluation failed user=%s flow=%s: %v", userID, stringOr(provenanceValue(provenance, "login_flow_id")), evalErr)
		} else {
			riskResult = evaluatedRisk
		}
	}

	metadata := map[string]any{
		"risk_level": string(riskResult.Level),
		"risk":       riskMetadata(riskResult, effectiveSignals),
	}
	if effectiveSignals != nil {
		if effectiveSignals.CaptchaProvider != "" {
			metadata["captcha"] = map[string]any{
				"provider": effectiveSignals.CaptchaProvider,
				"verified": effectiveSignals.CaptchaVerified,
				"score":    effectiveSignals.CaptchaScore,
				"pow":      effectiveSignals.PoWCompleted,
			}
		}
		if effectiveSignals.VisitorID != "" {
			metadata["fingerprint"] = map[string]any{
				"visitor_id": effectiveSignals.VisitorID,
			}
		}
		if effectiveSignals.RequestID != "" {
			metadata["telemetry"] = map[string]any{
				"request_id": effectiveSignals.RequestID,
			}
		}
	}
	if provenance != nil {
		if provenance.AuthMethod != "" {
			metadata["auth_method"] = provenance.AuthMethod
		}
		if provenance.ProviderID != "" {
			metadata["provider_id"] = provenance.ProviderID
		}
		if provenance.ProviderKind != "" {
			metadata["provider_kind"] = provenance.ProviderKind
		}
		if provenance.LoginFlowID != "" {
			metadata["login_flow_id"] = provenance.LoginFlowID
		}
		if len(provenance.AuthContext) > 0 {
			metadata["auth_context"] = provenance.AuthContext
		}
	}
	sessionFingerprint := ""
	if effectiveSignals != nil {
		sessionFingerprint = effectiveSignals.VisitorID
	}

	ctxWithSession := telemetry.WithSessionID(ctx, sessionID)
	if sessionFingerprint != "" {
		ctxWithSession = telemetry.WithFingerprint(ctxWithSession, sessionFingerprint)
	}

	tokenID := id.New()
	record, err := a.sessionStore.Create(ctxWithSession, sessionsvc.CreateParams{
		SessionID:   sessionID,
		TokenID:     tokenID,
		UserID:      userID,
		OrgID:       "_global",
		TokenHash:   tokenHash,
		UserAgent:   userAgent,
		IPAddress:   ipAddress,
		Fingerprint: sessionFingerprint,
		Metadata:    metadata,
		CreatedAt:   now,
		ExpiresAt:   expiresAt,
		SessionCreatedPayload: map[string]any{
			"user_id":       userID,
			"user_agent":    userAgent,
			"ip_address":    ipAddress,
			"auth_method":   metadata["auth_method"],
			"provider_id":   metadata["provider_id"],
			"provider_kind": metadata["provider_kind"],
			"login_flow_id": metadata["login_flow_id"],
			"auth_context":  metadata["auth_context"],
		},
		RiskEvaluatedPayload: riskEventPayload(riskResult, "builtin_post_auth_advisory_v1", "v1"),
	})
	if err != nil {
		return nil, err
	}

	if svc := FGAService; svc != nil {
		go func() {
			fgaCtx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			if err := svc.OnSessionCreated(fgaCtx, sessionID, userID, "_global"); err != nil {
				logging.Printf("[fga] warn: session tuple write failed (non-blocking): %v", err)
			}
		}()
	}

	a.bus.Signal()

	return &CreateSessionResponse{
		Session: sessionResponseFromRecord(record),
		Token:   rawToken,
	}, nil
}
