package eventbus

import (
	"context"
	"testing"
	"time"
)

func TestSignalAndWait(t *testing.T) {
	bus := New()
	consumer := bus.Register("test")

	// Signal should wake up consumer.
	bus.Signal()

	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	if !consumer.Wait(ctx) {
		t.Fatal("consumer did not receive signal")
	}
}

func TestMultipleConsumers(t *testing.T) {
	bus := New()
	c1 := bus.Register("lake")
	c2 := bus.Register("notify")
	c3 := bus.Register("threat")

	bus.Signal()

	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	for _, c := range []*Consumer{c1, c2, c3} {
		if !c.Wait(ctx) {
			t.Fatalf("consumer %q did not receive signal", c.Name)
		}
	}
}

func TestSignalNonBlocking(t *testing.T) {
	bus := New()
	consumer := bus.Register("test")

	// Signal twice — second should be silently dropped, not block.
	bus.Signal()
	bus.Signal()

	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	if !consumer.Wait(ctx) {
		t.Fatal("consumer did not receive signal")
	}

	// No second signal should be pending (it was coalesced).
	ctx2, cancel2 := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel2()

	if consumer.Wait(ctx2) {
		t.Fatal("expected no second signal (should be coalesced)")
	}
}

func TestWaitCancelled(t *testing.T) {
	bus := New()
	consumer := bus.Register("test")

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately.

	if consumer.Wait(ctx) {
		t.Fatal("expected false from cancelled context")
	}
}
