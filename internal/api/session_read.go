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

	scoped := a.db.Scoped(r.Context())
	instanceID := scoped.InstanceID()

	query := `SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
	          FROM sessions WHERE instance_id = ? ORDER BY created_at DESC LIMIT ?`
	args := []any{instanceID, limit}
	if userID != "" {
		query = `SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
		         FROM sessions WHERE instance_id = ? AND user_id = ? ORDER BY created_at DESC LIMIT ?`
		args = []any{instanceID, userID, limit}
	}

	rows, err := scoped.QueryContext(r.Context(), query, args...)
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
	scoped := a.db.Scoped(ctx)
	instanceID := scoped.InstanceID()
	tx, err := scoped.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	now := time.Now().UTC().Format(time.RFC3339)
	result, err := tx.ExecContext(ctx,
		`UPDATE sessions SET revoked_at = ? WHERE id = ? AND instance_id = ? AND revoked_at IS NULL`,
		now, sessionID, instanceID)
	if err != nil {
		return fmt.Errorf("revoke: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		return fmt.Errorf("session %s not found or already revoked", sessionID)
	}

	tx.ExecContext(ctx,
		`UPDATE tokens SET revoked_at = ? WHERE session_id = ? AND instance_id = ? AND revoked_at IS NULL`,
		now, sessionID, instanceID)

	var revokedIduserID string
	tx.QueryRowContext(ctx, `SELECT user_id FROM sessions WHERE id = ? AND instance_id = ?`, sessionID, instanceID).Scan(&revokedIduserID)

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
	scoped := a.db.Scoped(ctx)
	instanceID := scoped.InstanceID()
	var s SessionResponse
	var metadataJSON string
	err := scoped.QueryRowContext(ctx,
		`SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
		 FROM sessions WHERE id = ? AND instance_id = ?`, sessionID, instanceID,
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
