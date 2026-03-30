// Package logging provides structured, multi-destination logging for Zitadel.
//
// Design (inspired by zitadel/zitadel PR #11435):
//
//   - Streams identify the producer of a log record (runtime, request, jobs, event_pusher).
//   - Sinks identify the destination (stdout, otel, analytics).
//   - Each stream can be routed to any combination of sinks via config.
//   - A FanOutHandler dispatches each record to the configured sinks,
//     with per-sink circuit breakers to isolate non-critical destinations.
//   - A Redactor masks sensitive fields (passwords, tokens, secrets) before output.
//
// Usage:
//
//	logger := logging.New(logging.StreamRuntime)
//	logger.Info("server started", "port", 8080)
//
//	// Context-aware (request_id, session_id auto-attached):
//	logging.Info(ctx, "request handled", "status", 200)
package logging

import (
	"context"
	"database/sql"
	"log/slog"
	"os"
	"strings"
	"sync"
	"time"
)

// Stream identifies the logical producer of a log record.
type Stream string

const (
	// StreamRuntime covers startup, shutdown, config, migration logs.
	// Potentially sensitive: stack traces may leak internal state.
	StreamRuntime Stream = "runtime"

	// StreamRequest covers incoming HTTP/API requests.
	// Sensitive fields: IP addresses, auth headers.
	StreamRequest Stream = "request"

	// StreamJobs covers background job execution (scheduler, GC).
	StreamJobs Stream = "jobs"

	// StreamEventPusher covers raw event payloads being persisted.
	// Disabled by default — contains full entity payloads.
	StreamEventPusher Stream = "event_pusher"
)

// AllStreams is the list of all known streams.
var AllStreams = []Stream{StreamRuntime, StreamRequest, StreamJobs, StreamEventPusher}

// Sink identifies a log destination.
type Sink string

const (
	// SinkStdout writes to os.Stdout (always available).
	SinkStdout Sink = "stdout"

	// SinkOTEL exports log records as OTLP logs to the customer's collector.
	SinkOTEL Sink = "otel"

	// SinkAnalytics writes log records as events to the analytics backend
	// (events table). Distinct from domain events: uses log.* event types.
	SinkAnalytics Sink = "analytics"
)

// Config holds all logging configuration.
type Config struct {
	Level     string
	Format    string // "text" | "json"
	CachePath string // local SQLite cache file (default: "./data/zitadel-cache.db")
	CacheMax  int    // ring buffer max rows (default: 50000)
	Streams   StreamRouting
	Sinks     SinksConfig
	Redaction RedactionConfig
	DB        *sql.DB // for analytics drain destination (nil = skip drain)
}

// StreamConfig holds per-stream routing and reliability settings.
type StreamConfig struct {
	Sinks      []string
	Mode       string  // "buffered" | "sampled" | "off"
	SampleRate float64 // for "sampled" mode (e.g., 0.01 = 1%)
}

// StreamRouting maps each stream to its configuration.
type StreamRouting struct {
	Runtime     StreamConfig
	Request     StreamConfig
	Jobs        StreamConfig
	EventPusher StreamConfig
}

// SinksConfig holds per-sink configuration.
type SinksConfig struct {
	OTEL      OTELSinkConfig
	Analytics AnalyticsSinkConfig
}

// OTELSinkConfig holds OTEL exporter settings.
type OTELSinkConfig struct {
	Endpoint string
	Protocol string // "grpc" | "http"
}

// AnalyticsSinkConfig holds analytics sink settings.
type AnalyticsSinkConfig struct {
	Enabled       bool
	DrainInterval string // default: "5s"
	DrainBatch    int    // default: 500
}

// RedactionConfig controls field masking.
type RedactionConfig struct {
	Keys   []string
	Mask   string
	IPMode string // "keep" | "redact" | "hash" | "mask"
}

// --- Global state ---

var (
	globalMu            sync.RWMutex
	globalLevel         slog.Level
	globalFormat        string
	globalRouting       map[Stream][]Sink
	globalRedactor      *Redactor
	globalSinks         map[Sink]slog.Handler
	globalStreamCfg     map[Stream]StreamConfig
	globalCache         *Cache
	globalAnalyticsCfg  *AnalyticsSinkConfig // saved for deferred drainer activation
	globalDrainerCancel context.CancelFunc   // cancel handle for running drainer
	initialized         bool
)

