package api

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/instance"
	"strconv"
	"strings"

	"github.com/zitadel/zitadel/internal/id"
)

// --- Event types ---

type EventResponse struct {
	ID             string `json:"id"`
	EventType      string `json:"event_type"`
	OrgID          string `json:"org_id"`
	ActorID        string `json:"actor_id"`
	ActorType      string `json:"actor_type"`
	AggregateID    string `json:"aggregate_id"`
	AggregateType  string `json:"aggregate_type"`
	RequestID      string `json:"request_id,omitempty"`
	SessionID      string `json:"session_id,omitempty"`
	FlowID         string `json:"flow_id,omitempty"`
	Fingerprint    string `json:"fingerprint,omitempty"`
	ClientID       string `json:"client_id,omitempty"`
	TokenID        string `json:"token_id,omitempty"`
	DelegationType string `json:"delegation_type,omitempty"`
	SDKName        string `json:"sdk_name,omitempty"`
	SDKVersion     string `json:"sdk_version,omitempty"`
	Payload        any    `json:"payload,omitempty"`
	Metadata       any    `json:"metadata,omitempty"`
	CreatedAt      string `json:"created_at"`
}

type AggregateRow struct {
	Dimensions map[string]string `json:"dimensions"`
	Count      int64             `json:"count"`
}

// RegisterEventRoutes mounts event-related REST routes.
func (a *API) RegisterEventRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /v1/events", a.listEvents)
	mux.HandleFunc("GET /v1/events/aggregate", a.aggregateEvents)
	mux.HandleFunc("GET /v1/events/stream", a.streamEvents)
}

func (a *API) listEvents(w http.ResponseWriter, r *http.Request) {
	limit := 100
	if l := r.URL.Query().Get("limit"); l != "" {
		if n, err := strconv.Atoi(l); err == nil && n > 0 && n <= 1000 {
			limit = n
		}
	}
	var cursor string
	if c := r.URL.Query().Get("cursor"); c != "" {
		cursor = c
	}

	iid := instance.FromContext(r.Context())
	query := `SELECT id, event_type, org_id, actor_id, actor_type,
	                 aggregate_id, aggregate_type, payload, metadata, created_at,
	                 request_id, session_id, flow_id, fingerprint,
	                 client_id, token_id, delegation_type, sdk_name, sdk_version
	          FROM events WHERE instance_id = ? AND id > ?`
	args := []any{iid, cursor}

	if orgID := r.URL.Query().Get("org_id"); orgID != "" {
		query += ` AND org_id = ?`
		args = append(args, orgID)
	}
	if aggType := r.URL.Query().Get("aggregate_type"); aggType != "" {
		query += ` AND aggregate_type = ?`
		args = append(args, aggType)
	}
	if aggID := r.URL.Query().Get("aggregate_id"); aggID != "" {
		query += ` AND aggregate_id = ?`
		args = append(args, aggID)
	}
	if sessionID := r.URL.Query().Get("session_id"); sessionID != "" {
		query += ` AND session_id = ?`
		args = append(args, sessionID)
	}
	if fingerprint := r.URL.Query().Get("fingerprint"); fingerprint != "" {
		query += ` AND fingerprint = ?`
		args = append(args, fingerprint)
	}
	if clientID := r.URL.Query().Get("client_id"); clientID != "" {
		query += ` AND client_id = ?`
		args = append(args, clientID)
	}
	if delegationType := r.URL.Query().Get("delegation_type"); delegationType != "" {
		query += ` AND delegation_type = ?`
		args = append(args, delegationType)
	}
	if types := r.URL.Query().Get("types"); types != "" {
		typeList := strings.Split(types, ",")
		query += ` AND event_type IN (`
		for i, t := range typeList {
			if i > 0 {
				query += ","
			}
			query += "?"
			args = append(args, strings.TrimSpace(t))
		}
		query += `)`
	}
	if since := r.URL.Query().Get("since"); since != "" {
		query += ` AND created_at >= ?`
		args = append(args, since)
	}

	query += ` ORDER BY id ASC LIMIT ?`
	args = append(args, limit+1)

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var events []EventResponse
	for rows.Next() {
		var evt EventResponse
		var payloadStr, metadataStr string
		var requestID, sessionID, flowID, fingerprint *string
		var clientID, tokenID, delegationType, sdkName, sdkVersion *string
		if err := rows.Scan(
			&evt.ID, &evt.EventType, &evt.OrgID, &evt.ActorID, &evt.ActorType,
			&evt.AggregateID, &evt.AggregateType,
			&payloadStr, &metadataStr, &evt.CreatedAt,
			&requestID, &sessionID, &flowID, &fingerprint,
			&clientID, &tokenID, &delegationType, &sdkName, &sdkVersion,
		); err != nil {
			continue
		}
		if requestID != nil {
			evt.RequestID = *requestID
		}
		if sessionID != nil {
			evt.SessionID = *sessionID
		}
		if flowID != nil {
			evt.FlowID = *flowID
		}
		if fingerprint != nil {
			evt.Fingerprint = *fingerprint
		}
		if clientID != nil {
			evt.ClientID = *clientID
		}
		if tokenID != nil {
			evt.TokenID = *tokenID
		}
		if delegationType != nil {
			evt.DelegationType = *delegationType
		}
		if sdkName != nil {
			evt.SDKName = *sdkName
		}
		if sdkVersion != nil {
			evt.SDKVersion = *sdkVersion
		}
		json.Unmarshal([]byte(payloadStr), &evt.Payload)
		json.Unmarshal([]byte(metadataStr), &evt.Metadata)
		events = append(events, evt)
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}

	var nextCursor string
	if len(events) > limit {
		events = events[:limit]
		nextCursor = events[len(events)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: events, NextCursor: nextCursor})
}

