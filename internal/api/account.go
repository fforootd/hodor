package api

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/redact"
)

// RegisterAccountRoutes mounts self-service account endpoints.
// These require a valid session (cookie or bearer) but not admin.
func (a *API) RegisterAccountRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/account/profile", a.requireSession(a.getProfile))
	mux.HandleFunc("PATCH /v1/account/profile", a.requireSession(a.updateProfile))
	mux.HandleFunc("GET /v1/account/sessions", a.requireSession(a.listOwnSessions))
	mux.HandleFunc("POST /v1/account/sessions/{id}/revoke", a.requireSession(a.revokeOwnSession))
	mux.HandleFunc("POST /v1/account/sessions/revoke-others", a.requireSession(a.revokeOtherSessions))
	mux.HandleFunc("GET /v1/account/activity", a.requireSession(a.listOwnActivity))
}

// --- Session middleware (non-admin) ---

// requireSession is middleware that ensures a valid session exists.
// It injects the caller's identity ID into the request header.
func (a *API) requireSession(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		token := a.extractToken(r)
		if token == "" {
			writeError(w, http.StatusUnauthorized, "authentication required")
			return
		}

		h := sha256.Sum256([]byte(token))
		tokenHash := hex.EncodeToString(h[:])

		var identityID, sessionID int64
		err := a.db.SQL().QueryRowContext(r.Context(),
			`SELECT entity_id, id FROM sessions
			 WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > datetime('now')`,
			tokenHash,
		).Scan(&identityID, &sessionID)
		if err != nil {
			writeError(w, http.StatusUnauthorized, "invalid or expired session")
			return
		}

		// Inject caller info via headers (internal only).
		r.Header.Set("X-Identity-Id", fmt.Sprintf("%d", identityID))
		r.Header.Set("X-Session-Id", fmt.Sprintf("%d", sessionID))
		r.Header.Set("X-Token-Hash", tokenHash)

		next(w, r)
	}
}

func callerIdentityID(r *http.Request) int64 {
	var id int64
	_, _ = fmt.Sscanf(r.Header.Get("X-Identity-Id"), "%d", &id)
	return id
}

func callerSessionID(r *http.Request) int64 {
	var id int64
	_, _ = fmt.Sscanf(r.Header.Get("X-Session-Id"), "%d", &id)
	return id
}

// --- GET /v1/account/profile ---

func (a *API) getProfile(w http.ResponseWriter, r *http.Request) {
	identityID := callerIdentityID(r)

	// Load identity.
	var identifier, displayName, state, profile, schemaID, createdAt, updatedAt string
	var orgID int64
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT identifier, COALESCE(display_name,''), state, COALESCE(profile,'{}'),
		        org_id, COALESCE(schema_id,''), created_at, updated_at
		 FROM entities WHERE id = ?`, identityID,
	).Scan(&identifier, &displayName, &state, &profile, &orgID, &schemaID, &createdAt, &updatedAt)
	if err != nil {
		writeError(w, http.StatusNotFound, "identity not found")
		return
	}

	// Parse profile JSON.
	var profileMap map[string]any
	json.Unmarshal([]byte(profile), &profileMap)

	// Load schema to get field permissions.
	var schemaJSON, schemaType string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(schema,'{}'), type FROM schemas WHERE id = ?`, schemaID,
	).Scan(&schemaJSON, &schemaType)
	if err != nil {
		schemaJSON = "{}"
		schemaType = "unknown"
	}

	// Default editable for human_user, not for others.
	defaultEditable := schemaType == "human_user"
	fieldPerms := redact.FieldPermissions(schemaJSON, defaultEditable)

	// Emit view event.
	a.EmitAuthEvent(r.Context(), "account.profile_viewed", identityID, map[string]any{
		"entity_id": identityID,
	})

	writeJSON(w, http.StatusOK, map[string]any{
		"identity": map[string]any{
			"id":           fmt.Sprintf("%d", identityID),
			"identifier":   identifier,
			"display_name": displayName,
			"state":        state,
			"org_id":       fmt.Sprintf("%d", orgID),
			"profile":      profileMap,
			"created_at":   createdAt,
			"updated_at":   updatedAt,
		},
		"schema": map[string]any{
			"id":   schemaID,
			"type": schemaType,
		},
		"field_permissions": fieldPerms,
	})
}

