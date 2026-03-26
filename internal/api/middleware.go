package api

import (
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"net/http"
	"strings"
)

// requireAdmin is middleware that checks for a valid session with the "admin" capability.
// It supports both:
//   - Cookie-based auth (browser UI): __zitadel_session cookie
//   - Bearer token auth (API clients): Authorization: Bearer <token>
func (a *API) requireAdmin(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		token := extractToken(r)
		if token == "" {
			writeError(w, http.StatusUnauthorized, "authentication required")
			return
		}

		// Hash the token and look up the session + identity + capabilities.
		h := sha256.Sum256([]byte(token))
		tokenHash := hex.EncodeToString(h[:])

		var identityID int64
		err := a.db.SQL().QueryRowContext(r.Context(),
			`SELECT s.identity_id FROM sessions s
			 WHERE s.token_hash = ? AND s.revoked_at IS NULL AND s.expires_at > datetime('now')`,
			tokenHash,
		).Scan(&identityID)
		if err != nil {
			writeError(w, http.StatusUnauthorized, "invalid or expired session")
			return
		}

		// Check for admin capability.
		var adminCap int
		err = a.db.SQL().QueryRowContext(r.Context(),
			`SELECT 1 FROM identity_capabilities WHERE identity_id = ? AND capability = 'admin'`,
			identityID,
		).Scan(&adminCap)
		if err == sql.ErrNoRows {
			writeError(w, http.StatusForbidden, "admin capability required")
			return
		}
		if err != nil {
			writeError(w, http.StatusInternalServerError, "authorization check failed")
			return
		}

		next(w, r)
	}
}

// extractToken gets the session token from either the Authorization header or cookie.
func extractToken(r *http.Request) string {
	// Try Authorization: Bearer <token> first (API clients).
	if auth := r.Header.Get("Authorization"); auth != "" {
		if strings.HasPrefix(auth, "Bearer ") {
			return strings.TrimPrefix(auth, "Bearer ")
		}
	}
	// Fall back to session cookie (browser).
	if cookie, err := r.Cookie("__zitadel_session"); err == nil && cookie.Value != "" {
		return cookie.Value
	}
	return ""
}
