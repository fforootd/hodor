package logging

import (
	"context"
	"database/sql"
	"time"
)

// Drainer batch-flushes records from the local cache to the analytics backend.
// It uses a circuit breaker on the destination to handle backend outages
// gracefully — the cache accumulates locally and drains when the backend recovers.
type Drainer struct {
	cache    *Cache
	dest     *sql.DB
	interval time.Duration
	batch    int
	cb       *CircuitBreaker
}

// NewDrainer creates a drainer that flushes from cache to the analytics backend.
func NewDrainer(cache *Cache, dest *sql.DB, interval time.Duration, batch int) *Drainer {
	return &Drainer{
		cache:    cache,
		dest:     dest,
		interval: interval,
		batch:    batch,
		cb:       NewCircuitBreaker(5, 30*time.Second),
	}
}

// Run starts the drain loop. It blocks until ctx is cancelled.
// On shutdown (ctx.Done), it performs one final drain attempt.
func (d *Drainer) Run(ctx context.Context) {
	ticker := time.NewTicker(d.interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			d.flush()
		case <-ctx.Done():
			// Final drain on shutdown — best effort.
			d.flush()
			return
		}
	}
}

// flush reads a batch from the cache and inserts into the analytics backend.
func (d *Drainer) flush() {
	if !d.cb.Allow() {
		return // backend is down, skip
	}

	records, err := d.cache.ReadBatch(d.batch)
	if err != nil || len(records) == 0 {
		return
	}

	// Batch insert into the events table.
	tx, err := d.dest.Begin()
	if err != nil {
		d.cb.RecordFailure()
		return
	}

	stmt, err := tx.Prepare(
		`INSERT INTO events (id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, span_id, parent_span_id, session_id, flow_id, created_at)
		 VALUES (?, ?, ?, '0', ?, '', '', ?, ?, '{}', ?, ?, '', ?, ?, ?)`)
	if err != nil {
		tx.Rollback()
		d.cb.RecordFailure()
		return
	}
	defer stmt.Close()

	var ids []int64
	for _, rec := range records {
		eventID := generateDrainID(rec.ID)
		// Ensure payload is valid JSON.
		payload := rec.Payload
		if payload == "" {
			payload = "{}"
		}
		_, err := stmt.Exec(
			eventID, rec.EventType, rec.Category, rec.ActorID,
			rec.Stream, payload, rec.TraceID, rec.SpanID, rec.SessionID, rec.FlowID, rec.CreatedAt,
		)
		if err != nil {
			tx.Rollback()
			d.cb.RecordFailure()
			return
		}
		ids = append(ids, rec.ID)
	}

	if err := tx.Commit(); err != nil {
		d.cb.RecordFailure()
		return
	}

	d.cb.RecordSuccess()

	// Delete successfully drained records from the cache.
	if err := d.cache.Delete(ids); err != nil {
		return
	}

	// Trim to enforce ring buffer max.
	_ = d.cache.Trim()
}

// generateDrainID creates a unique event ID for drained log records.
// Prefixed with "log_" to distinguish from domain event IDs.
func generateDrainID(cacheID int64) string {
	return "log_" + time.Now().Format("20060102150405") + "_" + itoa(cacheID)
}

func itoa(n int64) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	neg := n < 0
	if neg {
		n = -n
	}
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}
