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
)

// --- Session types ---

type SessionResponse struct {
	ID        string  `json:"id"`
	IduserID  string  `json:"user_id"`
	OrgID     string  `json:"org_id"`
	UserAgent string  `json:"user_agent,omitempty"`
	IPAddress string  `json:"ip_address,omitempty"`
	CreatedAt string  `json:"created_at"`
	ExpiresAt string  `json:"expires_at"`
	RevokedAt *string `json:"revoked_at,omitempty"`
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

// ClientSignals contains client-side signals collected during a login flow.
// These are used for risk scoring and session metadata enrichment.
type ClientSignals struct {
	// Captcha (Altcha or third-party)
	CaptchaProvider string  `json:"captcha_provider,omitempty"`
	CaptchaVerified bool    `json:"captcha_verified,omitempty"`
	CaptchaScore    float64 `json:"captcha_score,omitempty"`
	PoWCompleted    bool    `json:"pow_completed,omitempty"`
	PoWDurationMs   float64 `json:"pow_duration_ms,omitempty"`

	// Fingerprint (ThumbmarkJS)
	VisitorID       string         `json:"visitor_id,omitempty"`
	FingerprintHash string         `json:"fingerprint_hash,omitempty"`
	BrowserSignals  map[string]any `json:"browser_signals,omitempty"`

	// Telemetry
	RequestID        string  `json:"request_id,omitempty"`
	DocumentLoadMs   float64 `json:"document_load_ms,omitempty"`
	InteractionCount int     `json:"interaction_count,omitempty"`
}

// computeRiskLevel derives a risk level from collected client signals.
func computeRiskLevel(signals *ClientSignals) string {
	if signals == nil {
		return "unknown"
	}

	score := 0.0

	// PoW completed → good signal (bots struggle with memory-bound PoW).
	if signals.PoWCompleted {
		score += 0.3
	}
	// PoW took realistic time (not instant = not pre-computed).
	if signals.PoWDurationMs > 100 && signals.PoWDurationMs < 30000 {
		score += 0.1
	}
	// Fingerprint present → browser has canvas, webgl, audio APIs.
	if signals.VisitorID != "" {
		score += 0.2
	}
	// Request ID present → real page load happened.
	if signals.RequestID != "" {
		score += 0.1
	}
	// Realistic document load time.
	if signals.DocumentLoadMs > 200 {
		score += 0.1
	}
	// Captcha verified.
	if signals.CaptchaVerified {
		score += 0.2
	}

	switch {
	case score >= 0.7:
		return "low" // likely human
	case score >= 0.4:
		return "medium" // suspicious, may trigger MFA
	default:
		return "high" // likely bot, may block
	}
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

	resp, err := a.CreateSessionInternal(r.Context(), req.IduserID, req.UserAgent, req.IPAddress, nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, err.Error())
		return
	}

	httputil.WriteJSON(w, http.StatusCreated, resp)
}

// CreateSessionInternal creates a session programmatically (used by UI login).
// signals may be nil for legacy callers.
func (a *API) CreateSessionInternal(ctx context.Context, userID string, userAgent, ipAddress string, signals *ClientSignals) (*CreateSessionResponse, error) {
	sessionID := id.New()

	rawToken, tokenHash, err := generatePrefixedToken(PrefixSession)
	if err != nil {
		return nil, err
	}

	now := time.Now().UTC()
	expiresAt := now.Add(24 * time.Hour)

	// Compute risk level from client signals.
	riskLevel := computeRiskLevel(signals)

	// Build metadata JSON with risk level.
	metadata := map[string]any{"risk_level": riskLevel}
	if signals != nil {
		if signals.CaptchaProvider != "" {
			metadata["captcha"] = map[string]any{
				"provider": signals.CaptchaProvider,
				"verified": signals.CaptchaVerified,
				"score":    signals.CaptchaScore,
				"pow":      signals.PoWCompleted,
			}
		}
		if signals.VisitorID != "" {
			metadata["fingerprint"] = map[string]any{
				"visitor_id": signals.VisitorID,
			}
		}
		if signals.RequestID != "" {
			metadata["telemetry"] = map[string]any{
				"request_id": signals.RequestID,
			}
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

	// Insert session (metadata record).
	_, err = tx.ExecContext(ctx,
		`INSERT INTO sessions (id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at)
		 VALUES (?, ?, '_global', ?, ?, ?, ?, ?, ?)`,
		sessionID, userID, tokenHash,
		userAgent, ipAddress, string(metadataJSON),
		now.Format(time.RFC3339), expiresAt.Format(time.RFC3339),
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

	emitEvent(ctx, tx, "session.created", userID, sessionID, "session", map[string]any{
		"user_id":    userID,
		"user_agent": userAgent,
		"ip_address": ipAddress,
	})

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
			ID:        sessionID,
			IduserID:  userID,
			OrgID:     "",
			UserAgent: userAgent,
			IPAddress: ipAddress,
			CreatedAt: now.Format(time.RFC3339),
			ExpiresAt: expiresAt.Format(time.RFC3339),
		},
		Token: rawToken,
	}, nil
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

	query := `SELECT id, user_id, org_id, user_agent, ip_address, created_at, expires_at, revoked_at
	          FROM sessions ORDER BY created_at DESC LIMIT ?`
	args := []any{limit}
	if userID != "" {
		query = `SELECT id, user_id, org_id, user_agent, ip_address, created_at, expires_at, revoked_at
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
		rows.Scan(&s.ID, &s.IduserID, &s.OrgID, &s.UserAgent, &s.IPAddress, &s.CreatedAt, &s.ExpiresAt, &s.RevokedAt)
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
	err := a.db.SQL().QueryRowContext(ctx,
		`SELECT id, user_id, org_id, user_agent, ip_address, created_at, expires_at, revoked_at
		 FROM sessions WHERE id = ?`, sessionID,
	).Scan(&s.ID, &s.IduserID, &s.OrgID, &s.UserAgent, &s.IPAddress, &s.CreatedAt, &s.ExpiresAt, &s.RevokedAt)
	return s, err
}
