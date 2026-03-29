package ui

import (
	"context"
	"database/sql"
	"encoding/json"
	"github.com/zitadel/zitadel/internal/logging"
	"net/http"

	"strings"

	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/session"
)

// UserContext holds the authenticated identity info on the request context.
type UserContext struct {
	IduserID   string
	Identifier   string
	DisplayName  string
	Capabilities []string
}

// UI holds the HTTP handlers for the login and admin UI.
type UI struct {
	db        *database.DB
	bus       *eventbus.Bus
	passwords *auth.Passwords
	api       *api.API
	cookies   *session.CookieConfig
}

// New creates a new UI handler set.
func New(db *database.DB, bus *eventbus.Bus, restAPI *api.API, cookies *session.CookieConfig) *UI {
	return &UI{
		db:        db,
		bus:       bus,
		passwords: auth.NewPasswords(db),
		api:       restAPI,
		cookies:   cookies,
	}
}

// RegisterRoutes mounts UI routes on the given mux.
func (u *UI) RegisterRoutes(mux *http.ServeMux) {
	// Static assets (legacy CSS, kept for backwards compat).
	mux.HandleFunc("GET /static/style.css", u.handleCSS)

	// Logout handler.
	mux.HandleFunc("GET /logout", u.handleLogout)

	// Redirect /admin to /console for backwards compat.
	mux.HandleFunc("GET /admin", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/console", http.StatusTemporaryRedirect)
	})
	mux.HandleFunc("/admin/", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/console", http.StatusTemporaryRedirect)
	})
}

// --- Login handlers ---

func (u *UI) handleLoginPage(w http.ResponseWriter, r *http.Request) {
	if _, ok := u.getSession(r); ok {
		http.Redirect(w, r, "/admin", http.StatusSeeOther)
		return
	}
	renderLoginPage(w, "", r.URL.Query().Get("redirect_to"))
}

func (u *UI) handleLoginSubmit(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		renderLoginPage(w, "Invalid form data", "")
		return
	}

	identifier := r.FormValue("identifier")
	password := r.FormValue("password")
	redirectTo := r.FormValue("redirect_to")

	if identifier == "" || password == "" {
		renderLoginPage(w, "Username and password are required", redirectTo)
		return
	}

	// Look up identity by identifier.
	var userID string
	err := u.db.SQL().QueryRowContext(r.Context(),
		`SELECT id FROM users WHERE identifier = ? AND state = 'active'`,
		identifier,
	).Scan(&userID)
	if err == sql.ErrNoRows {
		u.api.EmitAuthEvent(r.Context(), "auth.login_failure", "", map[string]any{
			"identifier": identifier,
			"reason":     "unknown_user",
			"ip_address": r.RemoteAddr,
			"user_agent": r.UserAgent(),
		})
		renderLoginPage(w, "Invalid username or password", redirectTo)
		return
	}
	if err != nil {
		renderLoginPage(w, "Internal error", redirectTo)
		return
	}

	// Verify password.
	ok, err := u.passwords.CheckPassword(r.Context(), userID, password)
	if err != nil || !ok {
		u.api.EmitAuthEvent(r.Context(), "auth.login_failure", userID, map[string]any{
			"identifier": identifier,
			"reason":     "invalid_password",
			"ip_address": r.RemoteAddr,
			"user_agent": r.UserAgent(),
		})
		renderLoginPage(w, "Invalid username or password", redirectTo)
		return
	}

	// Create session via api package (emits session.created).
	sessResp, err := u.api.CreateSessionInternal(r.Context(), userID, r.UserAgent(), r.RemoteAddr, nil)
	if err != nil {
		logging.Printf("create session failed: %v", err)
		renderLoginPage(w, "Failed to create session", redirectTo)
		return
	}

	// Emit login success event.
	u.api.EmitAuthEvent(r.Context(), "auth.login_success", userID, map[string]any{
		"identifier":  identifier,
		"session_id":  sessResp.Session.ID,
		"auth_method": "password",
		"ip_address":  r.RemoteAddr,
		"user_agent":  r.UserAgent(),
	})

	// Set session cookie.
	// Set session cookie (HMAC-signed).
	session.SetSessionCookie(w, sessResp.Token, u.cookies)

	target := "/admin"
	if redirectTo != "" {
		target = redirectTo
	}
	http.Redirect(w, r, target, http.StatusSeeOther)
}