// --- PATCH /v1/account/profile ---

func (a *API) updateProfile(w http.ResponseWriter, r *http.Request) {
	identityID := callerIdentityID(r)

	var req struct {
		DisplayName *string        `json:"display_name,omitempty"`
		Profile     map[string]any `json:"profile,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	// Load current identity data.
	var currentProfile, schemaID string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(profile,'{}'), COALESCE(schema_id,'') FROM entities WHERE id = ?`, identityID,
	).Scan(&currentProfile, &schemaID)
	if err != nil {
		writeError(w, http.StatusNotFound, "identity not found")
		return
	}

	// Load schema.
	var schemaJSON, schemaType string
	err = a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(schema,'{}'), type FROM schemas WHERE id = ?`, schemaID,
	).Scan(&schemaJSON, &schemaType)
	if err != nil {
		schemaJSON = "{}"
		schemaType = "unknown"
	}

	// FGA Layer 2: Check field-level permissions.
	// Owner can only edit x-editable fields.
	// (Layer 1 is handled by requireSession — if they have a valid session, they own the identity.)
	defaultEditable := schemaType == "human_user"
	editableFields := redact.UserEditableFields(schemaJSON, defaultEditable)

	if req.Profile != nil {
		for field := range req.Profile {
			if allowed, exists := editableFields[field]; exists && !allowed {
				writeError(w, http.StatusForbidden, fmt.Sprintf("field %q is not editable by the account owner", field))
				return
			}
		}
	}

	// Merge profile updates.
	var existingProfile map[string]any
	json.Unmarshal([]byte(currentProfile), &existingProfile)
	if existingProfile == nil {
		existingProfile = make(map[string]any)
	}

	beforeProfile := make(map[string]any, len(existingProfile))
	for k, v := range existingProfile {
		beforeProfile[k] = v
	}

	changedFields := []string{}
	if req.Profile != nil {
		for k, v := range req.Profile {
			existingProfile[k] = v
			changedFields = append(changedFields, k)
		}
	}

	profileBytes, _ := json.Marshal(existingProfile)

	// Update.
	updates := []string{"updated_at = datetime('now')"}
	args := []any{}

	if req.DisplayName != nil {
		updates = append(updates, "display_name = ?")
		args = append(args, strings.TrimSpace(*req.DisplayName))
		changedFields = append(changedFields, "display_name")
	}

	updates = append(updates, "profile = ?")
	args = append(args, string(profileBytes))
	args = append(args, identityID)

	query := fmt.Sprintf("UPDATE entities SET %s WHERE id = ?", strings.Join(updates, ", "))
	_, err = a.db.SQL().ExecContext(r.Context(), query, args...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "update failed")
		return
	}

	// Emit event with redacted before/after.
	a.EmitAuthEvent(r.Context(), "account.profile_updated", identityID, map[string]any{
		"fields_changed": changedFields,
		"before":         redact.Payload(schemaJSON, beforeProfile),
		"after":          redact.Payload(schemaJSON, existingProfile),
	})

	writeJSON(w, http.StatusOK, map[string]any{"status": "updated", "fields_changed": changedFields})
}

// --- GET /v1/account/sessions ---

func (a *API) listOwnSessions(w http.ResponseWriter, r *http.Request) {
	identityID := callerIdentityID(r)
	currentSessionID := callerSessionID(r)

	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, user_agent, ip_address, created_at, expires_at
		 FROM sessions WHERE entity_id = ? AND revoked_at IS NULL AND expires_at > datetime('now')
		 ORDER BY created_at DESC`, identityID,
	)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var sessions []map[string]any
	for rows.Next() {
		var sid int64
		var userAgent, ipAddress, createdAt, expiresAt string
		rows.Scan(&sid, &userAgent, &ipAddress, &createdAt, &expiresAt)
		sessions = append(sessions, map[string]any{
			"id":         fmt.Sprintf("%d", sid),
			"user_agent": userAgent,
			"ip_address": ipAddress,
			"created_at": createdAt,
			"expires_at": expiresAt,
			"current":    sid == currentSessionID,
		})
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, "rows error")
		return
	}
	if sessions == nil {
		sessions = []map[string]any{}
	}

	writeJSON(w, http.StatusOK, map[string]any{"sessions": sessions, "count": len(sessions)})
}

