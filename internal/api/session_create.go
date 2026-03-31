package api

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/login"
	"github.com/zitadel/zitadel/internal/risk"
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
	metadataJSON, _ := json.Marshal(metadata)

	tx, err := a.db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	var exists int
	err = tx.QueryRowContext(ctx, `SELECT 1 FROM users WHERE id = ?`, userID).Scan(&exists)
	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("identity %s not found", userID)
	}
	if err != nil {
		return nil, fmt.Errorf("check identity: %w", err)
	}

	sessionFingerprint := ""
	if effectiveSignals != nil {
		sessionFingerprint = effectiveSignals.VisitorID
	}

	_, err = tx.ExecContext(ctx,
		`INSERT INTO sessions (id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at, fingerprint)
		 VALUES (?, ?, '_global', ?, ?, ?, ?, ?, ?, ?)`,
		sessionID, userID, tokenHash,
		userAgent, ipAddress, string(metadataJSON),
		now.Format(time.RFC3339), expiresAt.Format(time.RFC3339), sessionFingerprint,
	)
	if err != nil {
		return nil, fmt.Errorf("insert session: %w", err)
	}

	tokenID := id.New()
	_, err = tx.ExecContext(ctx,
		`INSERT INTO tokens (id, type, token_hash, user_id, session_id, scopes, expires_at, created_at)
		 VALUES (?, 'session', ?, ?, ?, '[]', ?, ?)`,
		tokenID, tokenHash, userID, sessionID,
		expiresAt.Format(time.RFC3339), now.Format(time.RFC3339),
	)
	if err != nil {
		return nil, fmt.Errorf("insert token: %w", err)
	}

	ctxWithSession := telemetry.WithSessionID(ctx, sessionID)
	if sessionFingerprint != "" {
		ctxWithSession = telemetry.WithFingerprint(ctxWithSession, sessionFingerprint)
	}

	emitEvent(ctxWithSession, tx, "session.created", userID, sessionID, "session", map[string]any{
		"user_id":       userID,
		"user_agent":    userAgent,
		"ip_address":    ipAddress,
		"auth_method":   metadata["auth_method"],
		"provider_id":   metadata["provider_id"],
		"provider_kind": metadata["provider_kind"],
		"login_flow_id": metadata["login_flow_id"],
		"auth_context":  metadata["auth_context"],
	})
	emitEvent(ctxWithSession, tx, "signal.risk_evaluated", userID, sessionID, "session", riskEventPayload(riskResult, "builtin_post_auth_advisory_v1", "v1"))

	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit: %w", err)
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
		Session: SessionResponse{
			ID:           sessionID,
			IduserID:     userID,
			OrgID:        "",
			AuthMethod:   stringOr(metadata["auth_method"]),
			ProviderID:   stringOr(metadata["provider_id"]),
			ProviderKind: stringOr(metadata["provider_kind"]),
			LoginFlowID:  stringOr(metadata["login_flow_id"]),
			Metadata:     metadata,
			UserAgent:    userAgent,
			IPAddress:    ipAddress,
			CreatedAt:    now.Format(time.RFC3339),
			ExpiresAt:    expiresAt.Format(time.RFC3339),
		},
		Token: rawToken,
	}, nil
}
