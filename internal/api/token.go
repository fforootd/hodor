package api

import (
	"context"
	"database/sql"
	"fmt"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/httputil"
)

// Token type constants.
const (
	TokenTypeSession = "session"
	TokenTypePAT     = "pat"
	TokenTypeOpaque  = "opaque"

	// Token prefix constants — Stripe-style identifiable prefixes.
	PrefixSession = "zit_ses_"
	PrefixPAT     = "zit_pat_"
	PrefixOpaque  = "zit_opq_"
)

// TokenInfo carries the resolved identity of a token holder.
type TokenInfo struct {
	UserID    string   // The identity this token belongs to ("" if nullable)
	SessionID string   // Only for session tokens
	TokenType string   // "session", "pat", "opaque"
	OrgID     string   // The org_id from the entity
	Scopes    []string // Future: fine-grained scopes
}

// generatePrefixedToken creates a new random token with the given prefix.
// Returns (rawToken, sha256Hash, error).
// Token format: {prefix}{64 hex chars} — total length = len(prefix) + 64.
func generatePrefixedToken(prefix string) (raw string, hash string, err error) {
	hexPart, err := crypto.RandomHex(32)
	if err != nil {
		return "", "", fmt.Errorf("generate token: %w", err)
	}
	raw = prefix + hexPart
	hash = crypto.HashTokenHex(raw)
	return raw, hash, nil
}

// hashToken returns the SHA-256 hex digest of a raw token string.
func hashToken(raw string) string {
	return crypto.HashTokenHex(raw)
}

// resolveToken looks up a raw token in the database and returns its TokenInfo.
// It uses the token prefix to determine the resolution path:
//   - zit_ses_ → session token (validates via sessions + tokens tables)
//   - zit_pat_ → personal access token (validates via tokens table)
//   - zit_opq_ → opaque token (validates via tokens table)
//   - no prefix → legacy token (validates via sessions.token_hash)
func resolveToken(ctx context.Context, db *sql.DB, rawToken string) (*TokenInfo, error) {
	instanceID := httputil.InstanceIDFromContext(ctx)
	switch {
	case strings.HasPrefix(rawToken, PrefixSession):
		return resolveSessionToken(ctx, db, rawToken, instanceID)
	case strings.HasPrefix(rawToken, PrefixPAT):
		return resolvePATToken(ctx, db, rawToken, instanceID)
	case strings.HasPrefix(rawToken, PrefixOpaque):
		return resolveOpaqueToken(ctx, db, rawToken, instanceID)
	default:
		return resolveLegacyToken(ctx, db, rawToken, instanceID)
	}
}

// resolveSessionToken validates a session token via the tokens + sessions tables.
func resolveSessionToken(ctx context.Context, db *sql.DB, rawToken string, instanceID string) (*TokenInfo, error) {
	h := hashToken(rawToken)
	now := time.Now().UTC()

	var info TokenInfo
	var tokenExpiresAt, tokenRevokedAt, sessionExpiresAt, sessionRevokedAt sql.NullString
	err := db.QueryRowContext(ctx,
		`SELECT t.user_id, t.session_id, COALESCE(u.org_id, '0'),
		        t.expires_at, t.revoked_at, s.expires_at, s.revoked_at
		 FROM tokens t
		 JOIN sessions s ON t.session_id = s.id AND s.instance_id = ?
		 LEFT JOIN users u ON t.user_id = u.id AND u.instance_id = ?
		 WHERE t.token_hash = ?
		   AND t.instance_id = ?
		   AND t.type = 'session'`,
		instanceID, instanceID, h, instanceID,
	).Scan(&info.UserID, &info.SessionID, &info.OrgID, &tokenExpiresAt, &tokenRevokedAt, &sessionExpiresAt, &sessionRevokedAt)
	if err != nil {
		return nil, fmt.Errorf("invalid session token")
	}
	if !tokenIsUsable(now, tokenExpiresAt, tokenRevokedAt) || !tokenIsUsable(now, sessionExpiresAt, sessionRevokedAt) {
		return nil, fmt.Errorf("invalid session token")
	}
	info.TokenType = TokenTypeSession

	// Update last_used (best-effort, inline — cheap single-row UPDATE).
	lastUsedAt := time.Now().UTC().Format(time.RFC3339)
	_, _ = db.ExecContext(ctx, `UPDATE tokens SET last_used = ? WHERE token_hash = ? AND instance_id = ?`, lastUsedAt, h, instanceID)

	return &info, nil
}