// Init configures the global logging system. Must be called once at startup
// before any loggers are created. Safe to call multiple times (last wins).
func Init(cfg Config) {
	globalMu.Lock()
	defer globalMu.Unlock()

	globalLevel = parseLevel(cfg.Level)
	globalFormat = strings.ToLower(cfg.Format)

	// Build redactor.
	globalRedactor = NewRedactorWithIP(cfg.Redaction.Keys, cfg.Redaction.Mask, cfg.Redaction.IPMode)

	// Build sink handlers.
	globalSinks = make(map[Sink]slog.Handler)
	globalSinks[SinkStdout] = newStdoutHandler(globalFormat, globalLevel)

	if cfg.Sinks.OTEL.Endpoint != "" {
		globalSinks[SinkOTEL] = newOTELHandler(cfg.Sinks.OTEL.Endpoint, cfg.Sinks.OTEL.Protocol, globalLevel)
	}

	// Open local cache for analytics buffering.
	if cfg.Sinks.Analytics.Enabled {
		cachePath := cfg.CachePath
		if cachePath == "" {
			cachePath = "./data/zitadel-cache.db"
		}
		cacheMax := cfg.CacheMax
		if cacheMax == 0 {
			cacheMax = 50000
		}
		cache, err := OpenCache(cachePath, cacheMax)
		if err == nil {
			globalCache = cache
			// The cache sink handler is created per-stream in New() based on stream mode.
		}

		// Save analytics config for deferred drainer activation.
		analyticsCfg := cfg.Sinks.Analytics
		globalAnalyticsCfg = &analyticsCfg

		// Start drainer if we have both cache and a destination DB.
		if globalCache != nil && cfg.DB != nil {
			startDrainer(cfg.DB, cfg.Sinks.Analytics)
		} else if globalCache != nil && cfg.DB == nil {
			// Warn early: analytics is buffering to cache but drainer
			// can't start until ActivateDrainer(db) is called after DB open.
			slog.Warn("analytics sink enabled but DB not yet available — call logging.ActivateDrainer(db) after database opens")
		}
	}

	// Store per-stream config.
	globalStreamCfg = map[Stream]StreamConfig{
		StreamRuntime:     cfg.Streams.Runtime,
		StreamRequest:     cfg.Streams.Request,
		StreamJobs:        cfg.Streams.Jobs,
		StreamEventPusher: cfg.Streams.EventPusher,
	}

	// Build stream → sink routing.
	globalRouting = make(map[Stream][]Sink)
	globalRouting[StreamRuntime] = parseSinks(cfg.Streams.Runtime.Sinks, []string{"stdout"})
	globalRouting[StreamRequest] = parseSinks(cfg.Streams.Request.Sinks, []string{"stdout", "otel", "analytics"})
	globalRouting[StreamJobs] = parseSinks(cfg.Streams.Jobs.Sinks, []string{"stdout", "otel"})
	globalRouting[StreamEventPusher] = parseSinks(cfg.Streams.EventPusher.Sinks, nil) // disabled by default

	initialized = true
}

// startDrainer creates and starts a drainer goroutine with the given config.
func startDrainer(db *sql.DB, cfg AnalyticsSinkConfig) {
	interval := 5 * time.Second
	if cfg.DrainInterval != "" {
		if d, err := time.ParseDuration(cfg.DrainInterval); err == nil {
			interval = d
		}
	}
	batch := cfg.DrainBatch
	if batch <= 0 {
		batch = 500
	}

	// Cancel any previously running drainer.
	if globalDrainerCancel != nil {
		globalDrainerCancel()
	}

	ctx, cancel := context.WithCancel(context.Background()) //nolint:gosec
	globalDrainerCancel = cancel

	drainer := NewDrainer(globalCache, db, interval, batch)
	go drainer.Run(ctx)

	slog.Info("analytics drainer activated", "interval", interval.String(), "batch", batch)
}

// ActivateDrainer starts the analytics drainer with the given database.
// Call this after the database has been opened if logging.Init was called
// without a DB (the common startup sequence).
func ActivateDrainer(db *sql.DB) {
	globalMu.Lock()
	defer globalMu.Unlock()

	if globalCache == nil {
		slog.Warn("ActivateDrainer called but analytics cache is not initialized")
		return
	}
	if globalAnalyticsCfg == nil {
		slog.Warn("ActivateDrainer called but analytics sink is not enabled")
		return
	}

	startDrainer(db, *globalAnalyticsCfg)
}

// InitDefaults initializes logging with sensible defaults (text to stdout, info level).
// Used when no config is loaded yet (e.g., during early startup).
func InitDefaults() {
	Init(Config{
		Level:  "info",
		Format: "text",
		Streams: StreamRouting{
			Runtime: StreamConfig{Sinks: []string{"stdout"}, Mode: "buffered"},
			Request: StreamConfig{Sinks: []string{"stdout"}, Mode: "sampled", SampleRate: 0.01},
			Jobs:    StreamConfig{Sinks: []string{"stdout"}, Mode: "buffered"},
		},
	})
}

