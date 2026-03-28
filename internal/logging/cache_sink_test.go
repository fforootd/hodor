package logging

import (
	"context"
	"log/slog"
	"testing"
	"time"
)

func TestCacheSink_Buffered(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	sink := newCacheSink(cache, StreamRuntime, "buffered", 0)

	// Write 100 records.
	for i := 0; i < 100; i++ {
		record := slog.NewRecord(time.Now(), slog.LevelInfo, "log.info", 0)
		record.AddAttrs(slog.String("key", "value"))
		if err := sink.Handle(context.Background(), record); err != nil {
			t.Fatalf("handle %d: %v", i, err)
		}
	}

	// All 100 should be written.
	if cache.Count() != 100 {
		t.Errorf("buffered mode: expected 100 records, got %d", cache.Count())
	}
}

func TestCacheSink_Sampled(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	// 10% sample rate — with 10K records we expect ~1000 ± 200.
	sink := newCacheSink(cache, StreamRequest, "sampled", 0.10)

	for i := 0; i < 10000; i++ {
		record := slog.NewRecord(time.Now(), slog.LevelInfo, "request.api", 0)
		if err := sink.Handle(context.Background(), record); err != nil {
			t.Fatal(err)
		}
	}

	count := cache.Count()
	// With 10% sample rate over 10K, expect ~1000. Allow wide range: 500-1500.
	if count < 500 || count > 1500 {
		t.Errorf("sampled mode: expected ~1000 records (10%% of 10K), got %d", count)
	}
	t.Logf("sampled mode: %d out of 10000 (%.1f%%)", count, float64(count)/100.0)
}

func TestCacheSink_Off(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	sink := newCacheSink(cache, StreamEventPusher, "off", 0)

	// Enabled should return false.
	if sink.Enabled(context.Background(), slog.LevelInfo) {
		t.Error("off mode: Enabled() should return false")
	}

	// Write should be a no-op.
	record := slog.NewRecord(time.Now(), slog.LevelInfo, "event.pushed", 0)
	if err := sink.Handle(context.Background(), record); err != nil {
		t.Fatal(err)
	}

	if cache.Count() != 0 {
		t.Errorf("off mode: expected 0 records, got %d", cache.Count())
	}
}
