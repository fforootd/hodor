package api

import (
	"encoding/json"
	"io"
	"net/http"
	"sync"
	"time"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
)

// ─── Fingerprint Ingest ────────────────────────────────────

var (
	// knownFingerprints stores recently seen fingerprints to drop duplicates before DB writes.
	// Since fingerprints are 32-char strings, 100k of them take ~3MB.
	knownFingerprints sync.Map
)

const (
	maxFingerprintBodyBytes = 16 * 1024 // 16KB max
)

// ingestFingerprint handles POST /v1/telemetry/fingerprints
func (a *API) ingestFingerprint(w http.ResponseWriter, r *http.Request) {
	ip := r.RemoteAddr
	if !otelLimiter.allow(ip) {
		http.Error(w, "rate limit exceeded", http.StatusTooManyRequests)
		return
	}

	body, err := io.ReadAll(io.LimitReader(r.Body, maxFingerprintBodyBytes))
	if err != nil {
		http.Error(w, "failed to read body", http.StatusBadRequest)
		return
	}

	var payload map[string]any
	if err := json.Unmarshal(body, &payload); err != nil {
		http.Error(w, "invalid json", http.StatusBadRequest)
		return
	}

	// Extract the 32-char fingerprint identifier
	fpVal, ok := payload["thumbmark"].(string)
	if !ok || fpVal == "" {
		http.Error(w, "missing or invalid thumbmark", http.StatusBadRequest)
		return
	}

	// 1. Aggressive Caching: check if we've seen this fingerprint recently
	if _, seen := knownFingerprints.Load(fpVal); seen {
		// We already know about this device, return 202 quickly
		w.WriteHeader(http.StatusAccepted)
		return
	}

	// 2. Not seen yet, insert into Postgres as a new 'client_fingerprint'
	payloadBytes, _ := json.Marshal(payload)

	// We use ON CONFLICT DO NOTHING to handle concurrent requests
	_, err = a.db.SQL().ExecContext(r.Context(),
		`INSERT INTO fingerprints (id, type, raw_data, created_at)
		 VALUES (?, 'client_fingerprint', ?, datetime('now'))
		 ON CONFLICT (id) DO NOTHING`,
		fpVal, string(payloadBytes))

	if err != nil {
		logging.Printf("[telemetry] failed to store fingerprint: %v", err)
		http.Error(w, "internal error", http.StatusInternalServerError)
		return
	}

	// 3. Mark as known so future requests in the same process skip the DB query
	knownFingerprints.Store(fpVal, time.Now())

	w.WriteHeader(http.StatusAccepted)
}

type FingerprintResponse struct {
	ID        string `json:"id"`
	Type      string `json:"type"`
	RawData   any    `json:"raw_data"`
	CreatedAt string `json:"created_at"`
}

// listFingerprints handles GET /v1/telemetry/fingerprints
func (a *API) listFingerprints(w http.ResponseWriter, r *http.Request) {
	limit := 100
	var cursor string
	if c := r.URL.Query().Get("cursor"); c != "" {
		cursor = c
	}

	query := `SELECT id, type, raw_data, created_at FROM fingerprints WHERE id > ? ORDER BY id ASC LIMIT ?`
	rows, err := a.db.SQL().QueryContext(r.Context(), query, cursor, limit+1)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}
	defer rows.Close()

	var fps []FingerprintResponse
	for rows.Next() {
		var fp FingerprintResponse
		var rawDataStr string
		if err := rows.Scan(&fp.ID, &fp.Type, &rawDataStr, &fp.CreatedAt); err != nil {
			continue
		}
		json.Unmarshal([]byte(rawDataStr), &fp.RawData)
		fps = append(fps, fp)
	}

	var nextCursor string
	if len(fps) > limit {
		fps = fps[:limit]
		nextCursor = fps[len(fps)-1].ID
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: fps, NextCursor: nextCursor})
}
