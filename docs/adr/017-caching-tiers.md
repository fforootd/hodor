# ADR-017: Tiered Caching Architecture

**Status**: Proposed  
**Date**: 2026-03-28  
**Builds on**: ADR-010 (Three-Tier Data Architecture)  
**Related**: [Glossary](../GLOSSARY.md), [Event Pipeline](../architecture/event-pipeline.md)

## Context

Zitadel is a single Go binary (~30MB) that must cache effectively without requiring external infrastructure by default. As deployments scale from homelab (SQLite) to multi-machine production (Postgres + Redis), caching needs vary in scope, durability, and consistency requirements.

This ADR defines a four-tier caching architecture where each tier serves a distinct purpose, and operators can opt-in to higher tiers as their scale demands.

## Four Cache Tiers

```
L1: In-Database      — always available, zero config
L2: In-Process        — per-instance, zero network overhead
L3: Shared (opt-in)   — cross-instance coordination
L4: HTTP / CDN        — client-side, automatic
```

### L1: In-Database

Available in every deployment. Uses database features to accelerate repeated reads.

| Technique | What | When |
|---|---|---|
| **Covering indexes** | Domain→org lookup, event_type+category | Already exists |
| **Materialized views** | Entity counts per schema_type, login trend aggregates | Refresh on mutation event |

### L2: In-Process (Zero Network)

Every Zitadel instance maintains a local cache — no network calls, no external deps.

#### SQLite Cache (`zitadel-cache.db`)

The existing log buffer expands into a general-purpose local cache:

