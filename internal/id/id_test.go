package id

import (
	"strings"
	"sync"
	"testing"

	"github.com/google/uuid"
)

func TestNew_ValidUUIDv7(t *testing.T) {
	raw := New()
	u, err := uuid.Parse(raw)
	if err != nil {
		t.Fatalf("New() returned invalid UUID %q: %v", raw, err)
	}
	if u.Version() != 7 {
		t.Errorf("New() version = %d, want 7", u.Version())
	}
	if len(raw) != 36 {
		t.Errorf("New() length = %d, want 36", len(raw))
	}
}

func TestUniqueness(t *testing.T) {
	const n = 10_000
	seen := make(map[string]bool, n)
	for i := 0; i < n; i++ {
		id := New()
		if seen[id] {
			t.Fatalf("duplicate ID %q on iteration %d", id, i)
		}
		seen[id] = true
	}
}

func TestMonotonicity(t *testing.T) {
	// UUIDv7 strings sort lexicographically in time order.
	prev := New()
	for i := 0; i < 1000; i++ {
		curr := New()
		if curr <= prev {
			t.Fatalf("non-monotonic: %q <= %q at iteration %d", curr, prev, i)
		}
		prev = curr
	}
}

func TestConcurrent(t *testing.T) {
	const goroutines = 10
	const idsPerGoroutine = 1000

	var mu sync.Mutex
	seen := make(map[string]bool, goroutines*idsPerGoroutine)
	errs := make(chan error, goroutines)

	var wg sync.WaitGroup
	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := 0; i < idsPerGoroutine; i++ {
				id := New()
				mu.Lock()
				if seen[id] {
					mu.Unlock()
					errs <- &duplicateError{id}
					return
				}
				seen[id] = true
				mu.Unlock()
			}
		}()
	}

	wg.Wait()
	close(errs)

	for err := range errs {
		t.Fatal(err)
	}
}

type duplicateError struct{ id string }

func (e *duplicateError) Error() string { return "duplicate ID: " + e.id }

func TestNewFlow(t *testing.T) {
	id := NewFlow()
	if !strings.HasPrefix(id, "flow_") {
		t.Errorf("NewFlow() = %q, want prefix 'flow_'", id)
	}
	// Parse the UUID portion after the prefix.
	uuidPart := strings.TrimPrefix(id, "flow_")
	if _, err := uuid.Parse(uuidPart); err != nil {
		t.Errorf("NewFlow() UUID portion %q is invalid: %v", uuidPart, err)
	}
}

func TestNewLoginSession(t *testing.T) {
	id := NewLoginSession()
	if !strings.HasPrefix(id, "ls_") {
		t.Errorf("NewLoginSession() = %q, want prefix 'ls_'", id)
	}
	uuidPart := strings.TrimPrefix(id, "ls_")
	if _, err := uuid.Parse(uuidPart); err != nil {
		t.Errorf("NewLoginSession() UUID portion %q is invalid: %v", uuidPart, err)
	}
}

func TestNewSSEConsumer(t *testing.T) {
	id := NewSSEConsumer()
	if !strings.HasPrefix(id, "sse-") {
		t.Errorf("NewSSEConsumer() = %q, want prefix 'sse-'", id)
	}
	uuidPart := strings.TrimPrefix(id, "sse-")
	if _, err := uuid.Parse(uuidPart); err != nil {
		t.Errorf("NewSSEConsumer() UUID portion %q is invalid: %v", uuidPart, err)
	}
}

func FuzzNew(f *testing.F) {
	f.Add(byte(0))
	f.Fuzz(func(t *testing.T, _ byte) {
		raw := New()
		if raw == "" {
			t.Fatal("New() returned empty string")
		}
		u, err := uuid.Parse(raw)
		if err != nil {
			t.Fatalf("New() returned invalid UUID %q: %v", raw, err)
		}
		if u.Version() != 7 {
			t.Errorf("New() version = %d, want 7", u.Version())
		}
	})
}

func BenchmarkNew(b *testing.B) {
	for i := 0; i < b.N; i++ {
		_ = New()
	}
}
