package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"

	"github.com/zitadel/zitadel/internal/id"
)

// --- Event types ---

type EventResponse struct {
	ID            int64  `json:"id,string"`
	EventType     string `json:"event_type"`
	OrgID         int64  `json:"org_id,string"`
	ActorID       int64  `json:"actor_id,string"`
	ActorType     string `json:"actor_type"`
	AggregateID   int64  `json:"aggregate_id,string"`
	AggregateType string `json:"aggregate_type"`
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
	var cursor int64
	if c := r.URL.Query().Get("cursor"); c != "" {
		cursor, _ = strconv.ParseInt(c, 10, 64)
	}

	query := `SELECT id, event_type, org_id, actor_id, actor_type,
	                 aggregate_id, aggregate_type, payload, metadata, created_at
	          FROM events WHERE id > ?`
	args := []any{cursor}

	if orgID := r.URL.Query().Get("org_id"); orgID != "" {
		if oid, err := strconv.ParseInt(orgID, 10, 64); err == nil {
			query += ` AND org_id = ?`
			args = append(args, oid)
		}
	}
	if aggType := r.URL.Query().Get("aggregate_type"); aggType != "" {
		query += ` AND aggregate_type = ?`
		args = append(args, aggType)
	}
	if aggID := r.URL.Query().Get("aggregate_id"); aggID != "" {
		if aid, err := strconv.ParseInt(aggID, 10, 64); err == nil {
			query += ` AND aggregate_id = ?`
			args = append(args, aid)
		}
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
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var events []EventResponse
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
		json.Unmarshal([]byte(payloadStr), &evt.Payload)
		json.Unmarshal([]byte(metadataStr), &evt.Metadata)
		events = append(events, evt)
	}

	var nextCursor string
	if len(events) > limit {
		events = events[:limit]
		nextCursor = strconv.FormatInt(events[len(events)-1].ID, 10)
	}

	writeJSON(w, http.StatusOK, ListResponse{Items: events, NextCursor: nextCursor})
}

func (a *API) aggregateEvents(w http.ResponseWriter, r *http.Request) {
	queryName := r.URL.Query().Get("query")

	var query string
	var args []any

	switch queryName {
	case "event_counts":
		orgID, _ := strconv.ParseInt(r.URL.Query().Get("org_id"), 10, 64)
		query = `SELECT event_type, COUNT(*) as cnt FROM events WHERE org_id = ? GROUP BY event_type`
		args = []any{orgID}
	default:
		writeError(w, http.StatusBadRequest, fmt.Sprintf("unknown aggregate query: %s", queryName))
		return
	}

	rows, err := a.db.SQL().QueryContext(r.Context(), query, args...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "aggregate query failed")
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

	writeJSON(w, http.StatusOK, ListResponse{Items: result})
}

// streamEvents provides Server-Sent Events (SSE) for real-time event streaming.
func (a *API) streamEvents(w http.ResponseWriter, r *http.Request) {
	flusher, ok := w.(http.Flusher)
	if !ok {
		writeError(w, http.StatusInternalServerError, "streaming not supported")
		return
	}

	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.WriteHeader(http.StatusOK)
	flusher.Flush()

	consumer := a.bus.Register(fmt.Sprintf("sse-%d", id.MustNew()))

	// Determine starting cursor.
	var cursor int64
	if c := r.URL.Query().Get("cursor"); c != "" && c != "now" {
		cursor, _ = strconv.ParseInt(c, 10, 64)
	} else {
		a.db.SQL().QueryRowContext(r.Context(), `SELECT COALESCE(MAX(id), 0) FROM events`).Scan(&cursor)
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
			json.Unmarshal([]byte(payloadStr), &evt.Payload)
			json.Unmarshal([]byte(metadataStr), &evt.Metadata)

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
		rows.Close()
	}
}
