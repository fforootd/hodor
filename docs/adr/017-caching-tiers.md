# ADR-017: Process Cache Semantics

**Status**: Proposed  
**Date**: 2026-03-28  
**Builds on**: ADR-010 (Three-Tier Data Architecture)  
**Related**: [Storage Architecture](../design/storage-architecture.md), [Glossary](../GLOSSARY.md)

## Context

The earlier cache ADR mixed together several different concerns:

- local read caching
- shared transient auth state
- analytics buffering
- Redis as a generic “escape hatch”

That made the storage story harder to reason about. The storage reset now defines explicit runtime roles:

- `stateful`
- `read`
- `kv`
- `sink`
- `process_cache`
- `analytics`

This ADR now narrows its scope to `ProcessCache` only.

## Decision

`ProcessCache` is a local, per-process read-acceleration layer.

It is:

- optional
- local-only
- never a source of truth
- never a distributed correctness mechanism

It is not:

- the transient auth state store
- the sink buffer
- the analytics SQLite buffer

## Role Boundaries

| Role | Owns | Does not own |
|---|---|---|
| `process_cache` | local read acceleration | distributed session/auth correctness |
| `kv` | transient auth state, TTL, consume-once behavior | durable retained history |
| `sink` | buffering, replay, ingestion into durable state | local read acceleration |
| observability cache | analytics drain buffering | general app cache semantics |

This separation is intentional. It keeps “cache” from becoming a catch-all word for every fast path.

## Default Implementation

The default implementation in this phase is:

- backend: `memory`
- scope: per process
- lifecycle: disposable

The role exists in config now:

```toml
[storage.process_cache]
backend = "memory"
```

Only `memory` is implemented in this pass.

## Candidate Uses

`ProcessCache` is appropriate for:

- domain-to-org lookups
- provider metadata
- schema and settings reads
- local query/result caching where stale reads are acceptable

`ProcessCache` is not appropriate for:

- shared sessions across instances
- auth request callbacks that may land on another instance
- consume-once security state
- durable buffering before ingest

Those belong to `kv` or `sink`, not `process_cache`.

## SQLite-Backed Cache

A SQLite-backed local cache remains a valid future optimization, but it is not the default runtime for this pass.

If added later, it should still preserve the same semantics:

- local-only
- disposable
- not a correctness dependency

That makes it useful for:

- larger local caches than in-memory LRU alone
- faster warm starts on one machine
- bounded query/result caches

But even then it must remain distinct from:

- `storage.kv`
- `storage.sink`
- `observability.cache_path`

## Consequences

- the common storage story is clearer
- Redis/Valkey is positioned under `storage.kv` or `storage.sink`, not as a blanket cache
- the observability cache can keep its own lifecycle and settings
- future SQLite-backed local caching can be added without re-blurring the role model
