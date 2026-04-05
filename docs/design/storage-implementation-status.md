# Storage Implementation Status

**Date**: 2026-04-05  
**Scope**: current repository state, not future Redis/analytics expansion  
**Related**: [Storage Architecture](storage-architecture.md), [ADR-010](../adr/010-three-tier-data.md), [ADR-017](../adr/017-caching-tiers.md)

## Verdict

The storage reset is now real in the codebase:

- `storage.*` is the canonical server config namespace
- `storage.stateful` is the only required block
- `StorageRuntime::from_config` derives the remaining roles automatically
- SQLite, Postgres, and native Spanner now share the same backend boundary

The current POC is no longer “just shared SQL pretending to be future architecture.” It now has a working Level 0/Level 1 storage runtime with real role separation.

## What Works Today

### 1. Canonical `storage.*` config is live

Implemented:

- `crates/zitadel-config/src/storage.rs`
- `crates/zitadel-config/src/lib.rs`
- `zitadel.reference.toml`

Behavior:

- `storage.stateful` replaces `database`
- old `[database]` config fails fast with a migration error
- old `ZITADEL_DATABASE_*` env vars fail fast with migration guidance
- `ZITADEL_STORAGE__...` nested env vars and `ZITADEL_STORAGE_STATEFUL_*` flat env vars work

### 2. Runtime role derivation is live

Implemented:

- `crates/zitadel-storage/src/runtime.rs`
- `crates/zitadel-server/src/lib.rs`
- `crates/zitadel-testkit/src/lib.rs`

Current defaults:

| `storage.stateful.backend` | Derived `read` | Derived `kv` | Derived `sink` | Derived `process_cache` | Derived `analytics` |
|---|---|---|---|---|---|
| SQLite | `same_connection` | `memory` | `channel` | `memory` | same stateful |
| Postgres | `same_primary` | `postgres_unlogged` | `postgres` | `memory` | same stateful |
| Spanner | `same_primary` | `shared_sql` | `noop` | `memory` | Spanner analytics backend |

### 3. `KvStore` is real for auth transient state

Implemented:

- `MemoryKvStore`
- `SqlKvStore`
- `TransientStorage<KvStore, Sink>`

Covered data families:

- sessions
- login flow runtime state
- provider auth state
- auth request completion state

Coverage exists in:

- `crates/zitadel-storage/src/transient/mod.rs`
- `crates/zitadel-login/src/lib.rs`
- `crates/zitadel-server/tests/router_contract.rs`

### 4. `Sink` is real for transient promotion

Implemented:

- `ChannelSink`
- `SqlSink`
- typed `TransientRecord` payloads
- background ingest into the main stateful tables

Important detail:

- the sink is no longer a no-op in the default runtime
- SQLite local mode now uses `memory KV + channel sink`
- Postgres default mode uses `postgres KV + postgres inbox sink`

Coverage exists in:

- `crates/zitadel-storage/src/transient/mod.rs`
- `crates/zitadel-login/src/lib.rs`

### 5. Stateful/read role naming is live

Implemented:

- `StatefulStore`
- `ReadStore`
- `SqlStatefulStore`
- `SqlReadStore`
- `SpannerStatefulStore`
- `SpannerReadStore`

The higher-level server wiring and testkit now depend on these names instead of the older `StateDb`/`EdgeReadDb` naming.

### 6. Analytics now has a native Spanner lane

Current behavior:

- observability buffering still uses the local SQLite analytics cache
- SQLite and Postgres analytics query through the SQL backend
- Spanner analytics query and schema introspection now use the native GoogleSQL backend
- dedicated external analytics backends are still out of scope for this pass

### 7. Mounted runtime paths now run without Spanner route guards

Current behavior:

- the API no longer uses a `spanner_backend_guard`
- the login router no longer uses a `spanner_login_guard`
- catalog install, SSO callback completion, OIDC adapters, and host-based routing now go through backend-aware DB helpers instead of ad-hoc SQL in the route layer
- `zitadel_db::repos` now provides a domain-oriented facade over the retained-data boundary, even though the internal implementation is still being carved out of the older monolithic module

## What Is Still Missing

### 1. Redis / Valkey backends

The docs and config now reserve `backend = "redis"` for both Redis and Valkey-compatible servers, but the runtime does not implement those backends yet.

Current behavior:

- `storage.kv.backend = "redis"` fails clearly at startup
- `storage.sink.backend = "redis"` fails clearly at startup

This is deliberate. The config surface is defined, but the runtime stays honest.

### 2. Dedicated process-cache backends

`ProcessCache` is defined as a role, but only `memory` is implemented in this pass.

Not implemented yet:

- SQLite-backed process cache
- cache invalidation policies beyond in-process behavior
- promoting settings/domain/query caches into the new role

### 3. Dedicated analytics backends

`storage.analytics` is reserved, but only “same stateful database” behavior exists today.

Not implemented yet:

- dedicated Postgres analytics backend
- ClickHouse backend
- alternate analytics query runtime

### 4. Full distributed validation for Postgres + Spanner

The code supports the derived Postgres topology and the native Spanner topology, but the strongest automated coverage today is still SQLite-first plus targeted contract tests and boundary checks.

Future work should deepen:

- multi-instance Postgres session visibility
- replica read semantics
- native Spanner emulator full-matrix end-to-end coverage
- Redis/Valkey split-topology behavior

## Test Coverage Snapshot

### Directly covered now

- storage config loading and schema generation
- SQLite derived role defaults
- Postgres derived role defaults
- Spanner derived role defaults
- legacy `[database]` rejection
- transient record emission
- sink failure not breaking the auth hot path
- consume-once provider auth state
- channel sink persisting memory-backed sessions into SQLite
- login flow/session behavior through the shared runtime
- backend-boundary regression tests for the converted native Spanner runtime paths

### Still worth expanding

- Postgres sink replay/restart behavior
- replica-read overrides
- Redis/Valkey once implemented
- `storage.analytics` override failure modes

## Practical Conclusion

If the question is:

- “Is `storage.*` now the real config model?”  
  **Yes.**

- “Does the runtime really derive `read`, `kv`, `sink`, `process_cache`, and `analytics` from `storage.stateful`?”  
  **Yes.**

- “Is Redis/Valkey already implemented?”  
  **No. The role surface is defined, but those backends still fail clearly.**

- “Is the POC still SQLite-first with zero external dependencies?”  
  **Yes. SQLite local mode now runs through the real role-based runtime instead of a special-case path.**
