# Storage Implementation Status

**Date**: 2026-04-01
**Scope**: current repository state, not the target architecture
**Related**: [Storage Architecture](storage-architecture.md), [ADR-010](../adr/010-three-tier-data.md), [Event Pipeline](../architecture/event-pipeline.md)

## Verdict

The current POC has a working and well-tested **shared SQL storage layer** with multi-tenant scoping via `instance_id`.

It also has a working and tested **durable analytics buffer** for structured request and runtime logs:

- logger
- local SQLite cache
- background drainer
- `events` table

What it does **not** have yet is the full **edge-first four-primitive storage architecture** described in [Storage Architecture](storage-architecture.md). In particular:

- transient auth data is still written directly to SQL on the request path
- there is no real `EdgeKV` implementation for sessions/tokens/auth state
- there is no separate `EdgeSink` or queue for transient auth writes
- the only analytics query backend today is the OLTP database itself

So the short answer is:

- **Tiered three-tier data flow**: partially real, partially tested, partially documented
- **Edge-first four-primitive storage**: mostly target architecture, not current implementation

## What Works Today

### 1. Shared OLTP storage works

The current binary uses the main SQL database as the source of truth for users, sessions, tokens, auth states, events, settings, secrets, and related resources.

Evidence in code:

- `internal/api/session_create.go`
- `internal/api/token.go`
- `internal/auth/password.go`
- `internal/oidcop/storage_auth_request.go`
- `internal/oidcop/storage_tokens.go`
- `internal/crypto/store.go`

Automated coverage exists for the core SQL layer:

- `internal/database/database_test.go`
- `internal/database/schema_test.go`
- `internal/database/postgres_test.go`
- `internal/api/postgres_integration_test.go`

### 2. Multi-tenant SQL scoping works

The current storage model is shared infrastructure partitioned by `instance_id`. This is the real multi-tenant boundary in the current POC.

Evidence in code:

- `internal/database/scoped.go`
- `internal/api/isolation_test.go`
- `internal/auth/password_test.go`
- `internal/loginflow/resolver_test.go`
- `internal/risk/risk_storage_test.go`
- `internal/tenantaudit/tenant_sql_audit_test.go`
- `internal/tenantaudit/storage_contract_audit_test.go`

This part is implemented, tested, and enforced much more strongly than the edge-storage story.

### 3. Tier 2 durable analytics buffering works for request and runtime logs

Structured request and runtime logs do not write straight to the database. They go through the local SQLite cache and drain asynchronously.

Implementation:

- `internal/logging/cache.go`
- `internal/logging/cache_sink.go`
- `internal/logging/drainer.go`
- `internal/api/middleware.go`

Automated coverage:

- `internal/logging/cache_test.go`
- `internal/logging/cache_sink_test.go`
- `internal/logging/drainer_test.go`
- `internal/logging/logging_test.go`

This is the strongest example of the tiered storage model actually existing in code today.

### 4. Tier 3 fire-and-forget fan-out exists

The logging layer has fan-out, redaction, and circuit-breaker behavior for non-critical sinks.

Implementation:

- `internal/logging/handler.go`
- `internal/logging/logging.go`
- `internal/logging/sinks.go`

Automated coverage:

- `internal/logging/logging_test.go`
- `internal/logging/fuzz_test.go`

Important caveat:

- `stdout` is real
- the `otel` sink is currently a **POC stub** that writes OTEL-shaped JSON to stdout
- it is **not** a real OTLP exporter yet

See `internal/logging/sinks.go` for the explicit TODO.

## What Is Not Implemented End-to-End

### 1. There is no real EdgeKV for auth transient data

The target architecture says transient auth data should be written to edge-local KV and only later ingested centrally. That is not what the current binary does.

Current hot-path SQL writes:

- sessions: `internal/api/session_create.go`
- auth requests: `internal/oidcop/storage_auth_request.go`
- OIDC access and refresh tokens: `internal/oidcop/storage_tokens.go`
- PATs: `internal/api/pat.go`
- magic-link tokens: `internal/login/login.go`

These all write directly to SQL tables such as:

- `sessions`
- `tokens`
- `auth_states`

That means the current system is still a **shared SQL system with tenant scoping**, not an edge-KV system.

