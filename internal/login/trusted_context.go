package login

import (
	"context"
	"database/sql"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/session"
)

func (h *Handler) resolveTrustedUserID(r *http.Request, oidcState string) (string, bool) {
	if userID, ok := h.resolveTrustedUserIDFromRequest(r); ok {
		return userID, true
	}
	if oidcState == "" {
		return "", false
	}
	return h.resolveTrustedUserIDFromOIDCState(r.Context(), oidcState)
}

func (h *Handler) resolveTrustedUserIDFromRequest(r *http.Request) (string, bool) {
	rawToken := extractBearerToken(r)
	if rawToken == "" && h.cookies != nil {
		if token, ok := session.ReadSessionCookie(r, h.cookies); ok {
			rawToken = token
		}
	}
	if rawToken == "" {
		return "", false
	}
	return h.resolveTrustedUserIDFromToken(r.Context(), rawToken)
}

func extractBearerToken(r *http.Request) string {
	authHeader := r.Header.Get("Authorization")
	if !strings.HasPrefix(authHeader, "Bearer ") {
		return ""
	}
	return strings.TrimSpace(strings.TrimPrefix(authHeader, "Bearer "))
}

func (h *Handler) resolveTrustedUserIDFromOIDCState(ctx context.Context, oidcState string) (string, bool) {
	var userID string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT user_id
		 FROM auth_states
		 WHERE type = 'oidc_auth'
		   AND state = ?
		   AND user_id != ''
		   AND expires_at > datetime('now')
		 ORDER BY created_at DESC
		 LIMIT 1`,
		oidcState,
	).Scan(&userID)
	if err != nil || userID == "" {
		return "", false
	}
	return userID, true
}

func (h *Handler) resolveTrustedUserIDFromToken(ctx context.Context, rawToken string) (string, bool) {
	hash := crypto.HashTokenHex(rawToken)

	var (
		userID    string
		tokenType string
		sessionID sql.NullString
	)
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT user_id, type, session_id
		 FROM tokens
		 WHERE token_hash = ?
		   AND revoked_at IS NULL
		   AND (expires_at IS NULL OR expires_at > datetime('now'))
		 LIMIT 1`,
		hash,
	).Scan(&userID, &tokenType, &sessionID)
	if err == nil && userID != "" {
		if tokenType != "session" {
			return userID, true
		}
		if sessionID.Valid && sessionID.String != "" {
			if h.sessionIsActive(ctx, sessionID.String) {
				_, _ = h.db.SQL().ExecContext(ctx, `UPDATE tokens SET last_used = ? WHERE token_hash = ?`, time.Now().UTC().Format(time.RFC3339), hash)
				return userID, true
			}
			return "", false
		}
	}

	if h.legacySessionIsActive(ctx, hash, &userID) {
		return userID, true
	}

	return "", false
}

func (h *Handler) sessionIsActive(ctx context.Context, sessionID string) bool {
	var active string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT id
		 FROM sessions
		 WHERE id = ?
		   AND revoked_at IS NULL
		   AND expires_at > datetime('now')`,
		sessionID,
	).Scan(&active)
	return err == nil && active != ""
}

func (h *Handler) legacySessionIsActive(ctx context.Context, hash string, userID *string) bool {
	var sessionID string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT user_id, id
		 FROM sessions
		 WHERE token_hash = ?
		   AND revoked_at IS NULL
		   AND expires_at > datetime('now')`,
		hash,
	).Scan(userID, &sessionID)
	return err == nil && *userID != "" && sessionID != ""
}