func (u *UI) handleLogout(w http.ResponseWriter, r *http.Request) {
	cookie, err := r.Cookie(u.cookies.CookieName())
	if err == nil && cookie.Value != "" {
		// Try HMAC-verified read first, fall back to raw.
		var rawToken string
		if token, ok := session.ReadSessionCookie(r, u.cookies); ok {
			rawToken = token
		} else {
			rawToken = cookie.Value
		}
		tokenHash := crypto.HashTokenHex(rawToken)

		var sessionID string
		err := u.db.SQL().QueryRowContext(r.Context(),
			`SELECT id FROM sessions WHERE token_hash = ? AND revoked_at IS NULL`, tokenHash,
		).Scan(&sessionID)
		if err == nil && sessionID != "" {
			_ = u.api.RevokeSessionInternal(r.Context(), sessionID)
		}
	}

	session.ClearSessionCookie(w, u.cookies)

	http.Redirect(w, r, "/login", http.StatusSeeOther)
}

// --- Admin handlers ---

func (u *UI) handleAdminDashboard(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)

	var identityCount, sessionCount, eventCount int
	u.db.SQL().QueryRowContext(r.Context(), `SELECT COUNT(*) FROM entities`).Scan(&identityCount)
	u.db.SQL().QueryRowContext(r.Context(), `SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL`).Scan(&sessionCount)
	u.db.SQL().QueryRowContext(r.Context(), `SELECT COUNT(*) FROM events`).Scan(&eventCount)

	renderAdminDashboard(w, ident, identityCount, sessionCount, eventCount)
}