```sql
-- Existing: log buffer (ring buffer for analytics)
CREATE TABLE IF NOT EXISTS log_buffer (...);

-- New: generic key-value cache with TTL
CREATE TABLE IF NOT EXISTS kv_cache (
    key        TEXT PRIMARY KEY,
    namespace  TEXT NOT NULL DEFAULT '',
    value      TEXT NOT NULL,
    ttl_secs   INTEGER NOT NULL DEFAULT 60,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- New: analytics query result cache (LRU, 1000 max)
CREATE TABLE IF NOT EXISTS query_cache (
    query_hash TEXT PRIMARY KEY,
    result     TEXT NOT NULL,
    row_count  INTEGER NOT NULL,
    ttl_secs   INTEGER NOT NULL DEFAULT 30,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**What lives in L2:**

| Data | Store | Eviction | Persistence |
|---|---|---|---|
| Log buffer | SQLite `log_buffer` | Ring buffer (50K) | ✅ Persistent |
| Schema cache | SQLite `kv_cache` | TTL 60s + event | ✅ Persistent |
| Settings cache | SQLite `kv_cache` | TTL 30s + event | ✅ Persistent |
| OIDC Discovery | SQLite `kv_cache` | TTL 5min | ✅ Persistent |
| JWKS | SQLite `kv_cache` | TTL 5min + event | ✅ Persistent |
| Domain → Org | SQLite `kv_cache` | TTL 5min + event | ✅ Persistent |
| Query results | SQLite `query_cache` | TTL 30s (flat), LRU 1000 | ✅ Persistent |
| Rate limiter | Go `sync.Map` | TTL (bucket window), LRU | ❌ Ephemeral |
| Compiled expr | Go `sync.Map` | LRU (500) | ❌ Ephemeral |
| FGA batch | Go per-request | Per-request lifecycle | ❌ Ephemeral |

#### Analytics Query Caching

The Explore SQL editor and dashboard use expensive queries (`COUNT(*)`, `GROUP BY`). Query results are cached with a flat 30-second TTL:

1. Hash the normalized SQL → check `query_cache`
2. If hit and not expired → return cached result
3. If miss → execute query → store result → return

Query caching is safe because analytics data is append-only and slight staleness is expected.

#### Invalidation

SQLite cache entries are invalidated by domain events via the EventBus:
- `schema.updated` → invalidate schema cache
- `settings.updated` → invalidate settings cache
- `org.domain.*` → invalidate domain→org cache
- Key rotation events → invalidate JWKS cache

### L3: Shared (Escape Hatch, Opt-In)

Redis/Valkey is positioned as an **escape hatch** — only needed when L1+L2 can't keep up in multi-instance deployments:

| When L3 is needed | Why |
|---|---|
| Session tokens across 5+ instances | Each instance caches separately → stale reads on revocation |
| Rate limiting across instances | In-memory buckets aren't shared → limits reset on different instance |
| Auth request state across instances | Login started on instance A, callback lands on instance B |

```toml
[cache]
backend = "memory"    # default: L1+L2 only (zero-config)
# backend = "redis"   # adds L3 for specific cross-instance state
# redis_url = "redis://localhost:6379"
```

When `backend = "redis"`, **only specific caches** are promoted:
- Session token → session validation
- Rate limiter buckets
- Auth request state

Everything else stays in L2. Redis is not a blanket cache — it's targeted at cross-instance consistency.

### L4: HTTP / CDN (Client-Side)

Automatic caching via HTTP headers:

| Header | What | Value |
|---|---|---|
| `ETag` | OIDC Discovery, JWKS, schema definitions | Hash of content |
| `Cache-Control` | Static UI assets | `max-age=3600, public` |
| `Cache-Control` | API responses | `no-cache` (revalidate via ETag) |

## Cache Mechanics

### Eviction Policies

| Policy | Description | When to use |
|---|---|---|
| **TTL** | Entries expire after a fixed duration | Most caches |
| **LRU** | Least Recently Used eviction at capacity | In-memory maps, bounded caches |
| **Ring Buffer** | Oldest dropped when max reached | Log buffer |
| **Event-Driven** | Invalidated by domain events | Settings, schemas |

### Per-Item Mechanics

| Data | Eviction | Persistence | Consistency | Invalidation |
|---|---|---|---|---|
| **Log buffer** | Ring buffer (50K) | ✅ SQLite | Eventual | Background drain |
| **Schema cache** | TTL 60s + event | ✅ SQLite | Read-your-writes | `schema.updated` |
| **Settings cache** | TTL 30s + event | ✅ SQLite | Eventual | `settings.updated` |
| **Query results** | TTL 30s | ✅ SQLite | Eventual | TTL only |
| **Rate limiter** | TTL (bucket) | ❌ Ephemeral | Strong | GC timer |
| **Compiled expr** | LRU (500) | ❌ Ephemeral | Strong | `settings.updated` |
| **Session tokens** | TTL 5min | ❌ or Redis | Eventual | `session.revoked` |

### Generic Interface

All caches implement a common interface for backend swappability:

```go
type Cache[K comparable, V any] interface {
    Get(ctx context.Context, key K) (V, bool)
    Set(ctx context.Context, key K, value V, opts ...CacheOption) error
    Delete(ctx context.Context, key K) error
    Clear(ctx context.Context) error
}

func WithTTL(d time.Duration) CacheOption
func WithMaxEntries(n int) CacheOption  // LRU
```

## Decision

1. **L1+L2 are the default** — zero external deps, single binary promise preserved
2. **L3 (Redis) is opt-in** — escape hatch for multi-instance consistency
3. **L4 is automatic** — ETag and Cache-Control added to cacheable endpoints
4. **Flat 30s TTL for query cache** — simple, revisit after load tests
5. **PG unlogged tables** — deferred to post-performance-testing
6. **Cold start / start-without-migrations** — deferred to separate devex workstream

## Consequences

- **Zero-config caching** — SQLite cache works on first run, no Redis needed
- **Graceful scaling** — add Redis when needed, not before
- **tmpfs compatible** — SQLite cache can be placed on tmpfs for in-memory speed
- **Disposable cache** — delete `zitadel-cache.db` at any time, no data loss (just refill delay)
- **Query caching reduces DB load** — dashboard pages served from cache
- **ETag reduces network** — unchanged OIDC Discovery returns 304
