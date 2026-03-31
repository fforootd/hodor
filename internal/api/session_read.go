package api

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
)

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
