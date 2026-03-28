package api

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
	"strconv"
	"strings"

	"github.com/zitadel/zitadel/internal/id"
)

// --- Event types ---

type EventResponse struct {
	ID            string `json:"id"`
	EventType     string `json:"event_type"`
	OrgID         string `json:"org_id"`
	ActorID       string `json:"actor_id"`
	ActorType     string `json:"actor_type"`
	AggregateID   string `json:"aggregate_id"`
	AggregateType string `json:"aggregate_type"`
	SessionID     string `json:"session_id,omitempty"`
	TraceID       string `json:"trace_id,omitempty"`
	SpanID        string `json:"span_id,omitempty"`
	ParentSpanID  string `json:"parent_span_id,omitempty"`
	Payload       any    `json:"payload,omitempty"`
	Metadata      any    `json:"metadata,omitempty"`
	CreatedAt     string `json:"created_at"`
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

	query := `SELECT id, event_type, org_id, actor_id, actor_type,
	                 aggregate_id, aggregate_type, payload, metadata, created_at, session_id, trace_id, span_id, parent_span_id
	          FROM events WHERE id > ?`
	args := []any{cursor}

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
		var sessionID, traceID, spanID, parentSpanID *string
		if err := rows.Scan(
			&evt.ID, &evt.EventType, &evt.OrgID, &evt.ActorID, &evt.ActorType,
			&evt.AggregateID, &evt.AggregateType,
			&payloadStr, &metadataStr, &evt.CreatedAt, &sessionID, &traceID, &spanID, &parentSpanID,
		); err != nil {
			continue
		}
		if sessionID != nil {
			evt.SessionID = *sessionID
		}
		if traceID != nil {
			evt.TraceID = *traceID
		}
		if spanID != nil {
			evt.SpanID = *spanID
		}
		if parentSpanID != nil {
			evt.ParentSpanID = *parentSpanID
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
		query = `SELECT event_type, COUNT(*) as cnt FROM events WHERE org_id = ? GROUP BY event_type`
		args = []any{orgID}
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

	for {
		if !consumer.Wait(r.Context()) {
			return // Client disconnected.
		}

		rows, err := a.db.SQL().QueryContext(r.Context(),
			`SELECT id, event_type, org_id, actor_id, actor_type,
			        aggregate_id, aggregate_type, payload, metadata, created_at
			 FROM events WHERE id > ? ORDER BY id ASC LIMIT 100`, cursor)
		if err != nil {
			return
		}

		for rows.Next() {
			var evt EventResponse
			var payloadStr, metadataStr string
			if err := rows.Scan(
				&evt.ID, &evt.EventType, &evt.OrgID, &evt.ActorID, &evt.ActorType,
				&evt.AggregateID, &evt.AggregateType,
				&payloadStr, &metadataStr, &evt.CreatedAt,
			); err != nil {
				continue
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
