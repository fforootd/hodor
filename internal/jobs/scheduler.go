// Package jobs implements a lightweight cron scheduler backed by the jobs table.
// Inspired by River's DB-backed approach, but on SQLite for single-binary deployment.
//
// Each job is a named function registered at startup. The scheduler ticks every
// 30 seconds, checks next_run_at for enabled jobs, and runs them.
package jobs

import (
	"context"
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
)

// JobFunc is the signature for a job implementation.
type JobFunc func(ctx context.Context) error

// Scheduler manages background jobs using the jobs table as state.
type Scheduler struct {
	db       *database.DB
	registry map[string]JobFunc
	tick     time.Duration
}

// New creates a new Scheduler.
func New(db *database.DB) *Scheduler {
	return &Scheduler{
		db:       db,
		registry: make(map[string]JobFunc),
		tick:     30 * time.Second,
	}
}

// Register adds a named job function. Must be called before Run.
func (s *Scheduler) Register(name string, fn JobFunc) {
	s.registry[name] = fn
}

// Run starts the scheduler loop. Blocks until ctx is cancelled.
func (s *Scheduler) Run(ctx context.Context) {
	logging.Printf("[scheduler] started with %d registered jobs", len(s.registry))

	// Initialize next_run_at for any jobs that don't have one.
	s.initNextRun()

	ticker := time.NewTicker(s.tick)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			logging.Println("[scheduler] shutting down")
			return
		case <-ticker.C:
			s.checkAndRun(ctx)
		}
	}
}

// initNextRun sets next_run_at for jobs that don't have one yet.
func (s *Scheduler) initNextRun() {
	now := time.Now().UTC()
	rows, err := s.db.SQL().Query(
		`SELECT instance_id, name, cron FROM jobs WHERE enabled = 1 AND (next_run_at IS NULL OR next_run_at = '')`,
	)
	if err != nil {
		logging.Printf("[scheduler] init error: %v", err)
		return
	}
	defer rows.Close()

	for rows.Next() {
		var instanceID, name, cron string
		if err := rows.Scan(&instanceID, &name, &cron); err != nil {
			continue
		}
		next := nextCronTime(now, cron)
		_, _ = s.db.SQL().Exec(
			`UPDATE jobs SET next_run_at = ? WHERE instance_id = ? AND name = ?`,
			next.Format(time.RFC3339), instanceID, name,
		)
	}
	if err := rows.Err(); err != nil {
		logging.Printf("[scheduler] init rows error: %v", err)
	}
}

// checkAndRun checks all enabled jobs and runs any that are due.
func (s *Scheduler) checkAndRun(ctx context.Context) {
	now := time.Now().UTC()

	rows, err := s.db.SQL().QueryContext(ctx,
		`SELECT instance_id, name, cron FROM jobs
		 WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= ?`,
		now.Format(time.RFC3339),
	)
	if err != nil {
		logging.Printf("[scheduler] check error: %v", err)
		return
	}
	defer rows.Close()

	type dueJob struct {
		instanceID string
		name       string
		cron       string
	}
	var due []dueJob
	for rows.Next() {
		var j dueJob
		if err := rows.Scan(&j.instanceID, &j.name, &j.cron); err != nil {
			continue
		}
		due = append(due, j)
	}
	if err := rows.Err(); err != nil {
		logging.Printf("[scheduler] rows error: %v", err)
		return
	}
	rows.Close()

	for _, j := range due {
		fn, ok := s.registry[j.name]
		if !ok {
			continue // No handler registered for this job.
		}

		// Mark as running.
		_, _ = s.db.SQL().ExecContext(ctx,
			`UPDATE jobs SET last_status = 'running', last_run_at = ? WHERE instance_id = ? AND name = ?`,
			now.Format(time.RFC3339), j.instanceID, j.name,
		)

		// Run the job.
		err := fn(httputil.WithInstanceID(ctx, j.instanceID))

		// Update status.
		status := "success"
		errMsg := ""
		if err != nil {
			status = "error"
			errMsg = err.Error()
			logging.Printf("[scheduler] job %q failed: %v", j.name, err)
		} else {
			logging.Printf("[scheduler] job %q completed", j.name)
		}

		next := nextCronTime(time.Now().UTC(), j.cron)
		_, _ = s.db.SQL().ExecContext(ctx,
			`UPDATE jobs SET last_status = ?, last_error = ?, run_count = run_count + 1, next_run_at = ? WHERE instance_id = ? AND name = ?`,
			status, errMsg, next.Format(time.RFC3339), j.instanceID, j.name,
		)
	}
}

// nextCronTime calculates the next run time from a simple cron expression.
// Supports: */N for intervals, specific values, and * for any.
// Format: minute hour day_of_month month day_of_week (5-field cron).
func nextCronTime(from time.Time, cronExpr string) time.Time {
	parts := strings.Fields(cronExpr)
	if len(parts) != 5 {
		return from.Add(5 * time.Minute) // fallback
	}

	// Parse minute field for interval.
	minutePart := parts[0]
	hourPart := parts[1]

	// Simple interval: */N means every N minutes.
	if strings.HasPrefix(minutePart, "*/") {
		n, err := strconv.Atoi(minutePart[2:])
		if err != nil || n <= 0 {
			n = 5
		}
		return from.Add(time.Duration(n) * time.Minute)
	}

	// Specific minute: "0" means at minute 0 of the hour.
	if minutePart != "*" && hourPart == "*" {
		m, err := strconv.Atoi(minutePart)
		if err != nil {
			return from.Add(5 * time.Minute)
		}
		// Next occurrence of minute M.
		t := time.Date(from.Year(), from.Month(), from.Day(), from.Hour(), m, 0, 0, time.UTC)
		if !t.After(from) {
			t = t.Add(time.Hour)
		}
		return t
	}

	// Default: every 5 minutes.
	return from.Add(5 * time.Minute)
}

// ParseTTL converts a TTL string like "7d", "24h", "30d" to a time.Duration.
// Returns 0 for "0" or "forever" (meaning never expire).
func ParseTTL(ttl string) time.Duration {
	ttl = strings.TrimSpace(strings.ToLower(ttl))
	if ttl == "0" || ttl == "forever" || ttl == "" {
		return 0
	}

	if strings.HasSuffix(ttl, "d") {
		n, err := strconv.Atoi(ttl[:len(ttl)-1])
		if err != nil {
			return 14 * 24 * time.Hour // default 14 days
		}
		return time.Duration(n) * 24 * time.Hour
	}

	if strings.HasSuffix(ttl, "h") {
		n, err := strconv.Atoi(ttl[:len(ttl)-1])
		if err != nil {
			return 24 * time.Hour
		}
		return time.Duration(n) * time.Hour
	}

	// Try parsing as Go duration directly.
	d, err := time.ParseDuration(ttl)
	if err != nil {
		return 14 * 24 * time.Hour
	}
	return d
}

// FormatDuration formats a duration as a human-readable string.
func FormatDuration(d time.Duration) string {
	if d <= 0 {
		return "forever"
	}
	days := int(d.Hours() / 24)
	if days > 0 {
		return fmt.Sprintf("%dd", days)
	}
	return d.String()
}
