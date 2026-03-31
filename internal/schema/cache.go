package schema

import (
	"sync"
	"time"
)

type cacheEntry struct {
	record    *SchemaRecord
	expiresAt time.Time
}

// SchemaCache is a concurrency-safe in-memory cache for schema records.
// It caches by exact ID and by default-for-type resolution.
type SchemaCache struct {
	mu        sync.RWMutex
	byID      map[string]cacheEntry
	byDefault map[string]cacheEntry
	ttl       time.Duration
}

// NewSchemaCache creates a cache with the given TTL safety net.
// Primary invalidation is explicit via Invalidate/InvalidateType/InvalidateAll.
func NewSchemaCache(ttl time.Duration) *SchemaCache {
	return &SchemaCache{
		byID:      make(map[string]cacheEntry),
		byDefault: make(map[string]cacheEntry),
		ttl:       ttl,
	}
}

func (c *SchemaCache) GetByID(id string) (*SchemaRecord, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	entry, ok := c.byID[id]
	if !ok || time.Now().After(entry.expiresAt) {
		return nil, false
	}
	return entry.record, true
}

func (c *SchemaCache) GetDefault(schemaType string) (*SchemaRecord, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	entry, ok := c.byDefault[schemaType]
	if !ok || time.Now().After(entry.expiresAt) {
		return nil, false
	}
	return entry.record, true
}

func (c *SchemaCache) PutByID(id string, rec *SchemaRecord) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.byID[id] = cacheEntry{record: rec, expiresAt: time.Now().Add(c.ttl)}
}

func (c *SchemaCache) PutDefault(schemaType string, rec *SchemaRecord) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.byDefault[schemaType] = cacheEntry{record: rec, expiresAt: time.Now().Add(c.ttl)}
}

// Invalidate removes a specific schema by ID from the cache.
func (c *SchemaCache) Invalidate(id string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	delete(c.byID, id)
}

// InvalidateType removes the default-for-type cache entry.
func (c *SchemaCache) InvalidateType(schemaType string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	delete(c.byDefault, schemaType)
}

// InvalidateAll clears the entire cache.
func (c *SchemaCache) InvalidateAll() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.byID = make(map[string]cacheEntry)
	c.byDefault = make(map[string]cacheEntry)
}