func (u *UI) handleAdminIdentities(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)

	rows, err := u.db.SQL().QueryContext(r.Context(),
		`SELECT i.id, i.identifier, json_extract(i.data, '$.display_name'), i.state, i.created_at
		 FROM users i ORDER BY i.id ASC LIMIT 100`)
	if err != nil {
		http.Error(w, "Failed to load entities", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	type IdentityRow struct {
		ID          string
		Identifier  string
		DisplayName string
		State       string
		CreatedAt   string
	}
	var identities []IdentityRow
	for rows.Next() {
		var row IdentityRow
		var dn sql.NullString
		if err := rows.Scan(&row.ID, &row.Identifier, &dn, &row.State, &row.CreatedAt); err != nil {
			continue
		}
		if dn.Valid {
			row.DisplayName = dn.String
		}
		identities = append(identities, row)
	}
	if err := rows.Err(); err != nil {
		http.Error(w, "rows error", http.StatusInternalServerError)
		return
	}

	renderAdminEntities(w, ident, identities)
}

func (u *UI) handleAdminSessions(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)

	rows, err := u.db.SQL().QueryContext(r.Context(),
		`SELECT s.id, i.identifier, s.user_agent, s.ip_address, s.created_at, s.expires_at
		 FROM sessions s JOIN entities e ON s.user_id = i.id
		 WHERE s.revoked_at IS NULL ORDER BY s.created_at DESC LIMIT 100`)
	if err != nil {
		http.Error(w, "Failed to load sessions", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	type SessionRow struct {
		ID         string
		Identifier string
		UserAgent  string
		IPAddress  string
		CreatedAt  string
		ExpiresAt  string
	}
	var sessions []SessionRow
	for rows.Next() {
		var row SessionRow
		var userAgent, ipAddress sql.NullString
		if err := rows.Scan(&row.ID, &row.Identifier, &userAgent, &ipAddress, &row.CreatedAt, &row.ExpiresAt); err != nil {
			continue
		}
		row.UserAgent = userAgent.String
		row.IPAddress = ipAddress.String
		sessions = append(sessions, row)
	}
	if err := rows.Err(); err != nil {
		http.Error(w, "rows error", http.StatusInternalServerError)
		return
	}

	renderAdminSessions(w, ident, sessions)
}

func (u *UI) handleAdminEvents(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)

	typeFilter := r.URL.Query().Get("type")

	query := `SELECT id, event_type, actor_id, aggregate_id, aggregate_type, payload, trace_id, session_id, created_at
		 FROM events`
	var args []any

	if typeFilter != "" {
		query += ` WHERE event_type LIKE ?`
		args = append(args, typeFilter+"%")
	}
	query += ` ORDER BY id DESC LIMIT 100`

	rows, err := u.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		http.Error(w, "Failed to load events", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	type EventRow struct {
		ID            string
		EventType     string
		ActorID       string
		AggregateID   string
		AggregateType string
		Payload       string
		TraceID       string
		SessionID     string
		CreatedAt     string
	}
	var events []EventRow
	for rows.Next() {
		var row EventRow
		if err := rows.Scan(&row.ID, &row.EventType, &row.ActorID, &row.AggregateID,
			&row.AggregateType, &row.Payload, &row.TraceID, &row.SessionID, &row.CreatedAt); err != nil {
			continue
		}
		events = append(events, row)
	}
	if err := rows.Err(); err != nil {
		http.Error(w, "rows error", http.StatusInternalServerError)
		return
	}

	renderAdminEvents(w, ident, events, typeFilter)
}

func (u *UI) handleAdminJobs(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)

	type JobRow struct {
		Name        string
		DisplayName string
		Description string
		Cron        string
		Enabled     bool
		LastRunAt   string
		NextRunAt   string
		LastStatus  string
		LastError   string
		RunCount    int64
	}
	var jobRows []JobRow
	rows, err := u.db.SQL().QueryContext(r.Context(),
		`SELECT name, display_name, description, cron, enabled, 
		        COALESCE(last_run_at,'—'), COALESCE(next_run_at,'—'), 
		        last_status, last_error, run_count
		 FROM jobs ORDER BY name`)
	if err == nil {
		defer rows.Close()
		for rows.Next() {
			var j JobRow
			var enabled int
			if err := rows.Scan(&j.Name, &j.DisplayName, &j.Description, &j.Cron, &enabled,
				&j.LastRunAt, &j.NextRunAt, &j.LastStatus, &j.LastError, &j.RunCount); err != nil {
				continue
			}
			j.Enabled = enabled == 1
			jobRows = append(jobRows, j)
		}
		if err := rows.Err(); err == nil {
			rows.Close()
		}
	}

	type PolicyRow struct {
		EventPattern string
		OLTPTTL      string
		LakeTTL      string
		Priority     int
	}
	var policies []PolicyRow
	pRows, err := u.db.SQL().QueryContext(r.Context(),
		`SELECT event_pattern, oltp_ttl, lake_ttl, priority FROM retention_policies ORDER BY priority DESC`)
	if err == nil {
		defer pRows.Close()
		for pRows.Next() {
			var p PolicyRow
			if err := pRows.Scan(&p.EventPattern, &p.OLTPTTL, &p.LakeTTL, &p.Priority); err != nil {
				continue
			}
			policies = append(policies, p)
		}
		if err := pRows.Err(); err == nil {
			pRows.Close()
		}
	}

	renderAdminJobs(w, ident, jobRows, policies)
}

func (u *UI) handleAdminJobToggle(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("name")
	u.db.SQL().ExecContext(r.Context(),
		`UPDATE jobs SET enabled = CASE WHEN enabled = 1 THEN 0 ELSE 1 END WHERE name = ?`, name)
	http.Redirect(w, r, "/admin/jobs", http.StatusSeeOther)
}

func (u *UI) loadSchemaOptions(ctx context.Context) []SchemaOption {
	rows, err := u.db.SQL().QueryContext(ctx,
		`SELECT id, type, schema FROM schemas ORDER BY type ASC`)
	if err != nil {
		return nil
	}
	defer rows.Close()

	var opts []SchemaOption
	for rows.Next() {
		var id, typ, schemaJSON string
		if err := rows.Scan(&id, &typ, &schemaJSON); err != nil {
			continue
		}

		// Parse schema to extract fields for the JS dropdown.
		var schemaDef map[string]any
		var fieldsForJS []map[string]any
		if json.Unmarshal([]byte(schemaJSON), &schemaDef) == nil {
			requiredSet := map[string]bool{}
			if reqArr, ok := schemaDef["required"].([]any); ok {
				for _, r := range reqArr {
					if s, ok := r.(string); ok {
						requiredSet[s] = true
					}
				}
			}
			if props, ok := schemaDef["properties"].(map[string]any); ok {
				for name, def := range props {
					f := map[string]any{"name": name, "required": requiredSet[name]}
					if defMap, ok := def.(map[string]any); ok {
						if t, ok := defMap["type"].(string); ok {
							f["type"] = t
						}
						if fmt, ok := defMap["format"].(string); ok {
							f["format"] = fmt
						}
						if desc, ok := defMap["description"].(string); ok {
							f["description"] = desc
						}
						if enums, ok := defMap["enum"].([]any); ok {
							var enumStrs []string
							for _, e := range enums {
								if s, ok := e.(string); ok {
									enumStrs = append(enumStrs, s)
								}
							}
							f["enum"] = enumStrs
						}
					}
					fieldsForJS = append(fieldsForJS, f)
				}
			}
		}

		fieldsJSON, _ := json.Marshal(fieldsForJS)
		opts = append(opts, SchemaOption{
			ID:         id,
			Type:       typ,
			FieldsJSON: string(fieldsJSON),
		})
	}
	if err := rows.Err(); err != nil {
		return nil
	}
	return opts
}

func (u *UI) handleAdminIdentityNew(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)
	schemas := u.loadSchemaOptions(r.Context())
	renderAdminIdentityForm(w, ident, nil, "", schemas)
}

