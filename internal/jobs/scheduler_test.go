package jobs

import (
	"testing"
	"time"
)

func TestNextCronTime_Interval(t *testing.T) {
	now := time.Date(2026, 1, 1, 12, 0, 0, 0, time.UTC)

	tests := []struct {
		cron     string
		wantAdd  time.Duration
	}{
		{"*/1 * * * *", 1 * time.Minute},
		{"*/5 * * * *", 5 * time.Minute},
		{"*/15 * * * *", 15 * time.Minute},
	}
	for _, tc := range tests {
		got := nextCronTime(now, tc.cron)
		want := now.Add(tc.wantAdd)
		if !got.Equal(want) {
			t.Errorf("nextCronTime(%q) = %v, want %v", tc.cron, got, want)
		}
	}
}

func TestNextCronTime_SpecificMinute(t *testing.T) {
	// If now is 12:05, and cron says "0 * * * *" (minute 0), next should be 13:00.
	now := time.Date(2026, 1, 1, 12, 5, 0, 0, time.UTC)
	got := nextCronTime(now, "0 * * * *")
	want := time.Date(2026, 1, 1, 13, 0, 0, 0, time.UTC)
	if !got.Equal(want) {
		t.Errorf("nextCronTime(0 * * * *) = %v, want %v", got, want)
	}

	// If now is exactly 12:00:00, the next occurrence is 13:00:00 (must be strictly after).
	now2 := time.Date(2026, 1, 1, 12, 0, 0, 0, time.UTC)
	got2 := nextCronTime(now2, "0 * * * *")
	want2 := time.Date(2026, 1, 1, 13, 0, 0, 0, time.UTC)
	if !got2.Equal(want2) {
		t.Errorf("nextCronTime(0 * * * * @noon) = %v, want %v", got2, want2)
	}
}

func TestNextCronTime_InvalidFallback(t *testing.T) {
	now := time.Date(2026, 1, 1, 12, 0, 0, 0, time.UTC)
	// Invalid cron (only 3 fields) should fall back to +5min.
	got := nextCronTime(now, "* * *")
	want := now.Add(5 * time.Minute)
	if !got.Equal(want) {
		t.Errorf("nextCronTime(invalid) = %v, want %v", got, want)
	}
}

func TestParseTTL(t *testing.T) {
	tests := []struct {
		input string
		want  time.Duration
	}{
		{"7d", 7 * 24 * time.Hour},
		{"30d", 30 * 24 * time.Hour},
		{"365d", 365 * 24 * time.Hour},
		{"24h", 24 * time.Hour},
		{"0", 0},
		{"forever", 0},
		{"", 0},
		{"invalid", 14 * 24 * time.Hour}, // default fallback
	}
	for _, tc := range tests {
		got := ParseTTL(tc.input)
		if got != tc.want {
			t.Errorf("ParseTTL(%q) = %v, want %v", tc.input, got, tc.want)
		}
	}
}

func TestFormatDuration(t *testing.T) {
	tests := []struct {
		input time.Duration
		want  string
	}{
		{0, "forever"},
		{-1, "forever"},
		{7 * 24 * time.Hour, "7d"},
		{30 * 24 * time.Hour, "30d"},
		{2 * time.Hour, "2h0m0s"},
	}
	for _, tc := range tests {
		got := FormatDuration(tc.input)
		if got != tc.want {
			t.Errorf("FormatDuration(%v) = %q, want %q", tc.input, got, tc.want)
		}
	}
}