// --- POST /v1/account/sessions/{id}/revoke ---

func (a *API) revokeOwnSession(w http.ResponseWriter, r *http.Request) {
	identityID := callerIdentityID(r)
	sessionID := r.PathValue("id")

	// Only allow revoking own sessions.
	var ownerID int64
	var userAgent, ipAddress string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT entity_id, COALESCE(user_agent,''), COALESCE(ip_address,'')
		 FROM sessions WHERE id = ? AND revoked_at IS NULL`, sessionID,
	).Scan(&ownerID, &userAgent, &ipAddress)
	if err != nil || ownerID != identityID {
		writeError(w, http.StatusNotFound, "session not found")
		return
	}

	_, _ = a.db.SQL().ExecContext(r.Context(),
		`UPDATE sessions SET revoked_at = datetime('now') WHERE id = ?`, sessionID)

	a.EmitAuthEvent(r.Context(), "account.session_revoked", identityID, map[string]any{
		"session_id": sessionID,
		"user_agent": userAgent,
		"ip_address": ipAddress,
	})

	writeJSON(w, http.StatusOK, map[string]any{"status": "revoked"})
}

// --- POST /v1/account/sessions/revoke-others ---

func (a *API) revokeOtherSessions(w http.ResponseWriter, r *http.Request) {
	identityID := callerIdentityID(r)
	currentSessionID := callerSessionID(r)

	result, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE sessions SET revoked_at = datetime('now')
		 WHERE entity_id = ? AND id != ? AND revoked_at IS NULL`,
		identityID, currentSessionID,
	)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "revocation failed")
		return
	}

	count, _ := result.RowsAffected()

	a.EmitAuthEvent(r.Context(), "account.sessions_revoked_all", identityID, map[string]any{
		"count":           count,
		"kept_session_id": fmt.Sprintf("%d", currentSessionID),
	})

	writeJSON(w, http.StatusOK, map[string]any{"status": "revoked", "count": count})
}

// --- GET /v1/account/activity ---

func (a *API) listOwnActivity(w http.ResponseWriter, r *http.Request) {
	identityID := callerIdentityID(r)

	limit := 20
	if l := r.URL.Query().Get("limit"); l != "" {
		_, _ = fmt.Sscanf(l, "%d", &limit)
		if limit > 100 {
			limit = 100
		}
	}

	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, event_type, COALESCE(aggregate_type,''), COALESCE(payload,'{}'), created_at
		 FROM events WHERE actor_id = ?
		 ORDER BY id DESC LIMIT ?`, identityID, limit,
	)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var events []map[string]any
	for rows.Next() {
		var eid int64
		var eventType, aggregateType, payload, createdAt string
		rows.Scan(&eid, &eventType, &aggregateType, &payload, &createdAt)

		var payloadMap map[string]any
		json.Unmarshal([]byte(payload), &payloadMap)

		events = append(events, map[string]any{
			"id":            fmt.Sprintf("%d", eid),
			"event_type":    eventType,
			"resource_type": aggregateType,
			"payload":       payloadMap,
			"created_at":    createdAt,
			"time_ago":      timeAgo(createdAt),
		})
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, "rows error")
		return
	}
	if events == nil {
		events = []map[string]any{}
	}

	writeJSON(w, http.StatusOK, map[string]any{"events": events, "count": len(events)})
}

// --- Helpers ---

func timeAgo(dateStr string) string {
	t, err := time.Parse(time.RFC3339, dateStr)
	if err != nil {
		t, err = time.Parse("2006-01-02 15:04:05", dateStr)
		if err != nil {
			return dateStr
		}
	}
	d := time.Since(t)
	switch {
	case d < time.Minute:
		return "just now"
	case d < time.Hour:
		return fmt.Sprintf("%dm ago", int(d.Minutes()))
	case d < 24*time.Hour:
		return fmt.Sprintf("%dh ago", int(d.Hours()))
	default:
		return fmt.Sprintf("%dd ago", int(d.Hours()/24))
	}
}
