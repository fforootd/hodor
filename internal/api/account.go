package api

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/redact"
	"github.com/zitadel/zitadel/internal/uniqueness"
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

func callerIduserID(r *http.Request) string {
	return r.Header.Get("X-Identity-Id")
}

func callerSessionID(r *http.Request) string {
	return r.Header.Get("X-Session-Id")
}

// --- GET /v1/account/profile ---

func (a *API) getProfile(w http.ResponseWriter, r *http.Request) {
	userID := callerIduserID(r)

	// Load identity.
	var identifier, displayName, state, metadata, schemaID, createdAt, updatedAt string
	var orgID string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT identifier, COALESCE(display_name,''), state, COALESCE(metadata,'{}'),
		        org_id, COALESCE(schema_id,''), created_at, updated_at
		 FROM users WHERE id = ?`, userID,
	).Scan(&identifier, &displayName, &state, &metadata, &orgID, &schemaID, &createdAt, &updatedAt)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
		return
	}

	// Parse profile JSON.
	var profileMap map[string]any
	json.Unmarshal([]byte(metadata), &profileMap)

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
	a.EmitAuthEvent(r.Context(), "account.profile_viewed", userID, map[string]any{
		"user_id": userID,
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"identity": map[string]any{
			"id":           userID,
			"identifier":   identifier,
			"display_name": displayName,
			"state":        state,
			"org_id":       orgID,
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
	userID := callerIduserID(r)

	var req struct {
		DisplayName *string        `json:"display_name,omitempty"`
		Profile     map[string]any `json:"profile,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid JSON body")
		return
	}

	// Load current identity data.
	var currentMetadata, schemaID, identifier, currentDisplayName, orgID string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT COALESCE(metadata,'{}'), COALESCE(schema_id,''), identifier, COALESCE(display_name,''), COALESCE(org_id,'')
		 FROM users WHERE id = ?`, userID,
	).Scan(&currentMetadata, &schemaID, &identifier, &currentDisplayName, &orgID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "identity not found")
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
				httputil.WriteError(w, http.StatusForbidden, fmt.Sprintf("field %q is not editable by the account owner", field))
				return
			}
		}
	}

	// Merge profile updates.
	var existingProfile map[string]any
	json.Unmarshal([]byte(currentMetadata), &existingProfile)
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

	nextDisplayName := currentDisplayName
	if req.DisplayName != nil {
		nextDisplayName = strings.TrimSpace(*req.DisplayName)
	}

	write, err := a.prepareExistingUserWrite(r.Context(), schemaID, identifier, nextDisplayName, existingProfile)
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, userWriteBadRequest(err))
		return
	}

	// Update.
	tx, err := a.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	defer tx.Rollback()

	query := "UPDATE users SET updated_at = datetime('now'), metadata = ? WHERE id = ?"
	args := []any{write.MetadataJSON, userID}
	if req.DisplayName != nil {
		query = "UPDATE users SET updated_at = datetime('now'), display_name = ?, metadata = ? WHERE id = ?"
		args = []any{nextDisplayName, write.MetadataJSON, userID}
		changedFields = append(changedFields, "display_name")
	}

	_, err = tx.ExecContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}
	if err := reindexUserUniqueness(r.Context(), tx, userID, orgID, identifier, write); err != nil {
		if v, ok := err.(*uniqueness.ViolationError); ok {
			httputil.WriteError(w, http.StatusConflict, fmt.Sprintf("field %q value %q already exists", v.Field, v.Value))
			return
		}
		httputil.WriteError(w, http.StatusConflict, "update failed")
		return
	}
	if err := tx.Commit(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "update failed")
		return
	}

	// Emit event with redacted before/after.
	a.EmitAuthEvent(r.Context(), "account.profile_updated", userID, map[string]any{
		"fields_changed": changedFields,
		"before":         redact.Payload(schemaJSON, beforeProfile),
		"after":          redact.Payload(schemaJSON, existingProfile),
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"status": "updated", "fields_changed": changedFields})
}

// --- GET /v1/account/sessions ---

func (a *API) listOwnSessions(w http.ResponseWriter, r *http.Request) {
	userID := callerIduserID(r)
	currentSessionID := callerSessionID(r)

	rows, err := a.db.SQL().QueryContext(r.Context(),
		`SELECT id, user_agent, ip_address, created_at, expires_at
		 FROM sessions WHERE user_id = ? AND revoked_at IS NULL AND expires_at > datetime('now')
		 ORDER BY created_at DESC`, userID,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var sessions []map[string]any
	for rows.Next() {
		var sid string
		var userAgent, ipAddress, createdAt, expiresAt string
		rows.Scan(&sid, &userAgent, &ipAddress, &createdAt, &expiresAt)
		sessions = append(sessions, map[string]any{
			"id":         sid,
			"user_agent": userAgent,
			"ip_address": ipAddress,
			"created_at": createdAt,
			"expires_at": expiresAt,
			"current":    sid == currentSessionID,
		})
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}
	if sessions == nil {
		sessions = []map[string]any{}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"sessions": sessions, "count": len(sessions)})
}

// --- POST /v1/account/sessions/{id}/revoke ---

func (a *API) revokeOwnSession(w http.ResponseWriter, r *http.Request) {
	userID := callerIduserID(r)
	sessionID := r.PathValue("id")

	// Only allow revoking own sessions.
	var ownerID string
	var userAgent, ipAddress string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT user_id, COALESCE(user_agent,''), COALESCE(ip_address,'')
		 FROM sessions WHERE id = ? AND revoked_at IS NULL`, sessionID,
	).Scan(&ownerID, &userAgent, &ipAddress)
	if err != nil || ownerID != userID {
		httputil.WriteError(w, http.StatusNotFound, "session not found")
		return
	}

	_, _ = a.db.SQL().ExecContext(r.Context(),
		`UPDATE sessions SET revoked_at = datetime('now') WHERE id = ?`, sessionID)

	a.EmitAuthEvent(r.Context(), "account.session_revoked", userID, map[string]any{
		"session_id": sessionID,
		"user_agent": userAgent,
		"ip_address": ipAddress,
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"status": "revoked"})
}

