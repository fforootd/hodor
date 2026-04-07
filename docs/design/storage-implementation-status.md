# Storage Implementation Status

**Date**: 2026-04-07  
**Scope**: current repository state, not future Redis/analytics expansion  
**Related**: [Storage Architecture](storage-architecture.md), [ADR-010](../adr/010-three-tier-data.md), [ADR-017](../adr/017-caching-tiers.md)

## Verdict

The simplified storage model is now live in the codebase:

- `storage.primary`, `storage.transient`, and `storage.analytics` are the public config model
- old `storage.stateful`, `storage.read`, `storage.kv`, `storage.sink`, and `storage.process_cache` settings fail fast
- `cache.shared` exists as a separate config surface
- Postgres replica reads are available as an explicit capability under `storage.primary.replica`

The POC now favors operator simplicity over degraded-mode auth continuity.

## What Works Today

### 1. Canonical config surface

Implemented:

- `crates/zitadel-config/src/storage.rs`
- `crates/zitadel-config/src/cache.rs`
- `crates/zitadel-config/src/lib.rs`
- `crates/zitadel-config/src/env.rs`
- `zitadel.reference.toml`

Behavior:

- `storage.primary` replaces `storage.stateful`
- `storage.transient` and `storage.analytics` inherit `primary` by default
- `cache.shared` is separate from `storage.*`
- legacy TOML keys and env vars fail fast with migration guidance

### 2. Runtime wiring

Implemented:

- `crates/zitadel-storage/src/runtime.rs`
- `crates/zitadel-server/src/lib.rs`
- `crates/zitadel-testkit/src/lib.rs`

Current runtime shape:

- `primary` is the durable authoritative DB
- `transient` is the auth-runtime DB
- `analytics` is the analytical DB
- separate transient and analytics SQL databases can be prepared automatically
- Postgres replicas are only used when a read path explicitly opts into stale reads

### 3. Explicit replica reads

Implemented:

- `ReadConsistency::{Strong, StaleOk}`
- replica wiring in `crates/zitadel-storage/src/stateful.rs`
- replica fallback logging in stale-tolerant reads
- search repository routing through the replica path

Current behavior:

- default reads stay strong
- stale-tolerant reads may hit `storage.primary.replica`
- replica failures fall back to primary

### 4. Direct transient authority

Implemented:

- sessions are written directly to the configured transient DB
- session reads do not fall back to the primary DB when a distinct transient DB is configured
- login flow state and provider callback state still use the transient storage APIs directly

The sink replay model is no longer part of the default runtime path.

### 5. Shared cache

Implemented:

- `cache.shared.backend = "db"` for instance-routing metadata
- local per-process LRU/TTL cache remains in place
- shared-cache failures log and fall back to direct DB reads

Current scope:

- instance and host resolution only
- metadata only
- non-authoritative

### 6. Analytics separation

Implemented:

- `storage.analytics` can inherit `primary` or point at its own SQL backend
- observability still uses the dedicated local SQLite buffer under `observability.cache_path`
- native Spanner analytics support remains intact

## What Is Still Missing

### 1. Redis / Valkey shared cache

The config surface reserves `cache.shared.backend = "redis"`, but this POC does not implement it yet.

Current behavior:

- the runtime logs a warning and continues without shared cache

### 2. Full transient move for every auth-request path

The session path now treats `storage.transient` as authoritative, but some OIDC auth-request creation and completion flows still rely on the retained-data boundary.

Future work should finish moving those paths fully behind the transient storage boundary.

### 3. Dedicated analytics backends beyond SQL / Spanner

Not implemented yet:

- dedicated ClickHouse backend
- alternate analytics query runtimes
- remote analytics pipelines beyond the current observability buffer

## Test Coverage Snapshot

Directly covered now:

- config parsing for the new storage and cache surface
- legacy config rejection
- SQLite, Postgres, and Spanner runtime construction
- Postgres explicit replica reads
- Postgres transient DB separation without session fallback
- shared-cache-safe instance routing behavior

Still worth expanding:

- more stale-tolerant query paths beyond search
- fully transient OIDC auth-request lifecycle coverage
- Redis/Valkey once implemented

## Practical Conclusion

If the question is:

- “Is `storage.primary / transient / analytics` now the real model?”  
  **Yes.**

- “Are replica reads opt-in only?”  
  **Yes.**

- “Is shared cache authoritative?”  
  **No.**

- “Does the POC still stay SQLite-first with zero required external services?”  
  **Yes.**
