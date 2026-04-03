# Storage Implementation Status

**Date**: 2026-04-02  
**Scope**: current repository state, not future Redis/analytics expansion  
**Related**: [Storage Architecture](storage-architecture.md), [ADR-010](../adr/010-three-tier-data.md), [ADR-017](../adr/017-caching-tiers.md)

## Verdict

The storage reset is now real in the codebase:

- `storage.*` is the canonical server config namespace
- `storage.stateful` is the only required block
- `StorageRuntime::from_config` derives the remaining roles automatically
- SQLite and Postgres both run through the same role-based runtime

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

| `storage.stateful.url` | Derived `read` | Derived `kv` | Derived `sink` | Derived `process_cache` | Derived `analytics` |
|---|---|---|---|---|---|
| SQLite | `same_connection` | `memory` | `channel` | `memory` | same stateful |
| Postgres | `same_primary` | `postgres_unlogged` | `postgres` | `memory` | same stateful |

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

The higher-level server wiring and testkit now depend on these names instead of the older `StateDb`/`EdgeReadDb` naming.

### 6. Analytics remains stable and unchanged

Still true today:

- observability buffering uses the local SQLite analytics cache
- analytics queries use the SQL backend
- `storage.analytics` exists in config/schema/docs, but advanced analytics backends are not implemented yet

This is intentional. The analytics workstream remains separate from the storage role reset.

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

### 4. Full distributed validation for Postgres + sink

The code supports the derived Postgres topology and the role model is in place, but the strongest automated coverage today is still SQLite-first plus targeted runtime derivation tests.

Future work should deepen:

- multi-instance Postgres session visibility
- replica read semantics
- Redis/Valkey split-topology behavior

## Test Coverage Snapshot

### Directly covered now

- storage config loading and schema generation
- SQLite derived role defaults
- Postgres derived role defaults
- legacy `[database]` rejection
- transient record emission
- sink failure not breaking the auth hot path
- consume-once provider auth state
- channel sink persisting memory-backed sessions into SQLite
- login flow/session behavior through the shared runtime

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
