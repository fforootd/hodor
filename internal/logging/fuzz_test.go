package logging

import (
	"context"
	"fmt"
	"log/slog"
	"strings"
	"testing"
	"time"
)

// --- IP Redaction Tests ---

func TestIPRedact_Keep(t *testing.T) {
	r := NewRedactorWithIP(nil, "", "keep")
	if got := r.RedactIP("192.168.1.100"); got != "192.168.1.100" {
		t.Errorf("keep mode: expected unchanged IP, got %q", got)
	}
}

func TestIPRedact_Redact(t *testing.T) {
	r := NewRedactorWithIP(nil, "***REDACTED***", "redact")
	if got := r.RedactIP("192.168.1.100"); got != "***REDACTED***" {
		t.Errorf("redact mode: expected mask, got %q", got)
	}
}

func TestIPRedact_Hash(t *testing.T) {
	r := NewRedactorWithIP(nil, "", "hash")
	hash1 := r.RedactIP("192.168.1.100")
	hash2 := r.RedactIP("192.168.1.100")
	hash3 := r.RedactIP("10.0.0.1")

	// Same IP → same hash.
	if hash1 != hash2 {
		t.Errorf("hash mode: same IP should produce same hash, got %q vs %q", hash1, hash2)
	}
	// Different IP → different hash.
	if hash1 == hash3 {
		t.Errorf("hash mode: different IPs should produce different hashes")
	}
	// Hash should be 16 hex chars.
	if len(hash1) != 16 {
		t.Errorf("hash mode: expected 16 char hash, got %d chars: %q", len(hash1), hash1)
	}
	// Should not contain original IP.
	if strings.Contains(hash1, "192.168") {
		t.Errorf("hash mode: hash should not contain original IP")
	}

	// IPv6.
	hashV6 := r.RedactIP("2001:db8::1")
	if len(hashV6) != 16 {
		t.Errorf("hash mode (IPv6): expected 16 char hash, got %d: %q", len(hashV6), hashV6)
	}
}

func TestIPRedact_Mask(t *testing.T) {
	r := NewRedactorWithIP(nil, "", "mask")

	// IPv4.
	if got := r.RedactIP("192.168.1.100"); got != "192.168.x.x" {
		t.Errorf("mask mode (IPv4): expected 192.168.x.x, got %q", got)
	}

	// Invalid IP.
	if got := r.RedactIP("not-an-ip"); got != "x.x.x.x" {
		t.Errorf("mask mode (invalid): expected x.x.x.x, got %q", got)
	}
}

func TestIPRedact_IntegrationWithRedactValue(t *testing.T) {
	r := NewRedactorWithIP([]string{"password"}, "", "hash")

	// IP field should be hashed.
	ipVal := r.RedactValue("client_ip", slog.StringValue("10.0.0.1"))
	if ipVal.String() == "10.0.0.1" {
		t.Error("IP field should have been hashed")
	}

	// Sensitive field should still be redacted.
	pwVal := r.RedactValue("password", slog.StringValue("hunter2"))
	if pwVal.String() != "***REDACTED***" {
		t.Errorf("sensitive field should be redacted, got %q", pwVal.String())
	}

	// Non-sensitive, non-IP field should be unchanged.
	normalVal := r.RedactValue("username", slog.StringValue("alice"))
	if normalVal.String() != "alice" {
		t.Errorf("normal field should be unchanged, got %q", normalVal.String())
	}
}

func TestIPRedact_RedactRecord(t *testing.T) {
	r := NewRedactorWithIP(nil, "", "mask")

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "test", 0)
	record.AddAttrs(
		slog.String("remote_addr", "203.0.113.42"),
		slog.String("user", "alice"),
	)

	redacted := r.RedactRecord(record)

	redacted.Attrs(func(a slog.Attr) bool {
		if a.Key == "remote_addr" && a.Value.String() == "203.0.113.42" {
			t.Error("remote_addr should have been masked")
		}
		if a.Key == "user" && a.Value.String() != "alice" {
			t.Errorf("user should be unchanged, got %q", a.Value.String())
		}
		return true
	})
}

