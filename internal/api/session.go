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

// --- Session types ---

type SessionResponse struct {
	ID           string         `json:"id"`
	IduserID     string         `json:"user_id"`
	OrgID        string         `json:"org_id"`
	AuthMethod   string         `json:"auth_method,omitempty"`
	ProviderID   string         `json:"provider_id,omitempty"`
	ProviderKind string         `json:"provider_kind,omitempty"`
	LoginFlowID  string         `json:"login_flow_id,omitempty"`
	Metadata     map[string]any `json:"metadata,omitempty"`
	UserAgent    string         `json:"user_agent,omitempty"`
	IPAddress    string         `json:"ip_address,omitempty"`
	CreatedAt    string         `json:"created_at"`
	ExpiresAt    string         `json:"expires_at"`
	RevokedAt    *string        `json:"revoked_at,omitempty"`
}

type CreateSessionRequest struct {
	IduserID  string `json:"user_id"`
	UserAgent string `json:"user_agent,omitempty"`
	IPAddress string `json:"ip_address,omitempty"`
}

type CreateSessionResponse struct {
	Session SessionResponse `json:"session"`
	Token   string          `json:"token"`
}

// RegisterSessionRoutes mounts session-related REST routes.
func (a *API) RegisterSessionRoutes(mux *http.ServeMux, requireAdmin func(http.HandlerFunc) http.HandlerFunc) {
	mux.HandleFunc("POST /v1/sessions", requireAdmin(a.createSession))
	mux.HandleFunc("GET /v1/sessions", requireAdmin(a.listSessions))
	mux.HandleFunc("GET /v1/sessions/{id}", requireAdmin(a.getSession))
	mux.HandleFunc("POST /v1/sessions/{id}/revoke", requireAdmin(a.revokeSession))
}

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

	// Build metadata JSON with structured risk and signal summaries.
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

	// Verify identity exists.
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

	// Insert session (metadata record).
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

	// Insert into unified tokens table.
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

	// FGA: write session tuples — truly best-effort, async with timeout.
	// Sessions expire naturally so missing tuples are harmless.
	if svc := FGAService; svc != nil {
		go func() { //nolint:gosec,contextcheck // intentional — FGA write must outlive request
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

func (a *API) getSession(w http.ResponseWriter, r *http.Request) {
	sessionID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	sess, err := a.loadSession(r.Context(), sessionID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "session not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, sess)
}

func (a *API) listSessions(w http.ResponseWriter, r *http.Request) {
	userID, _ := r.URL.Query().Get("user_id"), ""
	limit := 50

	query := `SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
	          FROM sessions ORDER BY created_at DESC LIMIT ?`
	args := []any{limit}
	if userID != "" {
		query = `SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
		         FROM sessions WHERE user_id = ? ORDER BY created_at DESC LIMIT ?`
		args = []any{userID, limit}
	}

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var sessions []SessionResponse
	for rows.Next() {
		var s SessionResponse
		var metadataJSON string
		rows.Scan(&s.ID, &s.IduserID, &s.OrgID, &s.UserAgent, &s.IPAddress, &metadataJSON, &s.CreatedAt, &s.ExpiresAt, &s.RevokedAt)
		applySessionMetadata(&s, metadataJSON)
		sessions = append(sessions, s)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: sessions})
}

func (a *API) revokeSession(w http.ResponseWriter, r *http.Request) {
	sessionID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	if err := a.RevokeSessionInternal(r.Context(), sessionID); err != nil {
		httputil.WriteError(w, http.StatusNotFound, err.Error())
		return
	}

	w.WriteHeader(http.StatusNoContent)
}

// RevokeSessionInternal revokes a session programmatically (used by UI logout).
func (a *API) RevokeSessionInternal(ctx context.Context, sessionID string) error {
	tx, err := a.db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	now := time.Now().UTC().Format(time.RFC3339)
	result, err := tx.ExecContext(ctx,
		`UPDATE sessions SET revoked_at = ? WHERE id = ? AND revoked_at IS NULL`,
		now, sessionID)
	if err != nil {
		return fmt.Errorf("revoke: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		return fmt.Errorf("session %s not found or already revoked", sessionID)
	}

	// Also revoke all tokens associated with this session.
	tx.ExecContext(ctx,
		`UPDATE tokens SET revoked_at = ? WHERE session_id = ? AND revoked_at IS NULL`,
		now, sessionID)

	var revokedIduserID string
	tx.QueryRowContext(ctx, `SELECT user_id FROM sessions WHERE id = ?`, sessionID).Scan(&revokedIduserID)

	emitEvent(ctx, tx, "session.revoked", revokedIduserID, sessionID, "session", map[string]any{
		"user_id": revokedIduserID,
		"reason":  "api_revoke",
	})

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	a.bus.Signal()
	return nil
}

func (a *API) loadSession(ctx context.Context, sessionID string) (SessionResponse, error) {
	var s SessionResponse
	var metadataJSON string
	err := a.db.SQL().QueryRowContext(ctx,
		`SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
		 FROM sessions WHERE id = ?`, sessionID,
	).Scan(&s.ID, &s.IduserID, &s.OrgID, &s.UserAgent, &s.IPAddress, &metadataJSON, &s.CreatedAt, &s.ExpiresAt, &s.RevokedAt)
	applySessionMetadata(&s, metadataJSON)
	return s, err
}

func applySessionMetadata(sess *SessionResponse, metadataJSON string) {
	if sess == nil {
		return
	}
	var metadata map[string]any
	if err := json.Unmarshal([]byte(metadataJSON), &metadata); err != nil {
		return
	}
	sess.Metadata = metadata
	sess.AuthMethod = stringOr(metadata["auth_method"])
	sess.ProviderID = stringOr(metadata["provider_id"])
	sess.ProviderKind = stringOr(metadata["provider_kind"])
	sess.LoginFlowID = stringOr(metadata["login_flow_id"])
}

func stringOr(value any) string {
	if str, ok := value.(string); ok {
		return str
	}
	return ""
}