func (u *UI) handleAdminIdentityCreate(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)
	schemas := u.loadSchemaOptions(r.Context())
	if err := r.ParseForm(); err != nil {
		renderAdminIdentityForm(w, ident, nil, "Invalid form data", schemas)
		return
	}

	identifier := strings.TrimSpace(r.FormValue("identifier"))
	schemaID := strings.TrimSpace(r.FormValue("schema_id"))
	_ = schemaID // legacy field, may be used later

	// Build data map from data_* form fields.
	data := map[string]any{}
	for key, values := range r.Form {
		if strings.HasPrefix(key, "data_") && len(values) > 0 && values[0] != "" {
			fieldName := strings.TrimPrefix(key, "data_")
			data[fieldName] = values[0]
		}
	}

	// Build capabilities from auth method checkboxes.
	caps := append([]string{}, r.Form["auth_methods"]...)

	resp, err := u.api.CreateUserInternal(r, api.UserRequest{
		Identifier:   identifier,
		Capabilities: caps,
		Profile:      data,
	})
	if err != nil {
		renderAdminIdentityForm(w, ident, nil, "Failed to create: "+err.Error(), schemas)
		return
	}

	password := r.FormValue("password")
	if password != "" {
		if err := u.passwords.SetPassword(r.Context(), resp.ID, password); err != nil {
			renderAdminIdentityForm(w, ident, nil, "Created but failed to set password: "+err.Error(), schemas)
			return
		}
	}

	http.Redirect(w, r, "/admin/entities", http.StatusSeeOther)
}

func (u *UI) handleAdminIdentityEdit(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)

	parsedID := r.PathValue("id")
	if parsedID == "" {
		http.Error(w, "Invalid ID", http.StatusBadRequest)
		return
	}

	identity, err := u.api.GetIdentityByID(r, parsedID)
	if err != nil {
		http.Error(w, "Identity not found", http.StatusNotFound)
		return
	}

	schemas := u.loadSchemaOptions(r.Context())
	renderAdminIdentityForm(w, ident, &identity, "", schemas)
}

func (u *UI) handleAdminIdentityUpdate(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)
	if err := r.ParseForm(); err != nil {
		http.Error(w, "Invalid form data", http.StatusBadRequest)
		return
	}

	parsedID := r.PathValue("id")
	if parsedID == "" {
		http.Error(w, "Invalid ID", http.StatusBadRequest)
		return
	}

	displayName := strings.TrimSpace(r.FormValue("display_name"))
	state := strings.TrimSpace(r.FormValue("state"))
	password := r.FormValue("password")

	// Build data map from data_* form fields.
	data := map[string]any{}
	for key, values := range r.Form {
		if strings.HasPrefix(key, "data_") && len(values) > 0 && values[0] != "" {
			fieldName := strings.TrimPrefix(key, "data_")
			data[fieldName] = values[0]
		}
	}
	if displayName != "" {
		data["display_name"] = displayName
	}

	_, updateErr := u.api.UpdateUserInternal(r, parsedID, api.UserRequest{
		State:   state,
		Profile: data,
	})
	if updateErr != nil {
		identity, _ := u.api.GetIdentityByID(r, parsedID)
		schemas := u.loadSchemaOptions(r.Context())
		renderAdminIdentityForm(w, ident, &identity, "Update failed: "+updateErr.Error(), schemas)
		return
	}

	if password != "" {
		_ = u.passwords.SetPassword(r.Context(), parsedID, password)
	}

	http.Redirect(w, r, "/admin/entities", http.StatusSeeOther)
}