// FuzzRedactor_IsSensitive fuzzes the Redactor with random key inputs
// to ensure no panics or unexpected behavior.
func FuzzRedactor_IsSensitive(f *testing.F) {
	// Seed corpus.
	f.Add("password")
	f.Add("secret")
	f.Add("token")
	f.Add("username")
	f.Add("")
	f.Add("client_secret_hash")
	f.Add("OIDC_TOKEN")
	f.Add("x-api-key")
	f.Add(string(make([]byte, 1024))) // large input

	r := NewRedactor([]string{"password", "secret", "token", "key"}, "***")

	f.Fuzz(func(t *testing.T, key string) {
		// Must not panic.
		_ = r.IsSensitive(key)
	})
}

// FuzzRedactor_RedactRecord fuzzes the full redaction pipeline with
// random keys and values to detect panics, memory issues, or data races.
func FuzzRedactor_RedactRecord(f *testing.F) {
	f.Add("username", "alice")
	f.Add("password", "hunter2")
	f.Add("", "")
	f.Add("client_secret", "abc123")
	f.Add("nested.password.field", "secret-value")

	r := NewRedactor([]string{"password", "secret", "token"}, "***REDACTED***")

	f.Fuzz(func(t *testing.T, key, value string) {
		record := slog.NewRecord(time.Now(), slog.LevelInfo, "fuzz test", 0)
		record.AddAttrs(slog.String(key, value))

		// Must not panic.
		redacted := r.RedactRecord(record)

		// If key is sensitive, value must be masked.
		if r.IsSensitive(key) {
			redacted.Attrs(func(a slog.Attr) bool {
				if a.Key == key && a.Value.String() != "***REDACTED***" {
					t.Errorf("sensitive key %q was not redacted: got %q", key, a.Value.String())
				}
				return true
			})
		}
	})
}

// FuzzRedactor_RedactValue_Group fuzzes group value redaction.
func FuzzRedactor_RedactValue_Group(f *testing.F) {
	f.Add("auth", "username", "alice", "password", "secret123")

	r := NewRedactor([]string{"password", "secret"}, "***")

	f.Fuzz(func(t *testing.T, groupName, k1, v1, k2, v2 string) {
		inner := slog.GroupValue(
			slog.String(k1, v1),
			slog.String(k2, v2),
		)

		// Must not panic.
		result := r.RedactValue(groupName, inner)

		if result.Kind() != slog.KindGroup {
			t.Error("expected group kind")
		}
	})
}

// FuzzCircuitBreaker exercises the circuit breaker with random
// sequences of Allow/RecordFailure/RecordSuccess to detect races.
func FuzzCircuitBreaker(f *testing.F) {
	f.Add(uint8(0), uint8(1), uint8(0), uint8(2))

	f.Fuzz(func(t *testing.T, a, b, c, d uint8) {
		cb := NewCircuitBreaker(3, 10*time.Millisecond)

		ops := []uint8{a, b, c, d}
		for _, op := range ops {
			switch op % 3 {
			case 0:
				_ = cb.Allow()
			case 1:
				cb.RecordFailure()
			case 2:
				cb.RecordSuccess()
			}
		}

		// State must be one of the valid values.
		state := cb.State()
		if state != "closed" && state != "open" && state != "half-open" {
			t.Errorf("invalid state: %s", state)
		}
	})
}

// FuzzFanOutHandler exercises the full fan-out pipeline with random
// log messages, keys, and values.
func FuzzFanOutHandler(f *testing.F) {
	f.Add("test message", "key1", "value1", "password", "secret123")

	f.Fuzz(func(t *testing.T, msg, k1, v1, k2, v2 string) {
		h := &testHandler{level: slog.LevelDebug}
		r := NewRedactor([]string{"password", "secret"}, "***")

		fan := &FanOutHandler{
			handlers: []guardedHandler{
				{handler: h, sink: SinkStdout},
			},
			redactor: r,
			level:    slog.LevelDebug,
		}

		record := slog.NewRecord(time.Now(), slog.LevelInfo, msg, 0)
		record.AddAttrs(
			slog.String(k1, v1),
			slog.String(k2, v2),
		)

		// Must not panic.
		if err := fan.Handle(context.Background(), record); err != nil {
			t.Fatal(err)
		}

		if len(h.records) != 1 {
			t.Fatal("expected exactly 1 record")
		}
	})
}

