package logging

import (
	"bytes"
	"context"
	"log/slog"
	"strings"
	"testing"
	"time"
)

// --- Redactor Tests ---

func TestRedactor_IsSensitive(t *testing.T) {
	r := NewRedactor([]string{"password", "secret", "token"}, "***")

	tests := []struct {
		key  string
		want bool
	}{
		{"password", true},
		{"Password", true},         // case insensitive
		{"PASSWORD", true},         // all caps
		{"client_secret", true},    // contains "secret"
		{"x_api_token_hash", true}, // contains "token"
		{"username", false},
		{"email", false},
		{"display_name", false},
		{"", false},
	}

	for _, tt := range tests {
		t.Run(tt.key, func(t *testing.T) {
			got := r.IsSensitive(tt.key)
			if got != tt.want {
				t.Errorf("IsSensitive(%q) = %v, want %v", tt.key, got, tt.want)
			}
		})
	}
}

func TestRedactor_RedactValue(t *testing.T) {
	r := NewRedactor([]string{"secret"}, "MASKED")

	// Non-sensitive value.
	v := r.RedactValue("username", slog.StringValue("alice"))
	if v.String() != "alice" {
		t.Errorf("expected alice, got %s", v.String())
	}

	// Sensitive value.
	v = r.RedactValue("client_secret", slog.StringValue("super-secret-123"))
	if v.String() != "MASKED" {
		t.Errorf("expected MASKED, got %s", v.String())
	}
}

func TestRedactor_RedactRecord(t *testing.T) {
	r := NewRedactor([]string{"password", "token"}, "***")

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "user login", 0)
	record.AddAttrs(
		slog.String("username", "alice"),
		slog.String("password", "hunter2"),
		slog.String("session_token", "abc123"),
		slog.Int("status", 200),
	)

	redacted := r.RedactRecord(record)

	// Check attributes.
	var found = make(map[string]string)
	redacted.Attrs(func(a slog.Attr) bool {
		found[a.Key] = a.Value.String()
		return true
	})

	if found["username"] != "alice" {
		t.Errorf("username should not be redacted, got %s", found["username"])
	}
	if found["password"] != "***" {
		t.Errorf("password should be redacted, got %s", found["password"])
	}
	if found["session_token"] != "***" {
		t.Errorf("session_token should be redacted (contains 'token'), got %s", found["session_token"])
	}
	if found["status"] != "200" {
		t.Errorf("status should not be redacted, got %s", found["status"])
	}
}

func TestRedactor_EmptyKeys(t *testing.T) {
	r := NewRedactor(nil, "")

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "test", 0)
	record.AddAttrs(slog.String("password", "secret"))

	redacted := r.RedactRecord(record)

	var found string
	redacted.Attrs(func(a slog.Attr) bool {
		if a.Key == "password" {
			found = a.Value.String()
		}
		return true
	})

	if found != "secret" {
		t.Errorf("with no keys, nothing should be redacted, got %s", found)
	}
}

func TestRedactor_GroupRecursion(t *testing.T) {
	r := NewRedactor([]string{"password"}, "***")

	inner := slog.GroupValue(
		slog.String("username", "alice"),
		slog.String("password", "hunter2"),
	)

	redacted := r.RedactValue("auth", inner)
	if redacted.Kind() != slog.KindGroup {
		t.Fatal("expected group")
	}

	attrs := redacted.Group()
	for _, a := range attrs {
		if a.Key == "password" && a.Value.String() != "***" {
			t.Errorf("nested password should be redacted, got %s", a.Value.String())
		}
		if a.Key == "username" && a.Value.String() != "alice" {
			t.Errorf("nested username should not be redacted, got %s", a.Value.String())
		}
	}
}

// --- CircuitBreaker Tests ---

func TestCircuitBreaker_Normal(t *testing.T) {
	cb := NewCircuitBreaker(3, 100*time.Millisecond)

	if !cb.Allow() {
		t.Error("should allow in closed state")
	}
	if cb.State() != "closed" {
		t.Errorf("expected closed, got %s", cb.State())
	}
}

