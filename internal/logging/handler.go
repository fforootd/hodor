package logging

import (
	"context"
	"log/slog"
	"sync/atomic"
	"time"
)

// guardedHandler wraps a slog.Handler with an optional circuit breaker
// to isolate non-critical sinks from affecting OLTP stability.
type guardedHandler struct {
	handler slog.Handler
	sink    Sink
	cb      *CircuitBreaker // nil for stdout (never trip)
}

// FanOutHandler dispatches each log record to multiple slog.Handlers.
// Records are redacted before being sent to any handler.
// Non-critical sinks (otel, analytics) are guarded by circuit breakers.
type FanOutHandler struct {
	handlers []guardedHandler
	redactor *Redactor
	level    slog.Level
	attrs    []slog.Attr
	groups   []string
}

// Enabled reports whether the handler handles records at the given level.
func (h *FanOutHandler) Enabled(_ context.Context, level slog.Level) bool {
	return level >= h.level
}

// Handle fans out a log record to all configured and available sinks.
// Redaction is applied once, before dispatching.
// Circuit-breaker-tripped sinks are silently skipped.
func (h *FanOutHandler) Handle(ctx context.Context, r slog.Record) error {
	// Apply redaction to the record.
	redacted := h.redactor.RedactRecord(r)

	for i := range h.handlers {
		g := &h.handlers[i]

		// Check circuit breaker.
		if g.cb != nil && !g.cb.Allow() {
			continue
		}

		if err := g.handler.Handle(ctx, redacted); err != nil {
			if g.cb != nil {
				g.cb.RecordFailure()
			}
			// Never return error from fan-out — don't let one sink kill the others.
			continue
		}

		if g.cb != nil {
			g.cb.RecordSuccess()
		}
	}
	return nil
}

// WithAttrs returns a new FanOutHandler with the given attributes.
func (h *FanOutHandler) WithAttrs(attrs []slog.Attr) slog.Handler {
	newHandlers := make([]guardedHandler, len(h.handlers))
	for i, g := range h.handlers {
		newHandlers[i] = guardedHandler{
			handler: g.handler.WithAttrs(attrs),
			sink:    g.sink,
			cb:      g.cb,
		}
	}
	return &FanOutHandler{
		handlers: newHandlers,
		redactor: h.redactor,
		level:    h.level,
		attrs:    append(h.attrs, attrs...),
		groups:   h.groups,
	}
}

// WithGroup returns a new FanOutHandler with the given group name.
func (h *FanOutHandler) WithGroup(name string) slog.Handler {
	newHandlers := make([]guardedHandler, len(h.handlers))
	for i, g := range h.handlers {
		newHandlers[i] = guardedHandler{
			handler: g.handler.WithGroup(name),
			sink:    g.sink,
			cb:      g.cb,
		}
	}
	return &FanOutHandler{
		handlers: newHandlers,
		redactor: h.redactor,
		level:    h.level,
		attrs:    h.attrs,
		groups:   append(h.groups, name),
	}
}

// --- Noop handler for disabled streams ---

type noopHandler struct{}

func newNoopHandler() *noopHandler                                   { return &noopHandler{} }
func (h *noopHandler) Enabled(_ context.Context, _ slog.Level) bool  { return false }
func (h *noopHandler) Handle(_ context.Context, _ slog.Record) error { return nil }
func (h *noopHandler) WithAttrs(_ []slog.Attr) slog.Handler          { return h }
func (h *noopHandler) WithGroup(_ string) slog.Handler               { return h }

// --- Circuit Breaker ---

// CircuitBreaker provides lightweight isolation for non-critical log sinks.
// When a sink fails consecutively (e.g., OTEL collector down), the breaker
// opens and skips the sink for a cooldown period, preventing blocking.
//
// States: closed (normal) → open (failing) → half-open (probing).
type CircuitBreaker struct {
	maxFailures int32
	cooldown    time.Duration

	failures    atomic.Int32
	state       atomic.Int32 // 0=closed, 1=open, 2=half-open
	lastFailure atomic.Int64 // unix nano
}

const (
	cbClosed   int32 = 0
	cbOpen     int32 = 1
	cbHalfOpen int32 = 2
)

// NewCircuitBreaker creates a circuit breaker with the given failure threshold
// and cooldown duration.
func NewCircuitBreaker(maxFailures int, cooldown time.Duration) *CircuitBreaker {
	return &CircuitBreaker{
		maxFailures: int32(maxFailures),
		cooldown:    cooldown,
	}
}

// Allow checks whether the sink should be called.
func (cb *CircuitBreaker) Allow() bool {
	switch cb.state.Load() {
	case cbClosed:
		return true
	case cbOpen:
		// Check if cooldown has elapsed.
		lastFail := time.Unix(0, cb.lastFailure.Load())
		if time.Since(lastFail) > cb.cooldown {
			cb.state.Store(cbHalfOpen)
			return true // Probe with one request.
		}
		return false
	case cbHalfOpen:
		return true // Allow probe.
	default:
		return true
	}
}

// RecordFailure records a failed attempt. Opens the breaker after maxFailures.
func (cb *CircuitBreaker) RecordFailure() {
	cb.lastFailure.Store(time.Now().UnixNano())
	n := cb.failures.Add(1)
	if n >= cb.maxFailures {
		cb.state.Store(cbOpen)
	}
}

// RecordSuccess records a successful attempt. Resets the breaker.
func (cb *CircuitBreaker) RecordSuccess() {
	if cb.state.Load() == cbHalfOpen {
		// Probe succeeded — close the breaker.
		cb.state.Store(cbClosed)
		cb.failures.Store(0)
	} else if cb.state.Load() == cbClosed {
		// Reset consecutive failure counter on success.
		cb.failures.Store(0)
	}
}

// State returns the current circuit breaker state as a string.
func (cb *CircuitBreaker) State() string {
	switch cb.state.Load() {
	case cbOpen:
		return "open"
	case cbHalfOpen:
		return "half-open"
	default:
		return "closed"
	}
}

// Failures returns the current consecutive failure count.
func (cb *CircuitBreaker) Failures() int {
	return int(cb.failures.Load())
}
