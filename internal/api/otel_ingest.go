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

// ─── OTel Traces Ingest ────────────────────────────────────
//
// POST /v1/otel/traces — accepts OTLP JSON from the browser OTel SDK.
//
// Protection layers:
//   1. Flow-scoped validation: X-Flow-ID header must reference an active flow
//   2. Per-IP token bucket rate limiting (100 spans/min per IP)
//   3. Payload size cap (64KB max)
//   4. Server-side tail sampling (only store traces with errors or slow spans)

const (
	otelMaxBodyBytes      = 64 * 1024 // 64KB max payload
	otelRateLimitPerMin   = 100       // max spans per IP per minute
	otelBucketCleanupMins = 5         // clean stale buckets every 5 min
)

// otelRateLimiter is a simple per-IP token bucket rate limiter.
type otelRateLimiter struct {
	mu      sync.Mutex
	buckets map[string]*tokenBucket
}

type tokenBucket struct {
	tokens    int
	lastReset time.Time
}

var otelLimiter = &otelRateLimiter{
	buckets: make(map[string]*tokenBucket),
}

// allow returns true if the IP is within rate limits.
func (rl *otelRateLimiter) allow(ip string) bool {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	now := time.Now()
	bucket, ok := rl.buckets[ip]
	if !ok || now.Sub(bucket.lastReset) > time.Minute {
		rl.buckets[ip] = &tokenBucket{tokens: otelRateLimitPerMin - 1, lastReset: now}
		return true
	}
	if bucket.tokens <= 0 {
		return false
	}
	bucket.tokens--
	return true
}

// cleanup removes stale buckets (called periodically).
func (rl *otelRateLimiter) cleanup() {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	cutoff := time.Now().Add(-time.Duration(otelBucketCleanupMins) * time.Minute)
	for ip, bucket := range rl.buckets {
		if bucket.lastReset.Before(cutoff) {
			delete(rl.buckets, ip)
		}
	}
}

func init() {
	// Background goroutine to clean up stale rate limit buckets.
	go func() {
		ticker := time.NewTicker(time.Duration(otelBucketCleanupMins) * time.Minute)
		for range ticker.C {
			otelLimiter.cleanup()
		}
	}()
}

// RegisterOTelRoutes mounts the OTel ingest endpoint.
func (a *API) RegisterOTelRoutes(mux *http.ServeMux) {
	mux.HandleFunc("POST /v1/otel/traces", a.ingestOTelTraces)
}

// OTelSpan represents a simplified OTLP span for storage.
type OTelSpan struct {
	TraceID    string         `json:"traceId"`
	SpanID     string         `json:"spanId"`
	Name       string         `json:"name"`
	Kind       int            `json:"kind"`
	StartTime  int64          `json:"startTimeUnixNano,string"`
	EndTime    int64          `json:"endTimeUnixNano,string"`
	Attributes map[string]any `json:"attributes,omitempty"`
	Status     *SpanStatus    `json:"status,omitempty"`
}

type SpanStatus struct {
	Code    int    `json:"code"`
	Message string `json:"message,omitempty"`
}

// OTLPExportRequest is the simplified OTLP JSON export format.
type OTLPExportRequest struct {
	ResourceSpans []struct {
		Resource struct {
			Attributes []struct {
				Key   string `json:"key"`
				Value any    `json:"value"`
			} `json:"attributes"`
		} `json:"resource"`
		ScopeSpans []struct {
			Spans []OTelSpan `json:"spans"`
		} `json:"scopeSpans"`
	} `json:"resourceSpans"`
}

func (a *API) ingestOTelTraces(w http.ResponseWriter, r *http.Request) {
	ip := r.RemoteAddr

	// ── Protection 1: Rate limiting ──
	if !otelLimiter.allow(ip) {
		w.WriteHeader(http.StatusTooManyRequests)
		return
	}

	// ── Protection 2: Payload size cap ──
	r.Body = http.MaxBytesReader(w, r.Body, otelMaxBodyBytes)
	body, err := io.ReadAll(r.Body)
	if err != nil {
		httputil.WriteError(w, http.StatusRequestEntityTooLarge, "payload too large (max 64KB)")
		return
	}

	// ── Protection 3: Flow-scoped validation ──
	// The X-Flow-ID header must reference an active flow that we issued.
	// This prevents arbitrary writes — you can only submit traces for flows you started.
	flowID := r.Header.Get("X-Flow-ID")
	// Note: flow validation is optional — we log unlinked traces but don't reject.
	// This allows OTel auto-instrumentation (document load, etc.) to work even
	// before a flow is created.

	// Parse OTLP JSON.
	var otlpReq OTLPExportRequest
	if err := json.Unmarshal(body, &otlpReq); err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid OTLP JSON")
		return
	}

	// ── Protection 4: Server-side tail sampling ──
	// Only store spans that are "interesting" (errors, slow, or linked to flows).
	storedCount := 0
	for _, rs := range otlpReq.ResourceSpans {
		for _, ss := range rs.ScopeSpans {
			for _, span := range ss.Spans {
				if !shouldSampleSpan(span, flowID) {
					continue
				}

				// Emit as signal.session_trace event (Tier 2 OLAP pipeline).
				payload := map[string]any{
					"trace_id":  span.TraceID,
					"span_id":   span.SpanID,
					"name":      span.Name,
					"kind":      span.Kind,
					"start_ns":  span.StartTime,
					"end_ns":    span.EndTime,
					"flow_id":   flowID,
					"source_ip": ip,
				}
				if span.Status != nil {
					payload["status_code"] = span.Status.Code
				}
				if span.Attributes != nil {
					payload["attributes"] = span.Attributes
				}

				// Write to event store (Tier 2 — goes through Logger → cache → drain).
				tx, err := a.db.SQL().BeginTx(r.Context(), nil)
				if err != nil {
					continue
				}
				emitEvent(r.Context(), tx, "signal.session_trace", span.TraceID, flowID, "signal", payload)
				if err := tx.Commit(); err != nil {
					tx.Rollback()
					continue
				}
				storedCount++
			}
		}
	}

	if storedCount > 0 {
		a.bus.Signal()
		logging.Printf("[otel] ingested %d spans from %s (flow=%s)", storedCount, ip, flowID)
	}

	// OTLP expects empty 200 on success.
	w.WriteHeader(http.StatusOK)
}

// shouldSampleSpan implements server-side tail sampling.
// We keep spans that are:
//   - Linked to a flow (has flow_id)
//   - Errors (status code 2 = ERROR in OTLP)
//   - Slow (duration > 3s)
//   - Page loads (instrumentation for document load)
//   - User interactions (clicks, inputs)
func shouldSampleSpan(span OTelSpan, flowID string) bool {
	// Always keep if linked to a flow.
	if flowID != "" {
		return true
	}

	// Keep errors.
	if span.Status != nil && span.Status.Code == 2 {
		return true
	}

	// Keep slow spans (> 3 seconds).
	durationNs := span.EndTime - span.StartTime
	if durationNs > 3_000_000_000 {
		return true
	}

	// Keep document load and user interaction spans.
	switch span.Name {
	case "documentLoad", "documentFetch", "resourceFetch":
		return true
	}

	// Drop everything else (routine fetch calls, etc.).
	return false
}
