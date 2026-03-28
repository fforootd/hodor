package api

import (
	"context"
	"database/sql"
	"fmt"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/crypto"
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
	EntityID  string   // The identity this token belongs to ("" if nullable)
	SessionID string   // Only for session tokens
	TokenType string   // "session", "pat", "opaque"
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
	switch {
	case strings.HasPrefix(rawToken, PrefixSession):
		return resolveSessionToken(ctx, db, rawToken)
	case strings.HasPrefix(rawToken, PrefixPAT):
		return resolvePATToken(ctx, db, rawToken)
	case strings.HasPrefix(rawToken, PrefixOpaque):
		return resolveOpaqueToken(ctx, db, rawToken)
	default:
		return resolveLegacyToken(ctx, db, rawToken)
	}
}

// resolveSessionToken validates a session token via the tokens + sessions tables.
func resolveSessionToken(ctx context.Context, db *sql.DB, rawToken string) (*TokenInfo, error) {
	h := hashToken(rawToken)

	var info TokenInfo
	err := db.QueryRowContext(ctx,
		`SELECT t.entity_id, t.session_id FROM tokens t
		 JOIN sessions s ON t.session_id = s.id
		 WHERE t.token_hash = ?
		   AND t.type = 'session'
		   AND t.revoked_at IS NULL
		   AND s.revoked_at IS NULL
		   AND s.expires_at > datetime('now')
		   AND (t.expires_at IS NULL OR t.expires_at > datetime('now'))`,
		h,
	).Scan(&info.EntityID, &info.SessionID)
	if err != nil {
		return nil, fmt.Errorf("invalid session token")
	}
	info.TokenType = TokenTypeSession

	// Update last_used (best-effort, inline — cheap single-row UPDATE).
	now := time.Now().UTC().Format(time.RFC3339)
	_, _ = db.ExecContext(ctx, `UPDATE tokens SET last_used = ? WHERE token_hash = ?`, now, h)

	return &info, nil
}

// resolvePATToken validates a personal access token via the tokens table.
func resolvePATToken(ctx context.Context, db *sql.DB, rawToken string) (*TokenInfo, error) {
	h := hashToken(rawToken)

	var info TokenInfo
	err := db.QueryRowContext(ctx,
		`SELECT entity_id FROM tokens
		 WHERE token_hash = ?
		   AND type = 'pat'
		   AND revoked_at IS NULL
		   AND (expires_at IS NULL OR expires_at > datetime('now'))`,
		h,
	).Scan(&info.EntityID)
	if err != nil {
		return nil, fmt.Errorf("invalid PAT")
	}
	info.TokenType = TokenTypePAT

	// Update last_used (best-effort, inline — cheap single-row UPDATE).
	now := time.Now().UTC().Format(time.RFC3339)
	_, _ = db.ExecContext(ctx, `UPDATE tokens SET last_used = ? WHERE token_hash = ?`, now, h)

	return &info, nil
}

// resolveOpaqueToken validates an opaque token via the tokens table.
func resolveOpaqueToken(ctx context.Context, db *sql.DB, rawToken string) (*TokenInfo, error) {
	h := hashToken(rawToken)

	var info TokenInfo
	err := db.QueryRowContext(ctx,
		`SELECT entity_id FROM tokens
		 WHERE token_hash = ?
		   AND type = 'opaque'
		   AND revoked_at IS NULL
		   AND (expires_at IS NULL OR expires_at > datetime('now'))`,
		h,
	).Scan(&info.EntityID)
	if err != nil {
		return nil, fmt.Errorf("invalid opaque token")
	}
	info.TokenType = TokenTypeOpaque

	return &info, nil
}

// resolveLegacyToken validates a token against the old sessions.token_hash column.
// This provides backward compatibility during migration.
func resolveLegacyToken(ctx context.Context, db *sql.DB, rawToken string) (*TokenInfo, error) {
	h := hashToken(rawToken)

	// First try the new tokens table (for migrated tokens).
	var info TokenInfo
	err := db.QueryRowContext(ctx,
		`SELECT t.entity_id, COALESCE(t.session_id, ''), t.type FROM tokens t
		 WHERE t.token_hash = ?
		   AND t.revoked_at IS NULL
		   AND (t.expires_at IS NULL OR t.expires_at > datetime('now'))`,
		h,
	).Scan(&info.EntityID, &info.SessionID, &info.TokenType)
	if err == nil {
		return &info, nil
	}

	// Fall back to the old sessions table.
	err = db.QueryRowContext(ctx,
		`SELECT entity_id, id FROM sessions
		 WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > datetime('now')`,
		h,
	).Scan(&info.EntityID, &info.SessionID)
	if err != nil {
		return nil, fmt.Errorf("invalid token")
	}
	info.TokenType = TokenTypeSession
	return &info, nil
}
