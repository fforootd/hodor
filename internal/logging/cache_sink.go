package logging

import (
	"context"
	crypto_rand "crypto/rand"
	"encoding/json"
	"log/slog"
	"math/big"
)

// cacheSink is an slog.Handler that writes records to the local SQLite cache.
// It supports three modes:
//   - "buffered": every record is written to the cache
//   - "sampled": records are written with probability sampleRate
//   - "off": no records are written
type cacheSink struct {
	cache      *Cache
	stream     Stream
	mode       string  // "buffered" | "sampled" | "off"
	sampleRate float64 // for "sampled" mode (e.g., 0.01 = 1%)
	level      slog.Level
}

// newCacheSink creates a cache sink for the given stream.
func newCacheSink(cache *Cache, stream Stream, mode string, sampleRate float64) *cacheSink {
	return &cacheSink{
		cache:      cache,
		stream:     stream,
		mode:       mode,
		sampleRate: sampleRate,
		level:      slog.LevelInfo,
	}
}

func (h *cacheSink) Enabled(_ context.Context, level slog.Level) bool {
	if h.mode == "off" {
		return false
	}
	return level >= h.level
}

func (h *cacheSink) Handle(_ context.Context, r slog.Record) error {
	if h.mode == "off" {
		return nil
	}
	if h.mode == "sampled" {
		// Use crypto/rand for gosec compliance.
		n, err := crypto_rand.Int(crypto_rand.Reader, big.NewInt(10000))
		if err != nil {
			return err
		}
		if float64(n.Int64())/10000.0 > h.sampleRate {
			return nil // skip — not sampled
		}
	}

	// Extract structured attributes into a JSON payload.
	attrs := make(map[string]any, r.NumAttrs())
	r.Attrs(func(a slog.Attr) bool {
		attrs[a.Key] = a.Value.Any()
		return true
	})
	payloadBytes, _ := json.Marshal(attrs)

	// Derive event type and category from the message.
	// Only structured messages with a dotted prefix (e.g. "request.api",
	// "auth.login_completed") are stored as events. Plain log lines
	// like "Catalog ready" or "Database: sqlite://..." are skipped.
	eventType := r.Message
	category := "log"
	for i := 0; i < len(eventType); i++ {
		if eventType[i] == '.' {
			prefix := eventType[:i]
			switch prefix {
			case "request", "api":
				category = "request"
			case "auth":
				category = "auth"
			case "session":
				category = "session"
			case "token":
				category = "token"
			case "signal":
				category = "signal"
			default:
				category = "log"
			}
			break
		}
	}
	if category == "log" {
		return nil // skip unstructured log messages
	}

	// Extract known fields from attributes.
	actorID, _ := attrs["actor_id"].(string)
	requestID, _ := attrs["request_id"].(string)
	sessionID, _ := attrs["session_id"].(string)
	flowID, _ := attrs["flow_id"].(string)
	fingerprint, _ := attrs["device_fingerprint"].(string)

	return h.cache.Write(CacheRecord{
		EventType:   eventType,
		Category:    category,
		Stream:      string(h.stream),
		Level:       r.Level.String(),
		Payload:     string(payloadBytes),
		ActorID:     actorID,
		RequestID:   requestID,
		SessionID:   sessionID,
		FlowID:      flowID,
		Fingerprint: fingerprint,
		CreatedAt:   createdAtNow(),
	})
}

func (h *cacheSink) WithAttrs(attrs []slog.Attr) slog.Handler {
	return h // cache sink doesn't need attribute scoping
}

func (h *cacheSink) WithGroup(name string) slog.Handler {
	return h // cache sink doesn't need group scoping
}
