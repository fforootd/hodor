# Storage Architecture — Three Stores, Optional Shared Cache

**Date**: 2026-04-07  
**Builds on**: ADR-010 (Three-Tier Data), ADR-017 (Caching Tiers), ADR-034 (Multi-Tenancy)  
**Related**: [Storage Implementation Status](storage-implementation-status.md), [Architecture Overview](../architecture/overview.md)

This document is the canonical storage design for the current Rust prototype.

## Operator Model

The operator-facing model is intentionally small:

- `storage.primary` is the authoritative durable database
- `storage.transient` is the authoritative auth-runtime database
- `storage.analytics` is the analytics database
- `cache.shared` is an optional accelerator for safe metadata reads

Most deployments only set `storage.primary`. `transient` and `analytics` inherit `primary` by default.

```toml
[storage.primary]
url = "sqlite://./data/zitadel.db"
migrate = "auto"
bootstrap = "auto"

[storage.primary.replica]
enabled = false
mode = "explicit"
# url = "postgres://readonly@pg-replica:5432/zitadel"

[storage.transient]
backend = "inherit"

[storage.analytics]
backend = "inherit"

[cache.shared]
backend = "disabled"
# backend = "db"
# url = "sqlite://./data/zitadel-cache.db"
```

## Defaults

### SQLite

If only `storage.primary.url = "sqlite://..."` is set:

- `primary` uses SQLite
- `transient` inherits the same SQLite database
- `analytics` inherits the same SQLite database
- `cache.shared` is disabled
- local per-process caching stays memory-only

### Postgres

If only `storage.primary.url = "postgres://..."` is set:

- `primary` uses Postgres
- `transient` inherits the same Postgres database
- `analytics` inherits the same Postgres database
- optional `storage.primary.replica` can serve explicitly stale-tolerant reads
- local per-process caching stays memory-only

### Spanner

If `storage.primary.backend = "spanner"` or `storage.primary.database` is set:

- `primary` uses native Spanner
- `transient` and `analytics` inherit Spanner by default
- replica reads are not part of the Spanner path in this POC
- shared cache stays optional and best-effort

## Semantics

The simplified public model does not collapse the data semantics.

### Primary

`storage.primary` owns durable relational state:

- users
- orgs
- providers
- settings
- passwords and factors
- PATs
- routing metadata

Default read behavior is strong. If the primary DB is unavailable, these operations fail.

### Transient

`storage.transient` owns auth-runtime state directly:

- sessions
- login flow state
- provider callback state
- auth-request progression

The POC favors simplicity over degraded-mode auth continuity. There is no default `kv + sink` replay path in the runtime anymore. If the configured transient DB is unavailable, those operations fail.

### Analytics

`storage.analytics` owns analytical queries and observability ingestion targets.

The observability SQLite buffer remains separate under `observability.cache_path`. It is not part of the generic cache model.

## Replica Reads

Replica reads are a capability of `storage.primary`, not a fourth storage type.

- only Postgres supports this path in the current POC
- the config lives under `storage.primary.replica`
- `mode = "explicit"` means queries must opt in
- default reads remain strong and hit the primary
- on replica failure, the runtime falls back to the primary and logs the fallback

Examples of good `StaleOk` candidates:

- search
- browse views
- non-critical admin lists
- metadata reads where slight lag is acceptable

Examples that must remain strong:

- session validation
- revocation checks
- logout-all
- consume-once auth state
- provider and policy changes that affect active auth

## Shared Cache

`cache.shared` is optional and non-authoritative.

- cache failures must not fail requests
- write-through or invalidate-on-write is preferred
- stale cache reads are only safe for stable metadata
- local caching remains per-process memory TTL/LRU

The current POC wires `cache.shared.backend = "db"` into instance-routing metadata. Redis/Valkey remains future work.

## Internal Boundary

The runtime still keeps separate code paths for:

- durable primary reads and writes
- transient auth-runtime operations
- analytics queries

That internal separation preserves room for future evolution, but it is no longer the first thing operators need to learn.