### 2. There is no separate EdgeSink / queue for transient auth writes

The target storage architecture describes a queue that ships transient auth writes from edge to central.

That queue does not currently exist in code for auth storage:

- no `event_inbox`
- no Redis stream implementation
- no SQS / Kafka implementation
- no transient-ingestion worker that moves sessions/tokens/auth states from a queue into OLTP

What *does* exist today:

- the `events` table acts as the durable queue for async consumers described in [Event Pipeline](../architecture/event-pipeline.md)
- `internal/eventbus` is an in-memory wake-up signal for those consumers
- `internal/notify` has its own notification request queue table

That is real async processing, but it is **not** the same thing as the edge transient-write queue described in [Storage Architecture](storage-architecture.md).

### 3. EdgeReadDB is only partial

There is a concrete split read/write experiment for Turso in:

- `internal/database/tursosync.go`

This gives:

- local partial-sync read replica
- remote write path
- pull-after-write behavior

But as of this repo state:

- there are **no automated tests** for `tursosync.go`
- it is not the default server path
- the higher-level auth/session/token stores are not abstracted against a generic edge-read interface

So this is best described as **partial groundwork**, not a verified storage tier.

### 4. Dedicated OLAP backends are not implemented

The analytics package has a backend interface, but only one real implementation:

- `internal/analytics/engine.go` → `OLTPBackend`

The server always wires:

- `analytics.NewOLTPBackend(db.SQL(), db.Dialect())`

There is currently no real:

- dedicated Postgres analytics backend
- ClickHouse backend
- alternate analytics storage implementation

There are also no dedicated tests for `/v1/analytics/query`, `/v1/analytics/tables`, or `/v1/analytics/schema`.

### 5. Tier 2 behavior is mixed for signal data

Request and runtime logs use the cache-and-drain path, but some signal events still write directly to `events` inside SQL transactions.

Examples:

- `internal/api/session_create.go` writes `signal.risk_evaluated` directly
- `internal/login/flow_handlers_risk.go` writes `signal.risk_evaluated` directly
- `internal/api/otel_ingest.go` writes `signal.session_trace` directly

So "Tier 2" is not one uniform pipeline today.

## Test Coverage Summary

### Implemented and tested

- SQL database open and migration paths for SQLite and Postgres
- tenant scoping via `instance_id`
- password storage isolation
- login flow isolation
- OIDC auth-request and token lifecycle
- token resolution and session lifecycle
- local analytics cache
- cache sink routing and sampling
- analytics drainer behavior and circuit-breaker behavior
- logging fan-out and redaction

### Implemented with weak or missing direct coverage

- Turso split read/write replica path in `internal/database/tursosync.go`
- analytics query API and backend behavior
- end-to-end proof that every documented Tier 2 event source uses the same buffered path

## Documentation Status

### Accurate enough for current implementation

- [ADR-010](../adr/010-three-tier-data.md): accurately describes the current request/log cache-and-drain pipeline at a high level
- [Event Pipeline](../architecture/event-pipeline.md): accurately describes the current events-table async consumer model

### Target architecture, not current implementation

- [Storage Architecture](storage-architecture.md): this is the **target** edge-first storage model, not the current storage implementation

### Current contradictions to be aware of

1. `storage-architecture.md` says the central database is never in the auth hot path for transient writes.
   Current code still writes sessions, tokens, PATs, magic links, and auth states directly to SQL.

2. `event-pipeline.md` says the events table is the queue.
   `storage-architecture.md` describes a separate edge-to-central queue for transient auth writes.

3. `developer-experience.md` and `README.md` describe Level 0 as if KV + queue are already the live auth storage path.
   In reality, the only strongly-realized tiered storage path today is the logging cache/drainer path.

## Practical Conclusion

If the question is:

- "Does the current POC storage layer work?"  
  **Yes**. The shared SQL storage layer works, multi-tenant scoping is tested, and the analytics cache/drain path works.

- "Is the full tiered edge-first storage architecture implemented and verified?"  
  **No**. That architecture is still mostly a design target.

- "Is it documented?"  
  **Yes, but inconsistently**. The repository has extensive storage documentation, but it currently mixes target architecture and present reality. This document is the reality check.
