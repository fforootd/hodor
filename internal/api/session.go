package api

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"time"

	"github.com/zitadel/zitadel/internal/id"
)

// --- Session types ---

type SessionResponse struct {
	ID         int64  `json:"id,string"`
	IdentityID int64  `json:"identity_id,string"`
	OrgID      int64  `json:"org_id,string"`
	UserAgent  string `json:"user_agent,omitempty"`
	IPAddress  string `json:"ip_address,omitempty"`
	CreatedAt  string `json:"created_at"`
	ExpiresAt  string `json:"expires_at"`
}

type CreateSessionRequest struct {
	IdentityID int64  `json:"identity_id"`
	UserAgent  string `json:"user_agent,omitempty"`
	IPAddress  string `json:"ip_address,omitempty"`
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
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}
	if req.IdentityID == 0 {
		writeError(w, http.StatusBadRequest, "identity_id is required")
		return
	}

	resp, err := a.CreateSessionInternal(r.Context(), req.IdentityID, req.UserAgent, req.IPAddress)
	if err != nil {
		writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	writeJSON(w, http.StatusCreated, resp)
}

// CreateSessionInternal creates a session programmatically (used by UI login).
func (a *API) CreateSessionInternal(ctx context.Context, identityID int64, userAgent, ipAddress string) (*CreateSessionResponse, error) {
	sessionID, err := id.New()
	if err != nil {
		return nil, fmt.Errorf("generate id: %w", err)
	}

	rawToken, tokenHash, err := generateToken()
	if err != nil {
		return nil, err
	}

	now := time.Now().UTC()
	expiresAt := now.Add(24 * time.Hour)

	tx, err := a.db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	// Verify identity exists.
	var exists int
	err = tx.QueryRowContext(ctx, `SELECT 1 FROM identities WHERE id = ?`, identityID).Scan(&exists)
	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("identity %d not found", identityID)
	}
	if err != nil {
		return nil, fmt.Errorf("check identity: %w", err)
	}

	_, err = tx.ExecContext(ctx,
		`INSERT INTO sessions (id, identity_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at)
		 VALUES (?, ?, 1, ?, ?, ?, '{}', ?, ?)`,
		sessionID, identityID, tokenHash,
		userAgent, ipAddress,
		now.Format(time.RFC3339), expiresAt.Format(time.RFC3339),
	)
	if err != nil {
		return nil, fmt.Errorf("insert session: %w", err)
	}

	emitEvent(ctx, tx, "session.created", identityID, sessionID, "session", map[string]any{
		"identity_id": identityID,
		"user_agent":  userAgent,
		"ip_address":  ipAddress,
	})

	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit: %w", err)
	}

	a.bus.Signal()

	return &CreateSessionResponse{
		Session: SessionResponse{
			ID:         sessionID,
			IdentityID: identityID,
			OrgID:      1,
			UserAgent:  userAgent,
			IPAddress:  ipAddress,
			CreatedAt:  now.Format(time.RFC3339),
			ExpiresAt:  expiresAt.Format(time.RFC3339),
		},
		Token: rawToken,
	}, nil
}

func (a *API) getSession(w http.ResponseWriter, r *http.Request) {
	sessionID, err := parseID(r, "id")
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid id")
		return
	}

	sess, err := a.loadSession(r.Context(), sessionID)
	if err != nil {
		writeError(w, http.StatusNotFound, "session not found")
		return
	}

	writeJSON(w, http.StatusOK, sess)
}

func (a *API) listSessions(w http.ResponseWriter, r *http.Request) {
	identityID, _ := strconv.ParseInt(r.URL.Query().Get("identity_id"), 10, 64)
	limit := 50

	query := `SELECT id, identity_id, org_id, user_agent, ip_address, created_at, expires_at
	          FROM sessions WHERE revoked_at IS NULL ORDER BY created_at DESC LIMIT ?`
	args := []any{limit}
	if identityID > 0 {
		query = `SELECT id, identity_id, org_id, user_agent, ip_address, created_at, expires_at
		         FROM sessions WHERE identity_id = ? AND revoked_at IS NULL ORDER BY created_at DESC LIMIT ?`
		args = []any{identityID, limit}
	}

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var sessions []SessionResponse
	for rows.Next() {
		var s SessionResponse
		rows.Scan(&s.ID, &s.IdentityID, &s.OrgID, &s.UserAgent, &s.IPAddress, &s.CreatedAt, &s.ExpiresAt)
		sessions = append(sessions, s)
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, "rows error")
		return
	}

	writeJSON(w, http.StatusOK, ListResponse{Items: sessions})
}

func (a *API) revokeSession(w http.ResponseWriter, r *http.Request) {
	sessionID, err := parseID(r, "id")
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid id")
		return
	}

	if err := a.RevokeSessionInternal(r.Context(), sessionID); err != nil {
		writeError(w, http.StatusNotFound, err.Error())
		return
	}

	w.WriteHeader(http.StatusNoContent)
}

// RevokeSessionInternal revokes a session programmatically (used by UI logout).
func (a *API) RevokeSessionInternal(ctx context.Context, sessionID int64) error {
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
		return fmt.Errorf("session %d not found or already revoked", sessionID)
	}

	var revokedIdentityID int64
	tx.QueryRowContext(ctx, `SELECT identity_id FROM sessions WHERE id = ?`, sessionID).Scan(&revokedIdentityID)

	emitEvent(ctx, tx, "session.revoked", revokedIdentityID, sessionID, "session", map[string]any{
		"identity_id": revokedIdentityID,
		"reason":      "api_revoke",
	})

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	a.bus.Signal()
	return nil
}

func (a *API) loadSession(ctx context.Context, sessionID int64) (SessionResponse, error) {
	var s SessionResponse
	err := a.db.SQL().QueryRowContext(ctx,
		`SELECT id, identity_id, org_id, user_agent, ip_address, created_at, expires_at
		 FROM sessions WHERE id = ? AND revoked_at IS NULL`, sessionID,
	).Scan(&s.ID, &s.IdentityID, &s.OrgID, &s.UserAgent, &s.IPAddress, &s.CreatedAt, &s.ExpiresAt)
	return s, err
}