func TestCircuitBreaker_Opens(t *testing.T) {
	cb := NewCircuitBreaker(3, 100*time.Millisecond)

	cb.RecordFailure()
	cb.RecordFailure()
	if cb.State() != "closed" {
		t.Errorf("should still be closed after 2 failures")
	}

	cb.RecordFailure() // 3rd failure = open
	if cb.State() != "open" {
		t.Errorf("expected open after 3 failures, got %s", cb.State())
	}
	if cb.Allow() {
		t.Error("should not allow when open")
	}
}

func TestCircuitBreaker_Recovery(t *testing.T) {
	cb := NewCircuitBreaker(2, 50*time.Millisecond)

	cb.RecordFailure()
	cb.RecordFailure() // open

	if cb.Allow() {
		t.Error("should not allow when open")
	}

	// Wait for cooldown.
	time.Sleep(60 * time.Millisecond)

	if !cb.Allow() {
		t.Error("should allow in half-open (probe)")
	}
	if cb.State() != "half-open" {
		t.Errorf("expected half-open, got %s", cb.State())
	}

	// Successful probe closes the breaker.
	cb.RecordSuccess()
	if cb.State() != "closed" {
		t.Errorf("expected closed after successful probe, got %s", cb.State())
	}
	if cb.Failures() != 0 {
		t.Errorf("failures should be reset, got %d", cb.Failures())
	}
}

func TestCircuitBreaker_SuccessResets(t *testing.T) {
	cb := NewCircuitBreaker(3, time.Second)

	cb.RecordFailure()
	cb.RecordFailure()
	cb.RecordSuccess() // reset
	if cb.Failures() != 0 {
		t.Errorf("failures should be reset on success, got %d", cb.Failures())
	}

	cb.RecordFailure()
	cb.RecordFailure()
	if cb.State() != "closed" {
		t.Errorf("should still be closed after 2 more failures")
	}
}

// --- FanOutHandler Tests ---

// testHandler captures log records for assertion.
type testHandler struct {
	records []slog.Record
	level   slog.Level
	err     error // if set, Handle returns this error
}

func (h *testHandler) Enabled(_ context.Context, l slog.Level) bool { return l >= h.level }
func (h *testHandler) Handle(_ context.Context, r slog.Record) error {
	h.records = append(h.records, r)
	return h.err
}
func (h *testHandler) WithAttrs(attrs []slog.Attr) slog.Handler { return h }
func (h *testHandler) WithGroup(name string) slog.Handler       { return h }

func TestFanOutHandler_FansToAll(t *testing.T) {
	h1 := &testHandler{level: slog.LevelInfo}
	h2 := &testHandler{level: slog.LevelInfo}
	r := NewRedactor(nil, "")

	fan := &FanOutHandler{
		handlers: []guardedHandler{
			{handler: h1, sink: SinkStdout},
			{handler: h2, sink: SinkOTEL},
		},
		redactor: r,
		level:    slog.LevelInfo,
	}

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "test message", 0)
	record.AddAttrs(slog.String("key", "value"))

	if err := fan.Handle(context.Background(), record); err != nil {
		t.Fatal(err)
	}

	if len(h1.records) != 1 {
		t.Errorf("h1 should have 1 record, got %d", len(h1.records))
	}
	if len(h2.records) != 1 {
		t.Errorf("h2 should have 1 record, got %d", len(h2.records))
	}
}

func TestFanOutHandler_RedactsBeforeDispatch(t *testing.T) {
	h := &testHandler{level: slog.LevelInfo}
	r := NewRedactor([]string{"password"}, "***")

	fan := &FanOutHandler{
		handlers: []guardedHandler{
			{handler: h, sink: SinkStdout},
		},
		redactor: r,
		level:    slog.LevelInfo,
	}

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "auth", 0)
	record.AddAttrs(slog.String("password", "hunter2"))

	_ = fan.Handle(context.Background(), record)

	if len(h.records) != 1 {
		t.Fatal("expected 1 record")
	}

	// Check the captured record has redacted password.
	h.records[0].Attrs(func(a slog.Attr) bool {
		if a.Key == "password" && a.Value.String() != "***" {
			t.Errorf("password should be redacted in dispatched record, got %s", a.Value.String())
		}
		return true
	})
}

