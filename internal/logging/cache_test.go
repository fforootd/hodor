package logging

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
)

// testCachePath returns a unique temp file path for a test cache DB.
func testCachePath(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	return filepath.Join(dir, "test-cache.db")
}

// testRecord creates a CacheRecord with the given event type.
func testRecord(eventType string) CacheRecord {
	return CacheRecord{
		InstanceID: "default",
		EventType:  eventType,
		Category:   "log",
		Stream:     "runtime",
		Level:      "INFO",
		Payload:    `{"msg":"test"}`,
		ActorID:    "actor_1",
		RequestID:  "request_1",
		SessionID:  "session_1",
		CreatedAt:  createdAtNow(),
	}
}

func TestCache_WriteRead(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	// Write 5 records.
	for i := 0; i < 5; i++ {
		if err := cache.Write(testRecord("log.info")); err != nil {
			t.Fatalf("write %d: %v", i, err)
		}
	}

	if cache.Count() != 5 {
		t.Errorf("expected 5, got %d", cache.Count())
	}

	// Read all back.
	records, err := cache.ReadBatch(100)
	if err != nil {
		t.Fatal(err)
	}
	if len(records) != 5 {
		t.Errorf("expected 5 records, got %d", len(records))
	}

	// Verify order (IDs should be ascending).
	for i := 1; i < len(records); i++ {
		if records[i].ID <= records[i-1].ID {
			t.Errorf("records not in order: id %d <= %d", records[i].ID, records[i-1].ID)
		}
	}
}

func TestCache_ReadBatch(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	// Write 20 records.
	for i := 0; i < 20; i++ {
		if err := cache.Write(testRecord("log.info")); err != nil {
			t.Fatal(err)
		}
	}

	// Read first 5.
	records, err := cache.ReadBatch(5)
	if err != nil {
		t.Fatal(err)
	}
	if len(records) != 5 {
		t.Errorf("expected 5 records, got %d", len(records))
	}

	// Verify these are the oldest (lowest IDs).
	if records[0].ID != 1 {
		t.Errorf("expected first record ID=1, got %d", records[0].ID)
	}
}

func TestCache_Delete(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	for i := 0; i < 5; i++ {
		if err := cache.Write(testRecord("log.info")); err != nil {
			t.Fatal(err)
		}
	}

	// Read first 3 and delete them.
	records, _ := cache.ReadBatch(3)
	ids := make([]int64, len(records))
	for i, r := range records {
		ids[i] = r.ID
	}

	if err := cache.Delete(ids); err != nil {
		t.Fatal(err)
	}

	if cache.Count() != 2 {
		t.Errorf("expected 2 remaining, got %d", cache.Count())
	}
}

func TestCache_Trim(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 50)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	// Write 100 records.
	for i := 0; i < 100; i++ {
		if err := cache.Write(testRecord("log.info")); err != nil {
			t.Fatal(err)
		}
	}

	if cache.Count() != 100 {
		t.Errorf("expected 100 before trim, got %d", cache.Count())
	}

	if err := cache.Trim(); err != nil {
		t.Fatal(err)
	}

	if cache.Count() != 50 {
		t.Errorf("expected 50 after trim, got %d", cache.Count())
	}

	// Verify the remaining records are the newest (highest IDs).
	records, _ := cache.ReadBatch(50)
	if records[0].ID != 51 {
		t.Errorf("expected oldest remaining id=51, got %d", records[0].ID)
	}
}

func TestCache_RingBuffer(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 10)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	// Write 30 records, trimming after each batch of 10.
	for batch := 0; batch < 3; batch++ {
		for i := 0; i < 10; i++ {
			if err := cache.Write(testRecord("log.info")); err != nil {
				t.Fatal(err)
			}
		}
		if err := cache.Trim(); err != nil {
			t.Fatal(err)
		}
	}

	count := cache.Count()
	if count != 10 {
		t.Errorf("expected 10 after ring buffer cycles, got %d", count)
	}
}

func TestCache_Concurrent(t *testing.T) {
	cache, err := OpenCache(testCachePath(t), 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache.Close()

	var wg sync.WaitGroup
	const writers = 50
	const recordsPerWriter = 20

	// Spawn writers.
	for i := 0; i < writers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < recordsPerWriter; j++ {
				_ = cache.Write(testRecord("log.info"))
			}
		}()
	}

	// Spawn a reader.
	wg.Add(1)
	go func() {
		defer wg.Done()
		for i := 0; i < 10; i++ {
			_, _ = cache.ReadBatch(100)
		}
	}()

	wg.Wait()

	count := cache.Count()
	expected := writers * recordsPerWriter
	if count != expected {
		t.Errorf("expected %d records after concurrent writes, got %d", expected, count)
	}
}

func TestOpenCacheCreatesParentDir(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nested", "logs", "cache.db")

	cache, err := OpenCache(path, 0)
	if err != nil {
		t.Fatalf("OpenCache: %v", err)
	}
	defer cache.Close()

	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatalf("cache database not created at %s", path)
	}
}

func TestCache_Persistence(t *testing.T) {
	path := testCachePath(t)

	// Open, write, close.
	cache1, err := OpenCache(path, 0)
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 5; i++ {
		if err := cache1.Write(testRecord("log.info")); err != nil {
			t.Fatal(err)
		}
	}
	cache1.Close()

	// Verify file exists.
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("cache file should exist on disk")
	}

	// Reopen and verify data persisted.
	cache2, err := OpenCache(path, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer cache2.Close()

	if cache2.Count() != 5 {
		t.Errorf("expected 5 records after reopen, got %d", cache2.Count())
	}
}
