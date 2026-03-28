package ratelimit

import (
	"context"
	"sync"
	"time"
)

// MemoryStore is an in-memory token bucket rate limiter.
// Each unique key gets its own bucket. Expired buckets are garbage collected
// by a background goroutine.
type MemoryStore struct {
	mu      sync.RWMutex
	buckets map[string]*bucket
	stopGC  chan struct{}
}

type bucket struct {
	tokens    float64
	limit     int // tokens per window
	burst     int // max tokens
	window    time.Duration
	lastCheck time.Time
	expiresAt time.Time // bucket expires if unused for 2× window
}

// NewMemoryStore creates a new in-memory rate limit store.
// It starts a background goroutine that sweeps expired buckets every gcInterval.
func NewMemoryStore(gcInterval time.Duration) *MemoryStore {
	if gcInterval == 0 {
		gcInterval = 60 * time.Second
	}
	s := &MemoryStore{
		buckets: make(map[string]*bucket),
		stopGC:  make(chan struct{}),
	}
	go s.gc(gcInterval)
	return s
}

// Allow implements Store. It uses a token bucket algorithm:
// tokens refill at (limit / window) rate, up to burst capacity.
func (s *MemoryStore) Allow(ctx context.Context, key string, limit int, burst int, window time.Duration) (Decision, error) {
	now := time.Now()

	s.mu.Lock()
	b, exists := s.buckets[key]
	if !exists || b.limit != limit || b.burst != burst {
		// Create or reset bucket when config changes.
		b = &bucket{
			tokens:    float64(burst),
			limit:     limit,
			burst:     burst,
			window:    window,
			lastCheck: now,
			expiresAt: now.Add(2 * window),
		}
		s.buckets[key] = b
	}

	// Refill tokens based on elapsed time.
	elapsed := now.Sub(b.lastCheck)
	b.lastCheck = now
	b.expiresAt = now.Add(2 * window)

	// Token refill rate: limit tokens per window.
	rate := float64(limit) / window.Seconds()
	b.tokens += rate * elapsed.Seconds()
	if b.tokens > float64(burst) {
		b.tokens = float64(burst)
	}

	decision := Decision{
		Limit: limit,
	}

	if b.tokens >= 1.0 {
		b.tokens -= 1.0
		decision.Allowed = true
		decision.Remaining = int(b.tokens)
		decision.ResetAt = now.Add(time.Duration(float64(time.Second) / rate))
	} else {
		decision.Allowed = false
		decision.Remaining = 0
		// Time until next token is available.
		waitTime := time.Duration(float64(time.Second) * (1.0 - b.tokens) / rate)
		decision.RetryAfter = waitTime
		decision.ResetAt = now.Add(waitTime)
	}

	s.mu.Unlock()
	return decision, nil
}

// gc periodically removes expired buckets to prevent memory leaks.
func (s *MemoryStore) gc(interval time.Duration) {
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			s.sweep()
		case <-s.stopGC:
			return
		}
	}
}

// sweep removes all expired buckets.
func (s *MemoryStore) sweep() {
	now := time.Now()
	s.mu.Lock()
	for key, b := range s.buckets {
		if now.After(b.expiresAt) {
			delete(s.buckets, key)
		}
	}
	s.mu.Unlock()
}

// Stop halts the background GC goroutine.
func (s *MemoryStore) Stop() {
	close(s.stopGC)
}

// Len returns the number of active buckets (for monitoring/testing).
func (s *MemoryStore) Len() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.buckets)
}