// --- POST /v1/account/sessions/revoke-others ---

func (a *API) revokeOtherSessions(w http.ResponseWriter, r *http.Request) {
	userID := callerIduserID(r)
	currentSessionID := callerSessionID(r)

	result, err := a.db.SQL().ExecContext(r.Context(),
		`UPDATE sessions SET revoked_at = datetime('now')
		 WHERE user_id = ? AND id != ? AND revoked_at IS NULL`,
		userID, currentSessionID,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "revocation failed")
		return
	}

	count, _ := result.RowsAffected()

	a.EmitAuthEvent(r.Context(), "account.sessions_revoked_all", userID, map[string]any{
		"count":           count,
		"kept_session_id": currentSessionID,
	})

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"status": "revoked", "count": count})
}

// --- GET /v1/account/activity ---

func (a *API) listOwnActivity(w http.ResponseWriter, r *http.Request) {
	userID := callerIduserID(r)

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
		 ORDER BY id DESC LIMIT ?`, userID, limit,
	)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var events []map[string]any
	for rows.Next() {
		var eid string
		var eventType, aggregateType, payload, createdAt string
		rows.Scan(&eid, &eventType, &aggregateType, &payload, &createdAt)

		var payloadMap map[string]any
		json.Unmarshal([]byte(payload), &payloadMap)

		events = append(events, map[string]any{
			"id":            eid,
			"event_type":    eventType,
			"resource_type": aggregateType,
			"payload":       payloadMap,
			"created_at":    createdAt,
			"time_ago":      timeAgo(createdAt),
		})
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}
	if events == nil {
		events = []map[string]any{}
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{"events": events, "count": len(events)})
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
