# Storage Architecture — Role-Based Runtime, Database-First Defaults

**Date**: 2026-04-02  
**Builds on**: ADR-010 (Three-Tier Data), ADR-017 (Process Cache Semantics), ADR-034 (Multi-Tenancy)  
**Related**: [Storage Implementation Status](storage-implementation-status.md), [Architecture Overview](../architecture/overview.md)

This document is the canonical storage design for the current Rust prototype.

The key reset is simple:

- operators configure `storage.*`
- most deployments only set `[storage.stateful]`
- the runtime derives the remaining storage roles automatically

That keeps the common path simple while still giving us a stable internal model that can grow toward read replicas, Redis/Valkey, and dedicated analytics backends later.

## Operator Model

The default operator story is database-first:

- choose SQLite for local or single-node operation
- choose Postgres for shared or multi-container operation
- let the runtime derive the rest

The canonical config shape is:

```toml
[storage.stateful]
url = "sqlite://./data/zitadel.db"
# or:
# url = "postgres://user:pass@host:5432/zitadel"
migrate = "auto"
bootstrap = "auto"

[storage.read]
# optional override

[storage.kv]
# optional override

[storage.sink]
# optional override

[storage.process_cache]
# optional override

[storage.analytics]
# reserved now, advanced analytics backends later
```

Only `storage.stateful` is required.

## Runtime Roles

Internally the runtime is always built from the same role set:

| Config role | Code interface | Purpose | Implementations in this phase |
|---|---|---|---|
| `storage.stateful` | `StatefulStore` | Durable transactional source of truth for stable relational data | SQLite, Postgres |
| `storage.read` | `ReadStore` | Stable-data read path | same connection, same primary, Postgres replica |
| `storage.kv` | `KvStore` | Live transient state with TTL and consume-once semantics | memory, Postgres-backed transient tables, Redis family reserved |
| `storage.sink` | `Sink` | Async promotion path from transient writes into durable state | in-process channel, Postgres inbox, Redis family reserved |
| `storage.process_cache` | `ProcessCache` | Local read acceleration only | memory |
| `storage.analytics` | analytics backend | Analytical query/storage role | same stateful store today |

These roles are semantic. Backends are implementation choices underneath them.

## Control Plane And Auth Data Plane

The storage-role model exists partly to support a specific failure model:

- the **control plane** owns admin mutations, routing, placement, provider and policy authoring
- the **auth data plane** owns end-user login, session and token handling, and auth runtime state

That means `storage.read`, `storage.kv`, and `storage.sink` are not only about performance. They are also the building blocks for regional auth continuity when the authoritative control-plane or home-region write path is degraded.

## Derived Defaults

### SQLite default

If only `storage.stateful.url = "sqlite://..."` is set, the runtime derives:

| Role | Default |
|---|---|
| `stateful` | SQLite |
| `read` | `same_connection` |
| `kv` | `memory` |
| `sink` | `channel` + in-process batch ingestor |
| `process_cache` | `memory` |
| `analytics` | same SQLite database |

This is the Level 0 local/single-node profile:

- transient auth state stays fast and in-memory
- sink batching persists that transient state back into SQLite asynchronously
- no external dependencies are required

### Postgres default

If only `storage.stateful.url = "postgres://..."` is set, the runtime derives:

| Role | Default |
|---|---|
| `stateful` | Postgres |
| `read` | `same_primary` |
| `kv` | `postgres_unlogged` |
| `sink` | Postgres inbox/spool + ingestor |
| `process_cache` | `memory` |
| `analytics` | same Postgres database |

This is the default shared/multi-container profile:

- transient auth state is immediately visible across instances
- the sink remains a separate ingestion boundary
- Redis/Valkey is optional, not required on day one

## Advanced Overrides

Advanced deployments can override individual roles without replacing the whole model.

Supported first-growth patterns:

```toml
[storage.stateful]
url = "postgres://primary/zitadel"

[storage.read]
backend = "postgres_replica"
url = "postgres://replica/zitadel"

[storage.kv]
backend = "redis"
url = "redis://cache.internal:6379"

[storage.sink]
backend = "redis"
url = "redis://cache.internal:6379"
```

Current runtime status:

- `postgres_replica` is supported as a read override
- `redis` is reserved as the backend family name for Redis and Valkey-compatible servers
- Redis/Valkey-backed `kv` and `sink` are not implemented yet in this POC and fail clearly at startup

## Data Families

### Stable data

Stable data is written synchronously to `StatefulStore` and read through `ReadStore`.

Examples:

