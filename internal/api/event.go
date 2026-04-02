package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"

	eventsvc "github.com/zitadel/zitadel/internal/events"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
)

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

	items, nextCursor, err := a.eventStore.List(r.Context(), eventsvc.Filter{
		Cursor:         r.URL.Query().Get("cursor"),
		Limit:          limit,
		OrgID:          r.URL.Query().Get("org_id"),
		AggregateType:  r.URL.Query().Get("aggregate_type"),
		AggregateID:    r.URL.Query().Get("aggregate_id"),
		SessionID:      r.URL.Query().Get("session_id"),
		Fingerprint:    r.URL.Query().Get("fingerprint"),
		ClientID:       r.URL.Query().Get("client_id"),
		DelegationType: r.URL.Query().Get("delegation_type"),
		Since:          r.URL.Query().Get("since"),
		Types:          splitCSV(r.URL.Query().Get("types")),
	})
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}

	events := make([]EventResponse, 0, len(items))
	for _, item := range items {
		events = append(events, eventResponseFromRecord(item))
	}
	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: events, NextCursor: nextCursor})
}

func (a *API) aggregateEvents(w http.ResponseWriter, r *http.Request) {
	queryName := r.URL.Query().Get("query")
	switch queryName {
	case "event_counts":
	default:
		httputil.WriteError(w, http.StatusBadRequest, fmt.Sprintf("unknown aggregate query: %s", queryName))
		return
	}

	rows, err := a.eventStore.AggregateCountsByEventType(r.Context(), r.URL.Query().Get("org_id"))
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "aggregate query failed")
		return
	}

	result := make([]AggregateRow, 0, len(rows))
	for _, row := range rows {
		result = append(result, AggregateRow{Dimensions: row.Dimensions, Count: row.Count})
	}
	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: result})
}

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
	cursor := r.URL.Query().Get("cursor")
	if cursor == "" || cursor == "now" {
		cursor, _ = a.eventStore.MaxID(r.Context())
	}

	typeList := splitCSV(r.URL.Query().Get("types"))
	fingerprintFilter := r.URL.Query().Get("fingerprint")
	clientFilter := r.URL.Query().Get("client_id")

	for {
		if !consumer.Wait(r.Context()) {
			return
		}

		records, err := a.eventStore.ListAfter(r.Context(), cursor, 100)
		if err != nil {
			return
		}
		for _, record := range records {
			evt := eventResponseFromRecord(record)
			if len(typeList) > 0 && !containsString(typeList, evt.EventType) {
				cursor = evt.ID
				continue
			}
			if fingerprintFilter != "" && evt.Fingerprint != fingerprintFilter {
				cursor = evt.ID
				continue
			}
			if clientFilter != "" && evt.ClientID != clientFilter {
				cursor = evt.ID
				continue
			}

			data, _ := json.Marshal(evt)
			fmt.Fprintf(w, "data: %s\n\n", data)
			flusher.Flush()
			cursor = evt.ID
		}
	}
}

func eventResponseFromRecord(record eventsvc.Event) EventResponse {
	return EventResponse{
		ID:             record.ID,
		EventType:      record.EventType,
		OrgID:          record.OrgID,
		ActorID:        record.ActorID,
		ActorType:      record.ActorType,
		AggregateID:    record.AggregateID,
		AggregateType:  record.AggregateType,
		RequestID:      record.RequestID,
		SessionID:      record.SessionID,
		FlowID:         record.FlowID,
		Fingerprint:    record.Fingerprint,
		ClientID:       record.ClientID,
		TokenID:        record.TokenID,
		DelegationType: record.DelegationType,
		SDKName:        record.SDKName,
		SDKVersion:     record.SDKVersion,
		Payload:        record.Payload,
		Metadata:       record.Metadata,
		CreatedAt:      record.CreatedAt,
	}
}

func splitCSV(value string) []string {
	if value == "" {
		return nil
	}
	parts := strings.Split(value, ",")
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part != "" {
			out = append(out, part)
		}
	}
	return out
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
