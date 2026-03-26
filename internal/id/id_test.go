package id

import (
	"fmt"
	"sync"
	"testing"
)

func TestNew(t *testing.T) {
	id, err := New()
	if err != nil {
		t.Fatalf("New() error: %v", err)
	}
	if id <= 0 {
		t.Errorf("New() = %d, want > 0", id)
	}
}

func TestUniqueness(t *testing.T) {
	seen := make(map[int64]bool, 1000)
	for i := 0; i < 1000; i++ {
		id, err := New()
		if err != nil {
			t.Fatalf("New() error on iteration %d: %v", i, err)
		}
		if seen[id] {
			t.Fatalf("duplicate ID %d on iteration %d", id, i)
		}
		seen[id] = true
	}
}

func TestConcurrent(t *testing.T) {
	const goroutines = 10
	const idsPerGoroutine = 100

	var mu sync.Mutex
	seen := make(map[int64]bool, goroutines*idsPerGoroutine)
	errs := make(chan error, goroutines)

	var wg sync.WaitGroup
	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := 0; i < idsPerGoroutine; i++ {
				id, err := New()
				if err != nil {
					errs <- err
					return
				}
				mu.Lock()
				if seen[id] {
					mu.Unlock()
					errs <- fmt.Errorf("duplicate ID %d", id)
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

func FuzzNew(f *testing.F) {
	f.Add(uint16(1))
	f.Fuzz(func(t *testing.T, _ uint16) {
		id, err := New()
		if err != nil {
			t.Skip("sonyflake timing constraint")
		}
		if id <= 0 {
			t.Errorf("got non-positive ID: %d", id)
		}
	})
}