// resolvePATToken validates a personal access token via the tokens table.
func resolvePATToken(ctx context.Context, db *sql.DB, rawToken string, instanceID string) (*TokenInfo, error) {
	h := hashToken(rawToken)
	now := time.Now().UTC()

	var info TokenInfo
	var expiresAt, revokedAt sql.NullString
	err := db.QueryRowContext(ctx,
		`SELECT t.user_id, COALESCE(u.org_id, '0'), t.expires_at, t.revoked_at
		 FROM tokens t
		 LEFT JOIN users u ON t.user_id = u.id AND u.instance_id = ?
		 WHERE t.token_hash = ?
		   AND t.instance_id = ?
		   AND t.type = 'pat'`,
		instanceID, h, instanceID,
	).Scan(&info.UserID, &info.OrgID, &expiresAt, &revokedAt)
	if err != nil {
		return nil, fmt.Errorf("invalid PAT")
	}
	if !tokenIsUsable(now, expiresAt, revokedAt) {
		return nil, fmt.Errorf("invalid PAT")
	}
	info.TokenType = TokenTypePAT

	// Update last_used (best-effort, inline — cheap single-row UPDATE).
	lastUsedAt := time.Now().UTC().Format(time.RFC3339)
	_, _ = db.ExecContext(ctx, `UPDATE tokens SET last_used = ? WHERE token_hash = ? AND instance_id = ?`, lastUsedAt, h, instanceID)

	return &info, nil
}

// resolveOpaqueToken validates an opaque token via the tokens table.
func resolveOpaqueToken(ctx context.Context, db *sql.DB, rawToken string, instanceID string) (*TokenInfo, error) {
	h := hashToken(rawToken)
	now := time.Now().UTC()

	var info TokenInfo
	var expiresAt, revokedAt sql.NullString
	err := db.QueryRowContext(ctx,
		`SELECT user_id, expires_at, revoked_at FROM tokens
		 WHERE token_hash = ?
		   AND instance_id = ?
		   AND type = 'opaque'`,
		h, instanceID,
	).Scan(&info.UserID, &expiresAt, &revokedAt)
	if err != nil {
		return nil, fmt.Errorf("invalid opaque token")
	}
	if !tokenIsUsable(now, expiresAt, revokedAt) {
		return nil, fmt.Errorf("invalid opaque token")
	}
	info.TokenType = TokenTypeOpaque

	return &info, nil
}

// resolveLegacyToken validates a token against the old sessions.token_hash column.
// This provides backward compatibility during migration.
func resolveLegacyToken(ctx context.Context, db *sql.DB, rawToken string, instanceID string) (*TokenInfo, error) {
	h := hashToken(rawToken)
	now := time.Now().UTC()

	// First try the new tokens table (for migrated tokens).
	var info TokenInfo
	var expiresAt, revokedAt sql.NullString
	err := db.QueryRowContext(ctx,
		`SELECT t.user_id, COALESCE(t.session_id, ''), t.type, t.expires_at, t.revoked_at
		 FROM tokens t
		 WHERE t.token_hash = ?
		   AND t.instance_id = ?`,
		h, instanceID,
	).Scan(&info.UserID, &info.SessionID, &info.TokenType, &expiresAt, &revokedAt)
	if err == nil && tokenIsUsable(now, expiresAt, revokedAt) {
		return &info, nil
	}

	// Fall back to the old sessions table.
	var sessionExpiresAt, sessionRevokedAt sql.NullString
	err = db.QueryRowContext(ctx,
		`SELECT user_id, id, expires_at, revoked_at FROM sessions
		 WHERE token_hash = ? AND instance_id = ?`,
		h, instanceID,
	).Scan(&info.UserID, &info.SessionID, &sessionExpiresAt, &sessionRevokedAt)
	if err != nil {
		return nil, fmt.Errorf("invalid token")
	}
	if !tokenIsUsable(now, sessionExpiresAt, sessionRevokedAt) {
		return nil, fmt.Errorf("invalid token")
	}
	info.TokenType = TokenTypeSession
	return &info, nil
}

func tokenIsUsable(now time.Time, expiresAt, revokedAt sql.NullString) bool {
	if revokedAt.Valid && revokedAt.String != "" {
		return false
	}
	if !expiresAt.Valid || expiresAt.String == "" {
		return true
	}
	expiration, ok := parseStoredTimestamp(expiresAt.String)
	return ok && expiration.After(now)
}

func parseStoredTimestamp(value string) (time.Time, bool) {
	for _, layout := range []string{time.RFC3339Nano, time.RFC3339, "2006-01-02 15:04:05"} {
		if t, err := time.Parse(layout, value); err == nil {
			return t, true
		}
	}
	return time.Time{}, false
}
