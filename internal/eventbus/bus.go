// Package eventbus provides the in-memory notification channel for async
// event processing. The events table IS the queue (durable). This bus
// just wakes up consumers so they poll the DB from their cursor.
package eventbus

import (
	"context"
	"sync"
)

// Bus is the event notification bus.
// It does NOT carry event data — it signals consumers that new events
// exist in the database. Consumers then read from their cursor.
type Bus struct {
	mu        sync.RWMutex
	consumers map[string]*Consumer
	notify    chan struct{}
}

// Consumer is an async event processor that maintains a cursor
// into the events table.
type Consumer struct {
	Name   string
	notify chan struct{}
}

// New creates a new event bus.
func New() *Bus {
	return &Bus{
		consumers: make(map[string]*Consumer),
		notify:    make(chan struct{}, 1),
	}
}

// Register adds a named consumer to the bus.
// Each consumer gets its own notification channel.
func (b *Bus) Register(name string) *Consumer {
	b.mu.Lock()
	defer b.mu.Unlock()

	c := &Consumer{
		Name:   name,
		notify: make(chan struct{}, 1),
	}
	b.consumers[name] = c
	return c
}

// Signal notifies all consumers that new events are available.
// Non-blocking: if a consumer's channel is full (already signaled), skip it.
// Called after event COMMIT.
func (b *Bus) Signal() {
	b.mu.RLock()
	defer b.mu.RUnlock()

	for _, c := range b.consumers {
		select {
		case c.notify <- struct{}{}:
		default:
			// Consumer already has a pending signal — skip.
		}
	}
}

// Wait blocks until a signal is received or the context is cancelled.
// Returns true if signaled, false if context cancelled.
func (c *Consumer) Wait(ctx context.Context) bool {
	select {
	case <-c.notify:
		return true
	case <-ctx.Done():
		return false
	}
}

// Chan returns the underlying notification channel for use in select statements.
func (c *Consumer) Chan() <-chan struct{} {
	return c.notify
}
