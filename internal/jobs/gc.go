package jobs

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/id"
)

// emitGCEvent emits an audit event for GC operations.
func emitGCEvent(ctx context.Context, db *database.DB, bus *eventbus.Bus, eventType string, payload map[string]any) {
	eventID := id.New()
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	_, _ = db.SQL().ExecContext(ctx,
		`INSERT INTO events (id, event_type, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, trace_id, span_id, parent_span_id, session_id, created_at)
		 VALUES (?, ?, '0', '', 'system', '', 'gc', ?, '{}', '', '', '', '', datetime('now'))`,
		eventID, eventType, payloadJSON)
	bus.Signal()
}

// SessionGC returns a job function that cleans up revoked and expired sessions.
func SessionGC(db *database.DB, bus *eventbus.Bus) JobFunc {
	return func(ctx context.Context) error {
		ttl := getRetentionTTL(ctx, db, "session.*")
		if ttl <= 0 {
			ttl = 7 * 24 * time.Hour
		}

		cutoff := time.Now().UTC().Add(-ttl).Format(time.RFC3339)

		res, err := db.SQL().ExecContext(ctx,
			`DELETE FROM sessions WHERE revoked_at IS NOT NULL AND revoked_at < ?`, cutoff,
		)
		if err != nil {
			return fmt.Errorf("gc revoked sessions: %w", err)
		}
		revokedCount, _ := res.RowsAffected()

		res, err = db.SQL().ExecContext(ctx,
			`DELETE FROM sessions WHERE expires_at < ?`, cutoff,
		)
		if err != nil {
			return fmt.Errorf("gc expired sessions: %w", err)
		}
		expiredCount, _ := res.RowsAffected()

		total := revokedCount + expiredCount
		if total > 0 {
			log.Printf("[session_gc] deleted %d revoked + %d expired sessions (ttl=%s)",
				revokedCount, expiredCount, FormatDuration(ttl))

			emitGCEvent(ctx, db, bus, "gc.sessions_cleaned", map[string]any{
				"revoked_count": revokedCount,
				"expired_count": expiredCount,
				"ttl":           FormatDuration(ttl),
				"cutoff":        cutoff,
			})
		}
		return nil
	}
}

// EventGC returns a job function that deletes OLTP events past their retention period.
func EventGC(db *database.DB, bus *eventbus.Bus) JobFunc {
	return func(ctx context.Context) error {
		var lakeCursor string
		err := db.SQL().QueryRowContext(ctx,
			`SELECT last_event_id FROM consumer_cursors WHERE consumer_name = 'lake_writer'`,
		).Scan(&lakeCursor)
		if err == sql.ErrNoRows {
			return nil
		}
		if err != nil {
			return fmt.Errorf("get lake cursor: %w", err)
		}

		policies, err := loadRetentionPolicies(ctx, db)
		if err != nil {
			return fmt.Errorf("load retention: %w", err)
		}

		var totalDeleted int64
		deletedByPattern := map[string]int64{}
		for _, p := range policies {
			ttl := ParseTTL(p.OLTPTTL)
			if ttl <= 0 {
				continue
			}

			cutoff := time.Now().UTC().Add(-ttl).Format(time.RFC3339)

			res, err := db.SQL().ExecContext(ctx,
				`DELETE FROM events
				 WHERE event_type GLOB ?
				   AND created_at < ?
				   AND id <= ?`,
				p.EventPattern, cutoff, lakeCursor,
			)
			if err != nil {
				log.Printf("[event_gc] pattern %q error: %v", p.EventPattern, err)
				continue
			}
			n, _ := res.RowsAffected()
			totalDeleted += n
			if n > 0 {
				deletedByPattern[p.EventPattern] = n
			}
		}

		if totalDeleted > 0 {
			log.Printf("[event_gc] deleted %d events past retention (cursor=%s)", totalDeleted, lakeCursor)

			emitGCEvent(ctx, db, bus, "gc.events_cleaned", map[string]any{
				"total_deleted":      totalDeleted,
				"deleted_by_pattern": deletedByPattern,
				"lake_cursor":        lakeCursor,
			})
		}
		return nil
	}
}

// retentionPolicy holds a row from the retention_policies table.
type retentionPolicy struct {
	EventPattern string
	OLTPTTL      string
	LakeTTL      string
	Priority     int
}

func loadRetentionPolicies(ctx context.Context, db *database.DB) ([]retentionPolicy, error) {
	rows, err := db.SQL().QueryContext(ctx,
		`SELECT event_pattern, oltp_ttl, lake_ttl, priority
		 FROM retention_policies ORDER BY priority DESC`,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var policies []retentionPolicy
	for rows.Next() {
		var p retentionPolicy
		if err := rows.Scan(&p.EventPattern, &p.OLTPTTL, &p.LakeTTL, &p.Priority); err != nil {
			continue
		}
		policies = append(policies, p)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("retention policy rows: %w", err)
	}
	return policies, nil
}

func getRetentionTTL(ctx context.Context, db *database.DB, pattern string) time.Duration {
	var ttl string
	err := db.SQL().QueryRowContext(ctx,
		`SELECT oltp_ttl FROM retention_policies
		 WHERE ? GLOB event_pattern
		 ORDER BY priority DESC LIMIT 1`,
		pattern,
	).Scan(&ttl)
	if err != nil {
		return 14 * 24 * time.Hour
	}
	return ParseTTL(ttl)
}
