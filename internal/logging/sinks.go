package logging

import (
	"context"
	"log/slog"
	"os"
	"strings"

	"github.com/zitadel/zitadel/internal/telemetry"
)

// --- Stdout Sink ---

// newStdoutHandler creates a slog handler that writes to os.Stdout.
// Format is either "text" (human-readable) or "json" (structured).
func newStdoutHandler(format string, level slog.Level) slog.Handler {
	opts := &slog.HandlerOptions{
		Level:     level,
		AddSource: false,
	}
	switch strings.ToLower(format) {
	case "json":
		return slog.NewJSONHandler(os.Stdout, opts)
	default:
		return slog.NewTextHandler(os.Stdout, opts)
	}
}

// --- OTEL Sink ---

// otelHandler adapts slog records to the OTEL log format.
// For now, this is a structured JSON logger that writes to stdout with
// an "otel" marker. When a real OTLP log exporter is wired in,
// this handler will be replaced with the official SDK exporter.
//
// TODO: Replace with go.opentelemetry.io/otel/sdk/log when it stabilizes.
type otelHandler struct {
	endpoint string
	protocol string
	inner    slog.Handler
}

func newOTELHandler(endpoint, protocol string, level slog.Level) slog.Handler {
	// For the POC, we write OTEL-formatted JSON to stdout.
	// In production, this would use the OTLP log exporter.
	opts := &slog.HandlerOptions{
		Level: level,
	}
	inner := slog.NewJSONHandler(os.Stdout, opts)
	return &otelHandler{
		endpoint: endpoint,
		protocol: protocol,
		inner:    inner,
	}
}

func (h *otelHandler) Enabled(ctx context.Context, level slog.Level) bool {
	return h.inner.Enabled(ctx, level)
}

func (h *otelHandler) Handle(ctx context.Context, r slog.Record) error {
	// Enrich with OTEL resource attributes.
	r.AddAttrs(
		slog.String("otel.endpoint", h.endpoint),
		slog.String("otel.protocol", h.protocol),
	)

	// Extract request context if available (maps to OTEL trace_id for export).
	if requestID := telemetry.RequestIDFromContext(ctx); requestID != "" {
		r.AddAttrs(slog.String("trace_id", requestID)) // OTEL export uses trace_id naming
	}

	return h.inner.Handle(ctx, r)
}

func (h *otelHandler) WithAttrs(attrs []slog.Attr) slog.Handler {
	return &otelHandler{
		endpoint: h.endpoint,
		protocol: h.protocol,
		inner:    h.inner.WithAttrs(attrs),
	}
}

func (h *otelHandler) WithGroup(name string) slog.Handler {
	return &otelHandler{
		endpoint: h.endpoint,
		protocol: h.protocol,
		inner:    h.inner.WithGroup(name),
	}
}

// --- Analytics Sink ---
// The old direct-write analyticsHandler has been replaced by the cache-based
// architecture. See cache_sink.go (writes to local SQLite) and drainer.go
// (batch-flushes from cache to analytics backend).
