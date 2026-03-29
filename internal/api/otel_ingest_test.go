package api

import (
	"testing"
	"time"
)

func TestOtelRateLimiter_Allow(t *testing.T) {
	rl := &otelRateLimiter{
		buckets: make(map[string]*tokenBucket),
	}

	ip := "192.168.1.1"

	// First request should be allowed.
	if !rl.allow(ip) {
		t.Error("First request should be allowed")
	}

	// Send up to the limit.
	// We already consumed 1 token, so we have otelRateLimitPerMin-1 left.
	for i := 0; i < otelRateLimitPerMin-1; i++ {
		rl.allow(ip)
	}

	// Next request should be blocked.
	if rl.allow(ip) {
		t.Error("Request past limit should be blocked")
	}
}

func TestOtelRateLimiter_DifferentIPs(t *testing.T) {
	rl := &otelRateLimiter{
		buckets: make(map[string]*tokenBucket),
	}

	// Different IPs should have independent buckets.
	if !rl.allow("10.0.0.1") {
		t.Error("10.0.0.1 should be allowed")
	}
	if !rl.allow("10.0.0.2") {
		t.Error("10.0.0.2 should be allowed")
	}

	if len(rl.buckets) != 2 {
		t.Errorf("Expected 2 buckets, got %d", len(rl.buckets))
	}
}

func TestOtelRateLimiter_Cleanup(t *testing.T) {
	rl := &otelRateLimiter{
		buckets: make(map[string]*tokenBucket),
	}

	// Add a stale bucket.
	rl.buckets["stale-ip"] = &tokenBucket{
		tokens:    50,
		lastReset: time.Now().Add(-10 * time.Minute), // 10 min ago
	}

	// Add a fresh bucket.
	rl.buckets["fresh-ip"] = &tokenBucket{
		tokens:    50,
		lastReset: time.Now(),
	}

	rl.cleanup()

	if _, ok := rl.buckets["stale-ip"]; ok {
		t.Error("Stale bucket should have been cleaned up")
	}
	if _, ok := rl.buckets["fresh-ip"]; !ok {
		t.Error("Fresh bucket should still exist")
	}
}

func TestShouldSampleSpan(t *testing.T) {
	tests := []struct {
		name     string
		span     OTelSpan
		flowID   string
		expected bool
	}{
		{
			name:     "flow-linked span",
			span:     OTelSpan{Name: "someSpan"},
			flowID:   "flow_123",
			expected: true,
		},
		{
			name:     "error span",
			span:     OTelSpan{Name: "someSpan", Status: &SpanStatus{Code: 2}},
			flowID:   "",
			expected: true,
		},
		{
			name: "slow span (>3s)",
			span: OTelSpan{
				Name:      "someSpan",
				StartTime: 1000000000,
				EndTime:   5000000000, // 4 second span
			},
			flowID:   "",
			expected: true,
		},
		{
			name:     "documentLoad",
			span:     OTelSpan{Name: "documentLoad"},
			flowID:   "",
			expected: true,
		},
		{
			name:     "documentFetch",
			span:     OTelSpan{Name: "documentFetch"},
			flowID:   "",
			expected: true,
		},
		{
			name:     "resourceFetch",
			span:     OTelSpan{Name: "resourceFetch"},
			flowID:   "",
			expected: true,
		},
		{
			name: "routine short span (dropped)",
			span: OTelSpan{
				Name:      "HTTP GET /v1/schemas",
				StartTime: 1000000000,
				EndTime:   1050000000, // 50ms
			},
			flowID:   "",
			expected: false,
		},
		{
			name:     "unlinked non-error span (dropped)",
			span:     OTelSpan{Name: "randomSpan"},
			flowID:   "",
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := shouldSampleSpan(tt.span, tt.flowID)
			if got != tt.expected {
				t.Errorf("shouldSampleSpan(%q, flow=%q) = %v, want %v", tt.span.Name, tt.flowID, got, tt.expected)
			}
		})
	}
}
