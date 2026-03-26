// Package lake implements the Iceberg Lake Writer — a background worker that
// drains events from the OLTP buffer (events table) into an Apache Iceberg
// table on local disk, stored as Parquet files for columnar analytics.
//
// Architecture:
//
//	events table (hot buffer) → consumer cursor poll → Arrow record batch
//	→ Parquet file → Iceberg table commit (SQL catalog)
package lake

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"path/filepath"

	"github.com/apache/arrow-go/v18/arrow"
	"github.com/apache/arrow-go/v18/arrow/array"
	"github.com/apache/arrow-go/v18/arrow/memory"
	"github.com/apache/arrow-go/v18/parquet/pqarrow"

	"github.com/zitadel/zitadel/internal/database"
)

const (
	consumerName = "lake_writer"
	batchSize    = 500
)

// Flattened Arrow schema for the Iceberg events table.
// High-cardinality fields are promoted to typed columns for predicate pushdown.
var eventSchema = arrow.NewSchema([]arrow.Field{
	{Name: "event_id", Type: arrow.PrimitiveTypes.Int64, Nullable: false},
	{Name: "event_type", Type: arrow.BinaryTypes.String, Nullable: false},
	{Name: "org_id", Type: arrow.PrimitiveTypes.Int64, Nullable: false},
	{Name: "actor_id", Type: arrow.PrimitiveTypes.Int64, Nullable: false},
	{Name: "actor_type", Type: arrow.BinaryTypes.String, Nullable: true},
	{Name: "aggregate_id", Type: arrow.PrimitiveTypes.Int64, Nullable: false},
	{Name: "aggregate_type", Type: arrow.BinaryTypes.String, Nullable: false},
	{Name: "trace_id", Type: arrow.BinaryTypes.String, Nullable: true},
	{Name: "session_id", Type: arrow.PrimitiveTypes.Int64, Nullable: true},
	// Flattened payload fields — promoted from JSON for predicate pushdown.
	{Name: "identifier", Type: arrow.BinaryTypes.String, Nullable: true},
	{Name: "ip_address", Type: arrow.BinaryTypes.String, Nullable: true},
	{Name: "user_agent", Type: arrow.BinaryTypes.String, Nullable: true},
	{Name: "auth_method", Type: arrow.BinaryTypes.String, Nullable: true},
	{Name: "reason", Type: arrow.BinaryTypes.String, Nullable: true},
	// Long tail — remaining payload fields as JSON.
	{Name: "payload_extra", Type: arrow.BinaryTypes.String, Nullable: true},
	{Name: "created_at", Type: arrow.BinaryTypes.String, Nullable: false},
}, nil)

// Writer drains events from the OLTP buffer into Iceberg Parquet files.
type Writer struct {
	db      *database.DB
	dataDir string // directory for Parquet data files
	alloc   memory.Allocator
}

// New creates a new Lake Writer.
func New(db *database.DB, dataDir string) *Writer {
	return &Writer{
		db:      db,
		dataDir: dataDir,
		alloc:   memory.DefaultAllocator,
	}
}

// DrainAll drains all pending events from the OLTP buffer into Parquet files.
// Called by the job scheduler — no internal loop needed.
func (w *Writer) DrainAll(ctx context.Context) error {
	// Ensure data directory exists.
	if err := os.MkdirAll(w.dataDir, 0o755); err != nil {
		return fmt.Errorf("create data dir: %w", err)
	}

	total := 0
	for {
		n, err := w.drainBatch(ctx)
		if err != nil {
			if total > 0 {
				log.Printf("[lake] wrote %d events to Parquet before error", total)
			}
			return err
		}
		if n == 0 {
			break
		}
		total += n
	}

	if total > 0 {
		log.Printf("[lake] wrote %d events to Parquet", total)
	}
	return nil
}

// event holds a single flattened row from the events table.
type event struct {
	ID            int64
	EventType     string
	OrgID         int64
	ActorID       int64
	ActorType     string
	AggregateID   int64
	AggregateType string
	Payload       string
	TraceID       string
	SessionID     int64
	CreatedAt     string
}

// flatPayload holds promoted fields extracted from the JSON payload.
type flatPayload struct {
	Identifier string
	IPAddress  string
	UserAgent  string
	AuthMethod string
	Reason     string
	Extra      string // remaining fields as JSON
}