func (a *API) aggregateEvents(w http.ResponseWriter, r *http.Request) {
	queryName := r.URL.Query().Get("query")

	var query string
	var args []any

	switch queryName {
	case "event_counts":
		orgID := r.URL.Query().Get("org_id")
		iid := instance.FromContext(r.Context())
		query = `SELECT event_type, COUNT(*) as cnt FROM events WHERE instance_id = ? AND org_id = ? GROUP BY event_type`
		args = []any{iid, orgID}
	default:
		httputil.WriteError(w, http.StatusBadRequest, fmt.Sprintf("unknown aggregate query: %s", queryName))
		return
	}

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "aggregate query failed")
		return
	}
	defer rows.Close()

	var result []AggregateRow
	for rows.Next() {
		var key string
		var count int64
		if err := rows.Scan(&key, &count); err != nil {
			continue
		}
		result = append(result, AggregateRow{
			Dimensions: map[string]string{"event_type": key},
			Count:      count,
		})
	}
	if err := rows.Err(); err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "rows error")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: result})
}

// streamEvents provides Server-Sent Events (SSE) for real-time event streaming.
func (a *API) streamEvents(w http.ResponseWriter, r *http.Request) {
	flusher, ok := w.(http.Flusher)
	if !ok {
		httputil.WriteError(w, http.StatusInternalServerError, "streaming not supported")
		return
	}

	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.WriteHeader(http.StatusOK)
	flusher.Flush()

	consumer := a.bus.Register(id.NewSSEConsumer())

	// Determine starting cursor.
	var cursor string
	if c := r.URL.Query().Get("cursor"); c != "" && c != "now" {
		cursor = c
	} else {
		a.db.SQL().QueryRowContext(r.Context(), `SELECT COALESCE(MAX(id), '') FROM events`).Scan(&cursor)
	}

	typeFilter := r.URL.Query().Get("types")
	var typeList []string
	if typeFilter != "" {
		for _, t := range strings.Split(typeFilter, ",") {
			typeList = append(typeList, strings.TrimSpace(t))
		}
	}

	fingerprintFilter := r.URL.Query().Get("fingerprint")
	clientFilter := r.URL.Query().Get("client_id")

	for {
		if !consumer.Wait(r.Context()) {
			return // Client disconnected.
		}

		iid := instance.FromContext(r.Context())
		rows, err := a.db.SQL().QueryContext(r.Context(),
			`SELECT id, event_type, org_id, actor_id, actor_type,
			        aggregate_id, aggregate_type, payload, metadata, created_at,
			        request_id, session_id, flow_id, fingerprint,
			        client_id, token_id, delegation_type, sdk_name, sdk_version
			 FROM events WHERE instance_id = ? AND id > ? ORDER BY id ASC LIMIT 100`, iid, cursor)
		if err != nil {
			return
		}

		for rows.Next() {
			var evt EventResponse
			var payloadStr, metadataStr string
			var requestID, sessionID, flowID, fingerprint *string
			var clientID, tokenID, delegationType, sdkName, sdkVersion *string
			if err := rows.Scan(
				&evt.ID, &evt.EventType, &evt.OrgID, &evt.ActorID, &evt.ActorType,
				&evt.AggregateID, &evt.AggregateType,
				&payloadStr, &metadataStr, &evt.CreatedAt,
				&requestID, &sessionID, &flowID, &fingerprint,
				&clientID, &tokenID, &delegationType, &sdkName, &sdkVersion,
			); err != nil {
				continue
			}
			if requestID != nil {
				evt.RequestID = *requestID
			}
			if sessionID != nil {
				evt.SessionID = *sessionID
			}
			if flowID != nil {
				evt.FlowID = *flowID
			}
			if fingerprint != nil {
				evt.Fingerprint = *fingerprint
			}
			if clientID != nil {
				evt.ClientID = *clientID
			}
			if tokenID != nil {
				evt.TokenID = *tokenID
			}
			if delegationType != nil {
				evt.DelegationType = *delegationType
			}
			if sdkName != nil {
				evt.SDKName = *sdkName
			}
			if sdkVersion != nil {
				evt.SDKVersion = *sdkVersion
			}
			_ = json.Unmarshal([]byte(payloadStr), &evt.Payload)
			_ = json.Unmarshal([]byte(metadataStr), &evt.Metadata)

			// Apply type filter.
			if len(typeList) > 0 {
				matched := false
				for _, t := range typeList {
					if evt.EventType == t {
						matched = true
						break
					}
				}
				if !matched {
					cursor = evt.ID
					continue
				}
			}

			// Apply fingerprint filter.
			if fingerprintFilter != "" && evt.Fingerprint != fingerprintFilter {
				cursor = evt.ID
				continue
			}

			// Apply client_id filter.
			if clientFilter != "" && evt.ClientID != clientFilter {
				cursor = evt.ID
				continue
			}

			data, _ := json.Marshal(evt)
			fmt.Fprintf(w, "data: %s\n\n", data)
			flusher.Flush()
			cursor = evt.ID
		}
		if err := rows.Err(); err != nil {
			return // Rows iteration error.
		}
		_ = rows.Close() //nolint:sqlclosecheck // SSE loop: rows created each iteration, defer would leak.
	}
}