func (u *UI) handleAdminIdentityDelete(w http.ResponseWriter, r *http.Request) {
	parsedID := r.PathValue("id")
	if parsedID == "" {
		http.Error(w, "Invalid ID", http.StatusBadRequest)
		return
	}

	if err := u.api.DeleteIdentityInternal(r, parsedID); err != nil {
		http.Error(w, "Delete failed: "+err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/admin/entities", http.StatusSeeOther)
}

// --- Session helpers ---

func (u *UI) getSession(r *http.Request) (*UserContext, bool) {
	rawToken, ok := session.ReadSessionCookie(r, u.cookies)
	if !ok {
		return nil, false
	}

	tokenHash := crypto.HashTokenHex(rawToken)

	var userID string
	var identifier string
	var dataJSON sql.NullString
	err := u.db.SQL().QueryRowContext(r.Context(),
		`SELECT s.user_id, i.identifier, i.data
		 FROM sessions s JOIN entities e ON s.user_id = i.id
		 WHERE s.token_hash = ? AND s.revoked_at IS NULL AND s.expires_at > datetime('now')`,
		tokenHash,
	).Scan(&userID, &identifier, &dataJSON)
	if err != nil {
		return nil, false
	}

	// Extract display_name from data JSON.
	displayName := identifier
	if dataJSON.Valid {
		var data map[string]any
		if json.Unmarshal([]byte(dataJSON.String), &data) == nil {
			if dn, ok := data["display_name"].(string); ok && dn != "" {
				displayName = dn
			}
		}
	}

	// Load capabilities.
	capRows, err := u.db.SQL().QueryContext(r.Context(),
		`SELECT capability FROM user_capabilities WHERE user_id = ?`, userID)
	if err != nil {
		return nil, false
	}
	defer capRows.Close()

	var caps []string
	for capRows.Next() {
		var cap string
		if err := capRows.Scan(&cap); err == nil {
			caps = append(caps, cap)
		}
	}
	if err := capRows.Err(); err != nil {
		return nil, false
	}
	return &UserContext{
		IduserID:   userID,
		Identifier:   identifier,
		DisplayName:  displayName,
		Capabilities: caps,
	}, true
}

func (u *UI) handleAdminSchemas(w http.ResponseWriter, r *http.Request) {
	ident := r.Context().Value(ctxKeyIdentity).(*UserContext)

	rows, err := u.db.SQL().QueryContext(r.Context(),
		`SELECT id, type, schema, version, created_at FROM schemas ORDER BY type ASC`)
	if err != nil {
		http.Error(w, "Failed to load schemas", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var schemas []SchemaCard
	for rows.Next() {
		var s SchemaCard
		var schemaJSON string
		if err := rows.Scan(&s.ID, &s.Type, &schemaJSON, &s.Version, &s.CreatedAt); err != nil {
			continue
		}

		// Parse JSON Schema to extract fields.
		var schemaDef map[string]any
		if json.Unmarshal([]byte(schemaJSON), &schemaDef) == nil {
			// Get required fields set.
			requiredSet := map[string]bool{}
			if reqArr, ok := schemaDef["required"].([]any); ok {
				for _, r := range reqArr {
					if s, ok := r.(string); ok {
						requiredSet[s] = true
					}
				}
			}

			// Extract properties.
			if props, ok := schemaDef["properties"].(map[string]any); ok {
				for name, def := range props {
					fieldType := "any"
					if defMap, ok := def.(map[string]any); ok {
						if t, ok := defMap["type"].(string); ok {
							fieldType = t
						}
						if _, hasEnum := defMap["enum"]; hasEnum {
							fieldType = "enum"
						}
					}
					s.Fields = append(s.Fields, SchemaField{
						Name:     name,
						Type:     fieldType,
						Required: requiredSet[name],
					})
				}
			}
		}

		s.FieldCount = len(s.Fields)
		for _, f := range s.Fields {
			if f.Required {
				s.RequiredCount++
			}
		}

		schemas = append(schemas, s)
	}
	if err := rows.Err(); err != nil {
		http.Error(w, "rows error", http.StatusInternalServerError)
		return
	}

	renderAdminSchemas(w, ident, schemas)
}