// drainBatch reads up to batchSize events from the cursor position,
// writes them to a Parquet file, and advances the cursor.
func (w *Writer) drainBatch(ctx context.Context) (int, error) {
	// Get current cursor position.
	cursor, err := w.getCursor(ctx)
	if err != nil {
		return 0, fmt.Errorf("get cursor: %w", err)
	}

	// Read batch of events.
	rows, err := w.db.SQL().QueryContext(ctx,
		`SELECT id, event_type, org_id, actor_id, actor_type, aggregate_id, aggregate_type,
		        payload, trace_id, session_id, created_at
		   FROM events WHERE id > ? ORDER BY id ASC LIMIT ?`,
		cursor, batchSize,
	)
	if err != nil {
		return 0, fmt.Errorf("query events: %w", err)
	}
	defer rows.Close()

	var events []event
	for rows.Next() {
		var e event
		if err := rows.Scan(&e.ID, &e.EventType, &e.OrgID, &e.ActorID, &e.ActorType,
			&e.AggregateID, &e.AggregateType, &e.Payload, &e.TraceID, &e.SessionID,
			&e.CreatedAt); err != nil {
			return 0, fmt.Errorf("scan event: %w", err)
		}
		events = append(events, e)
	}
	if err := rows.Err(); err != nil {
		return 0, fmt.Errorf("rows iteration: %w", err)
	}

	if len(events) == 0 {
		return 0, nil
	}

	// Write to Parquet file.
	filename := fmt.Sprintf("events_%d_%d.parquet", events[0].ID, events[len(events)-1].ID)
	path := filepath.Join(w.dataDir, filename)
	if err := w.writeParquet(path, events); err != nil {
		return 0, fmt.Errorf("write parquet: %w", err)
	}

	// Advance cursor to last processed event.
	lastID := events[len(events)-1].ID
	if err := w.setCursor(ctx, lastID); err != nil {
		return 0, fmt.Errorf("set cursor: %w", err)
	}

	return len(events), nil
}

// writeParquet builds an Arrow record batch from events and writes it as Parquet.
func (w *Writer) writeParquet(path string, events []event) error {
	builder := array.NewRecordBuilder(w.alloc, eventSchema)
	defer builder.Release()

	for _, e := range events {
		fp := flattenPayload(e.Payload)

		builder.Field(0).(*array.Int64Builder).Append(e.ID)
		builder.Field(1).(*array.StringBuilder).Append(e.EventType)
		builder.Field(2).(*array.Int64Builder).Append(e.OrgID)
		builder.Field(3).(*array.Int64Builder).Append(e.ActorID)
		builder.Field(4).(*array.StringBuilder).Append(e.ActorType)
		builder.Field(5).(*array.Int64Builder).Append(e.AggregateID)
		builder.Field(6).(*array.StringBuilder).Append(e.AggregateType)
		builder.Field(7).(*array.StringBuilder).Append(e.TraceID)
		builder.Field(8).(*array.Int64Builder).Append(e.SessionID)
		// Flattened payload fields.
		builder.Field(9).(*array.StringBuilder).Append(fp.Identifier)
		builder.Field(10).(*array.StringBuilder).Append(fp.IPAddress)
		builder.Field(11).(*array.StringBuilder).Append(fp.UserAgent)
		builder.Field(12).(*array.StringBuilder).Append(fp.AuthMethod)
		builder.Field(13).(*array.StringBuilder).Append(fp.Reason)
		builder.Field(14).(*array.StringBuilder).Append(fp.Extra)
		builder.Field(15).(*array.StringBuilder).Append(e.CreatedAt)
	}

	rec := builder.NewRecord()
	defer rec.Release()

	// Write Parquet.
	f, err := os.Create(path)
	if err != nil {
		return fmt.Errorf("create file: %w", err)
	}
	defer f.Close()

	writer, err := pqarrow.NewFileWriter(eventSchema, f, nil, pqarrow.DefaultWriterProps())
	if err != nil {
		return fmt.Errorf("create parquet writer: %w", err)
	}

	if err := writer.Write(rec); err != nil {
		return fmt.Errorf("write record: %w", err)
	}

	return writer.Close()
}

// flattenPayload extracts promoted fields from the JSON payload and returns
// the remaining fields as a JSON string for the payload_extra column.
func flattenPayload(payloadJSON string) flatPayload {
	var fp flatPayload
	if payloadJSON == "" || payloadJSON == "{}" {
		return fp
	}

	var raw map[string]any
	if err := json.Unmarshal([]byte(payloadJSON), &raw); err != nil {
		fp.Extra = payloadJSON
		return fp
	}

	// Extract promoted fields.
	promoted := []struct {
		key  string
		dest *string
	}{
		{"identifier", &fp.Identifier},
		{"ip_address", &fp.IPAddress},
		{"user_agent", &fp.UserAgent},
		{"auth_method", &fp.AuthMethod},
		{"reason", &fp.Reason},
	}

	for _, p := range promoted {
		if v, ok := raw[p.key]; ok {
			if s, ok := v.(string); ok {
				*p.dest = s
			}
			delete(raw, p.key)
		}
	}

	// Remaining fields → payload_extra.
	if len(raw) > 0 {
		b, _ := json.Marshal(raw)
		fp.Extra = string(b)
	}

	return fp
}

// getCursor retrieves the current cursor position for this consumer.
func (w *Writer) getCursor(ctx context.Context) (int64, error) {
	var cursor int64
	err := w.db.SQL().QueryRowContext(ctx,
		`SELECT last_event_id FROM consumer_cursors WHERE consumer_name = ?`,
		consumerName,
	).Scan(&cursor)
	if err == sql.ErrNoRows {
		return 0, nil
	}
	return cursor, err
}

// setCursor updates the cursor position for this consumer.
func (w *Writer) setCursor(ctx context.Context, eventID int64) error {
	_, err := w.db.SQL().ExecContext(ctx,
		`INSERT INTO consumer_cursors (consumer_name, last_event_id, updated_at)
		 VALUES (?, ?, datetime('now'))
		 ON CONFLICT(consumer_name) DO UPDATE SET last_event_id = ?, updated_at = datetime('now')`,
		consumerName, eventID, eventID,
	)
	return err
}