// New creates a Logger for the given stream.
// The logger automatically fans out to all sinks configured for that stream.
func New(stream Stream) *Logger {
	globalMu.RLock()
	defer globalMu.RUnlock()

	if !initialized {
		// Fallback: if Init hasn't been called, use a basic stdout logger.
		h := slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo})
		return &Logger{
			Logger: slog.New(h).With("stream", string(stream)),
			stream: stream,
		}
	}

	// Check if stream mode is "off".
	streamCfg := globalStreamCfg[stream]
	if streamCfg.Mode == "off" {
		return &Logger{
			Logger: slog.New(newNoopHandler()),
			stream: stream,
		}
	}

	sinks := globalRouting[stream]
	if len(sinks) == 0 {
		// Stream is disabled — use a noop handler.
		return &Logger{
			Logger: slog.New(newNoopHandler()),
			stream: stream,
		}
	}

	// Gather the sink handlers for this stream.
	handlers := make([]guardedHandler, 0, len(sinks))
	for _, sink := range sinks {
		if sink == SinkAnalytics {
			// Analytics uses the local cache sink, not a direct DB handler.
			if globalCache != nil {
				cs := newCacheSink(globalCache, stream, streamCfg.Mode, streamCfg.SampleRate)
				handlers = append(handlers, guardedHandler{
					handler: cs,
					sink:    SinkAnalytics,
					cb:      nil, // local SQLite — always available
				})
			}
			continue
		}

		h, ok := globalSinks[sink]
		if !ok {
			continue // Sink not configured (e.g., otel endpoint not set).
		}
		var cb *CircuitBreaker
		if sink != SinkStdout {
			// Non-critical sinks get circuit breakers.
			cb = NewCircuitBreaker(5, 30*time.Second)
		}
		handlers = append(handlers, guardedHandler{
			handler: h,
			sink:    sink,
			cb:      cb,
		})
	}

	if len(handlers) == 0 {
		return &Logger{
			Logger: slog.New(newNoopHandler()),
			stream: stream,
		}
	}

	fanOut := &FanOutHandler{
		handlers: handlers,
		redactor: globalRedactor,
		level:    globalLevel,
	}

	return &Logger{
		Logger: slog.New(fanOut).With("stream", string(stream)),
		stream: stream,
	}
}

// Logger wraps slog.Logger with stream identity.
type Logger struct {
	*slog.Logger

	stream Stream
}

// Stream returns the logger's stream.
func (l *Logger) Stream() Stream { return l.stream }

// --- Context propagation ---

type ctxKey struct{}

// WithContext stores a Logger in the context.
func WithContext(ctx context.Context, l *Logger) context.Context {
	return context.WithValue(ctx, ctxKey{}, l)
}

// FromContext retrieves the Logger from context.
// Returns a default runtime logger if none is set.
func FromContext(ctx context.Context) *Logger {
	if l, ok := ctx.Value(ctxKey{}).(*Logger); ok {
		return l
	}
	return New(StreamRuntime)
}

// --- Convenience functions (use context logger) ---

// Info logs at info level using the context logger.
func Info(ctx context.Context, msg string, args ...any) {
	FromContext(ctx).InfoContext(ctx, msg, args...)
}

// Warn logs at warn level using the context logger.
func Warn(ctx context.Context, msg string, args ...any) {
	FromContext(ctx).WarnContext(ctx, msg, args...)
}

// Error logs at error level using the context logger.
func Error(ctx context.Context, msg string, args ...any) {
	FromContext(ctx).ErrorContext(ctx, msg, args...)
}

// Debug logs at debug level using the context logger.
func Debug(ctx context.Context, msg string, args ...any) {
	FromContext(ctx).DebugContext(ctx, msg, args...)
}

// --- Helpers ---

func parseLevel(s string) slog.Level {
	switch strings.ToLower(s) {
	case "debug":
		return slog.LevelDebug
	case "warn", "warning":
		return slog.LevelWarn
	case "error":
		return slog.LevelError
	default:
		return slog.LevelInfo
	}
}

func parseSinks(configured []string, defaults []string) []Sink {
	if len(configured) == 0 {
		configured = defaults
	}
	sinks := make([]Sink, 0, len(configured))
	for _, s := range configured {
		sinks = append(sinks, Sink(strings.ToLower(strings.TrimSpace(s))))
	}
	return sinks
}