// --- Benchmarks ---

// BenchmarkFanOutHandler_Handle measures the hot path: a single log record
// dispatched to 3 sinks (stdout, otel, analytics) with redaction.
func BenchmarkFanOutHandler_Handle(b *testing.B) {
	h1 := &discardHandler{}
	h2 := &discardHandler{}
	h3 := &discardHandler{}
	r := NewRedactor([]string{"password", "secret", "token"}, "***")

	fan := &FanOutHandler{
		handlers: []guardedHandler{
			{handler: h1, sink: SinkStdout},
			{handler: h2, sink: SinkOTEL, cb: NewCircuitBreaker(5, time.Second)},
			{handler: h3, sink: SinkAnalytics, cb: NewCircuitBreaker(5, time.Second)},
		},
		redactor: r,
		level:    slog.LevelInfo,
	}

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "request.api", 0)
	record.AddAttrs(
		slog.String("method", "GET"),
		slog.String("path", "/v1/users"),
		slog.Int("status", 200),
		slog.Int64("duration_ms", 42),
		slog.String("actor_id", "user_abc123"),
		slog.String("trace_id", "deadbeef12345678deadbeef12345678"),
	)

	ctx := context.Background()

	b.ReportAllocs()
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		_ = fan.Handle(ctx, record)
	}
}

// BenchmarkFanOutHandler_Handle_WithRedaction measures the hot path
// when redaction actually fires on sensitive fields.
func BenchmarkFanOutHandler_Handle_WithRedaction(b *testing.B) {
	h := &discardHandler{}
	r := NewRedactor([]string{"password", "secret", "token"}, "***REDACTED***")

	fan := &FanOutHandler{
		handlers: []guardedHandler{
			{handler: h, sink: SinkStdout},
		},
		redactor: r,
		level:    slog.LevelInfo,
	}

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "auth.login", 0)
	record.AddAttrs(
		slog.String("username", "alice"),
		slog.String("password", "hunter2"),
		slog.String("client_secret", "super-secret"),
		slog.String("api_token", "tok_12345"),
		slog.String("method", "POST"),
	)

	ctx := context.Background()

	b.ReportAllocs()
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		_ = fan.Handle(ctx, record)
	}
}

// BenchmarkRedactor_RedactRecord benchmarks redaction in isolation.
func BenchmarkRedactor_RedactRecord(b *testing.B) {
	r := NewRedactor([]string{"password", "secret", "token", "key", "private"}, "***")

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "test", 0)
	for i := 0; i < 20; i++ {
		record.AddAttrs(slog.String(fmt.Sprintf("field_%d", i), fmt.Sprintf("value_%d", i)))
	}
	record.AddAttrs(slog.String("password", "hunter2"))
	record.AddAttrs(slog.String("api_token", "tok"))

	b.ReportAllocs()
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		_ = r.RedactRecord(record)
	}
}

// BenchmarkCircuitBreaker_Allow benchmarks the CB check (hot path).
func BenchmarkCircuitBreaker_Allow(b *testing.B) {
	cb := NewCircuitBreaker(5, time.Second)

	b.ReportAllocs()
	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		_ = cb.Allow()
	}
}

// --- Test helpers ---

// discardHandler accepts all records and discards them.
type discardHandler struct{}

func (h *discardHandler) Enabled(_ context.Context, _ slog.Level) bool  { return true }
func (h *discardHandler) Handle(_ context.Context, _ slog.Record) error { return nil }
func (h *discardHandler) WithAttrs(_ []slog.Attr) slog.Handler          { return h }
func (h *discardHandler) WithGroup(_ string) slog.Handler               { return h }