- users
- orgs
- providers
- passwords and MFA configuration
- OIDC client definitions
- FGA tuples and authorization models
- PATs

Properties:

- relational integrity matters
- uniqueness matters
- write volume is relatively low
- if `StatefulStore` is unavailable, management writes fail

### Transient auth state

Transient auth state is written to `KvStore` first and promoted via `Sink`.

Examples:

- sessions
- login flow runtime state
- auth request redirect/progress state
- OIDC authorization codes and related auth request state
- provider auth state

Properties:

- TTL and consume-once behavior matter
- the hot path should avoid blocking on durable persistence
- local correctness comes from `KvStore`
- durable retention comes from `Sink` ingesting into `StatefulStore`
- in regional continuity mode, this layer can continue even when control-plane writes are paused

### Analytics and observability

Observability remains governed by ADR-010:

- request and runtime logs go through the observability SQLite buffer
- analytical queries use the analytics role
- `storage.analytics` is reserved now so analytics storage can join the same operator-facing namespace later

## Sink Semantics

`Sink` is not “blindly batch-insert rows into the main database.”

Internally it is split into three responsibilities:

| Component | Responsibility |
|---|---|
| `SinkEmitter` | called by the hot path, returns quickly |
| `SinkBuffer` | bounded queue/spool/inbox for retry and replay |
| `StatefulIngestor` | background worker that applies idempotent updates into `StatefulStore` |

That separation matters because the sink is the fault-isolation boundary between:

- transient hot-path state
- durable retained state
- future archive/export paths
- regional auth continuity and authoritative reconciliation

## Process Cache

`ProcessCache` is a first-class role, but it is intentionally narrow:

- local read acceleration only
- never a distributed correctness mechanism
- memory-only in this phase

This keeps it distinct from:

- `KvStore`, which owns transient auth correctness
- the observability SQLite buffer, which is a durable analytics drain path
- the control-plane routing cache, which must not become an auth correctness dependency

## Regional Continuity

For larger managed deployments, regional read models and replicas exist for auth continuity, not only for lower latency:

- `storage.read` can provide the regional stable-data read path during control-plane or home-region degradation
- `storage.kv` is the writable regional auth-runtime layer
- `storage.sink` is the replay and reconciliation path back to the authoritative plane

This is why `process_cache` remains intentionally narrow. It is not allowed to stand in for distributed auth correctness.

SQLite-backed process cache is a valid future optimization, but not part of the default runtime in this pass.

## Multi-Tenancy

Multi-tenancy still layers on top of the same runtime roles:

| Role | Shared infrastructure | Dedicated infrastructure |
|---|---|---|
| `stateful` | shared DB with `instance_id` scoping | dedicated DB per tenant |
| `read` | same shared DB or replica with `instance_id` scoping | dedicated read path |
| `kv` | shared KV namespace/prefixes by `instance_id` | dedicated KV per tenant |
| `sink` | shared inbox/stream tagged by `instance_id` | dedicated inbox/stream |
| `analytics` | shared analytics backend filtered by `instance_id` | dedicated analytics store |

The runtime model stays the same. Only the concrete backend topology changes.

## Growth Path

The intended progression is additive:

1. SQLite only
2. Postgres only
3. Postgres + Redis/Valkey KV
4. Postgres + Redis/Valkey KV + Redis sink + read replicas
5. Dedicated analytics backend under `storage.analytics`

| Profile | `stateful` | `read` | `kv` | `sink` | `analytics` |
|---|---|---|---|---|---|
| Local SQLite | SQLite | same connection | memory | channel | same SQLite |
| Shared Postgres | Postgres | same primary | Postgres transient tables | Postgres inbox | same Postgres |
| Split hot path | Postgres | same primary or replica | Redis/Valkey | Postgres or Redis | same Postgres |
| Distributed | Postgres | replicas | Redis/Valkey | Redis/stream or platform queue | dedicated backend |

## Relationship to ADRs

- **ADR-010** still governs the analytical and observability domains. `storage.analytics` aligns the config namespace, but it does not replace the three-tier data model.
- **ADR-017** now covers `ProcessCache` specifically, rather than trying to describe KV, sink, and analytics buffering all as one “cache” concept.
- **ADR-026** remains one deployment-specific interpretation of these same roles.

## Practical Rule

When documenting or explaining storage:

- say “configure `storage.stateful`” for the common path
- say “override `storage.read`, `storage.kv`, or `storage.sink`” for advanced paths
- keep backend names like SQLite, Postgres, Redis, and Valkey as implementation details under those roles