func TestFanOutHandler_CircuitBreakerSkips(t *testing.T) {
	h := &testHandler{level: slog.LevelInfo}
	cb := NewCircuitBreaker(1, time.Minute) // trips after 1 failure
	cb.RecordFailure()                      // trip it

	fan := &FanOutHandler{
		handlers: []guardedHandler{
			{handler: h, sink: SinkOTEL, cb: cb},
		},
		redactor: NewRedactor(nil, ""),
		level:    slog.LevelInfo,
	}

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "test", 0)
	_ = fan.Handle(context.Background(), record)

	if len(h.records) != 0 {
		t.Errorf("handler should not receive records when circuit breaker is open, got %d", len(h.records))
	}
}

func TestFanOutHandler_ErrorTripsBreaker(t *testing.T) {
	h := &testHandler{level: slog.LevelInfo, err: errSinkFailed}
	cb := NewCircuitBreaker(2, time.Minute)

	fan := &FanOutHandler{
		handlers: []guardedHandler{
			{handler: h, sink: SinkOTEL, cb: cb},
		},
		redactor: NewRedactor(nil, ""),
		level:    slog.LevelInfo,
	}

	record := slog.NewRecord(time.Now(), slog.LevelInfo, "test", 0)
	_ = fan.Handle(context.Background(), record) // failure 1
	_ = fan.Handle(context.Background(), record) // failure 2 → open

	if cb.State() != "open" {
		t.Errorf("breaker should be open after 2 errors, got %s", cb.State())
	}
}

var errSinkFailed = &sinkError{"sink failed"}

type sinkError struct{ msg string }

func (e *sinkError) Error() string { return e.msg }

// --- Noop Handler Tests ---

func TestNoopHandler(t *testing.T) {
	h := newNoopHandler()

	if h.Enabled(context.Background(), slog.LevelDebug) {
		t.Error("noop handler should not be enabled")
	}

	if err := h.Handle(context.Background(), slog.Record{}); err != nil {
		t.Error("noop handler should not error")
	}
}

// --- Logger & Context Tests ---

func TestNew_DisabledStream(t *testing.T) {
	Init(Config{
		Level:  "info",
		Format: "text",
		Streams: StreamRouting{
			EventPusher: StreamConfig{Mode: "off"}, // disabled
		},
	})
	defer InitDefaults()

	logger := New(StreamEventPusher)
	// Should not panic; should be a noop logger.
	logger.Info("this should go nowhere")
}

func TestContextRoundtrip(t *testing.T) {
	InitDefaults()

	logger := New(StreamJobs)
	ctx := WithContext(context.Background(), logger)

	got := FromContext(ctx)
	if got.Stream() != StreamJobs {
		t.Errorf("expected jobs stream, got %s", got.Stream())
	}
}

func TestFromContext_Default(t *testing.T) {
	InitDefaults()

	got := FromContext(context.Background())
	if got.Stream() != StreamRuntime {
		t.Errorf("expected runtime stream as default, got %s", got.Stream())
	}
}

// --- Integration: stdout sink output verification ---

func TestStdoutHandler_TextFormat(t *testing.T) {
	var buf bytes.Buffer
	h := slog.NewTextHandler(&buf, &slog.HandlerOptions{Level: slog.LevelInfo})
	logger := slog.New(h)

	logger.Info("test output", "key", "value")

	out := buf.String()
	if !strings.Contains(out, "test output") {
		t.Errorf("expected 'test output' in output, got: %s", out)
	}
	if !strings.Contains(out, "key=value") {
		t.Errorf("expected 'key=value' in output, got: %s", out)
	}
}

func TestStdoutHandler_JSONFormat(t *testing.T) {
	var buf bytes.Buffer
	h := slog.NewJSONHandler(&buf, &slog.HandlerOptions{Level: slog.LevelInfo})
	logger := slog.New(h)

	logger.Info("json test", "status", 200)

	out := buf.String()
	if !strings.Contains(out, `"msg":"json test"`) {
		t.Errorf("expected json msg in output, got: %s", out)
	}
}
