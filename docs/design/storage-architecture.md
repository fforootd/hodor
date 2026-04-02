# Storage Architecture — Edge-First Auth with Central Consistency

**Date**: 2026-04-01
**Builds on**: ADR-010 (Three-Tier Data), ADR-017 (Caching Tiers), ADR-021 (Multi-Tenancy)
**Supersedes**: ADR-026 (Container-per-Tenant with D1) — generalized beyond Cloudflare

## Core Principle

The architecture separates **stable data** (users, orgs, configs) from **transient data** (sessions, tokens, auth requests, events). Stable data requires uniqueness and relational integrity. Transient data requires availability and write speed. These two categories have fundamentally different requirements and should never share the same write path.

**The central database is never in the auth hot path for transient writes.** This is the non-negotiable invariant. A noisy tenant spinning up millions of sessions must never impact another tenant's login experience. Auth traffic hits the edge. The central database receives transient data asynchronously, at a controlled rate, via a queue.

The deeper consequence: **auth survives central database failure.** When the central OLTP is overloaded, undergoing maintenance, or experiencing a split-brain scenario, the edge continues to operate normally. The edge read replica has all the stable data (users, keys, FGA tuples) — even if stale by seconds or minutes, this data changes so rarely that staleness is irrelevant for auth flows. The edge KV has all the transient data (sessions, tokens) created since the last sync. The queue absorbs writes that can't reach central yet — they accumulate and drain when central recovers.

This means:

- **Central overloaded:** Queue grows. Edge auth continues at full speed. Central catches up when load subsides. No auth requests fail.
- **Central maintenance window:** Queue accumulates. Edge operates on its read replica + KV. After maintenance, queue drains. Users never noticed.
- **Split brain / network partition:** Each edge region is self-sufficient. Auth works with local replica + KV. Events queue locally. When connectivity restores, queues drain and central reconciles. No data loss — the queue is the durable buffer.
- **Central total loss:** Edge continues serving auth from replica + KV. Management API (user creation, config changes) is unavailable — this is acceptable. Auth is not. Restore central from backup, replay queues, resume normal operation.

The queue is not just a performance optimization. It is the **fault isolation boundary** between the auth hot path and the management/analytics cold path.

## Four Primitives

Every deployment, regardless of scale, is composed of four primitives:

**OLTP** — The central SQL database. Single source of truth for stable data. Receives bulk-inserted transient data from the queue. Eventually contains all data. Enforces uniqueness and relational constraints.

**KV** — The edge transient store. Holds sessions, tokens, auth requests, OIDC codes. Multi-writer, TTL-based expiry, disposable. If the KV loses data, the queue has already captured it for central ingestion. If a KV lookup misses (e.g., session created on another edge), the system falls back to reading from the central OLTP.

**Queue** — Ships transient data from edge to central. Every transient write at the edge also emits to the queue. The central consumer reads the queue and bulk-inserts into OLTP at its own pace. This is the backpressure mechanism that protects central from edge traffic spikes.

**OLAP** (optional) — Analytical queries, usage data, observability. Can be the same OLTP database, or a dedicated store for heavier workloads.

## Data Categories

### Stable Data (Central Write, Edge Read)

Created and modified only through the central management API. Replicated to edge via read replicas.

- Users, passwords, MFA configuration
- Organizations
- Projects, applications, OIDC client configurations
- Roles and grants
- Signing keys
- Identity provider configurations
- Policies (login, password, lockout)
- Authorization 

**Properties:** Low write volume, human-initiated, requires uniqueness and ACID transactions. If the central database is down, management operations fail — this is acceptable.

### Transient Data (Edge Write, Central Async)

Created at the edge during auth flows. Written to KV for immediate access, emitted to the queue for central ingestion.

- Sessions
- Auth requests
- Login Flows
- Access tokens, refresh tokens, ID tokens
- OIDC authorization codes
- Device authorization requests
- Audit events and usage data

**Properties:** High write volume, machine-initiated, no uniqueness requirements, TTL-based lifecycle. If central is down, auth keeps working — this is required.

## Auth Flow at the Edge

Every step is local. No synchronous call to central.

```
1. Resolve user           → read from edge replica (stable data, local)
2. Verify credentials     → local computation against cached password hash
3. Create auth request    → write to edge KV + emit to queue
4. User completes login   → update auth request in edge KV
5. Create session         → write to edge KV + emit to queue
6. Issue tokens           → sign with local key (from edge replica) + emit to queue
7. Token validation       → verify JWT signature (no I/O at all)
8. Userinfo endpoint      → read from edge replica
9. Session lookup miss    → fallback read from central OLTP (rare)
```

## Deployment Profiles

### Level 0 — Local (SQLite)

Everything runs in a single process on a single machine. SQLite is used only in this local mode — never in client/server scenarios.

```
OLTP:          SQLite file
Read replica:  same connection (no replication needed)
KV:            in-memory map with TTL
Queue:         in-process go channel
OLAP:          same SQLite file
Bulk inserter: goroutine reading from channel, writing to SQLite
```

Transient data lives in memory during operation and is flushed to SQLite via the queue (channel → goroutine → INSERT). The full architecture is present — same code paths as production, lighter implementations.

**This is production for small deployments.** A company with 500 users self-hosting Zitadel runs this indefinitely. One binary, one SQLite file, zero operational overhead.

### Level 1 — Scale Out (Postgres)

Postgres replaces SQLite. Edge nodes are separate processes or machines. The KV stays in-memory (or moves to Redis). A queue ships transient data back to central asynchronously.

```
OLTP:          Postgres primary
Read replica:  none yet (edge reads from primary, low latency single region)
KV:            in-memory or Redis
Queue:         Postgres unlogged table or Redis stream
OLAP:          same Postgres instance
```

The central PG unlogged table as a queue:

```sql
CREATE UNLOGGED TABLE event_inbox (
    id         UUID PRIMARY KEY,
    region     TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload    JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);
```

A worker process polls and batch-moves rows to permanent tables at a controlled rate. No external queue infrastructure needed.

**Who runs this:** Mid-market, single region, moderate scale.

### Level 2 — Dedicated KV + Read Replicas

As traffic grows, add a dedicated KV store and PG read replicas to keep the edge fast and the central database protected.

```
OLTP:          Postgres primary
Read replica:  Postgres read replica(s) at edge
KV:            Redis or platform-native (Workers KV, DynamoDB, etc.)
Queue:         PG table, Redis stream, or SQS/Kafka
OLAP:          same PG, or ClickHouse / DuckDB for heavier analytics
```

Read replicas serve stable data locally at the edge. The dedicated KV handles transient data without touching PG on every session creation. The queue scales to dedicated infrastructure if PG inbox table isn't sufficient.

**Who runs this:** Larger deployments, multi-region, enterprise.

### Level 3 — Multi-Region

Central PG primary in one region, read replicas and edge infrastructure in each region.

```
OLTP:          Postgres primary (one region)
Read replica:  PG read replica per region
KV:            Platform-native per region (Redis, ElastiCache, Memorystore)
Queue:         Platform-native per region (SQS, Kafka, Pub/Sub)
OLAP:          ClickHouse, PG with partitions, or platform analytics
```

Each region operates autonomously for auth. Queue consumers in the central region process events from all regional queues and bulk-insert into the primary.

**Who runs this:** Global SaaS, enterprise with regional requirements.

## Scaling Progression

Each step adds one thing. Nothing gets replaced. Nothing gets rewritten.

```
SQLite (local)  →  PG (scale out)  →  PG + KV (hot path)  →  PG + KV + replicas + queue
                                                               (multi-region)
```

| Component | Local | Scale Out | Dedicated KV | Multi-Region |
|-----------|-------|-----------|--------------|--------------|
| OLTP | SQLite | Postgres | Postgres | Postgres |
| Read Replica | same connection | same PG | PG read replica | PG replica per region |
| KV | in-memory | in-memory or Redis | Redis / dedicated | platform-native |
| Queue | go channel | PG unlogged table | PG table / Redis | SQS / Kafka / dedicated |
| OLAP | same SQLite | same PG | same PG or dedicated | ClickHouse / dedicated |

## Multi-Tenancy

Multi-tenancy is a data partitioning decision, not a separate architecture. It layers on top of any deployment profile.

### Instance and Org Scoping

Zitadel uses a two-level hierarchy (per ADR-021):

- **`instance_id`** — The tenant boundary. Each customer (or self-hosted deployment) is an instance. This is the top-level discriminator for all shared infrastructure.
- **`org_id`** — Organizational scope within an instance. A single customer may have multiple orgs (departments, subsidiaries, environments).

Every piece of data in the system is scoped to an instance. How `instance_id` applies to each primitive:

| Primitive | Shared infra | Dedicated infra |
|-----------|-------------|-----------------|
| **OLTP** | Every table has `instance_id` column. Queries always filter by it. Indexes are prefixed with it. | Tenant has own database. No `instance_id` column needed (implicit). |
| **KV** | Keys prefixed: `{instance_id}:session:{sid}` | Tenant has own KV namespace or own Redis instance. |
| **Queue** | Events tagged with `instance_id`. Consumer can rate-limit per instance. | Tenant has own queue topic or channel. |
| **OLAP** | Events include `instance_id`. Dashboards filter by it. | Tenant has own analytics store, or shared with instance-level partitioning. |
| **Read Replica** | Shared replica with `instance_id` filtering. Or per-instance replica (SQLite file per tenant via Litestream). | Tenant has own replica. |

### Shared vs Dedicated

**Shared infrastructure (most tenants):** All tenants share the same OLTP, KV, queue, and OLAP. The `instance_id` column partitions data. The queue's bulk-insert rate-limiting operates per `instance_id` — a noisy tenant's queue backlog doesn't starve other tenants' ingestion.

**Dedicated infrastructure (enterprise tier):** Tenant gets their own deployment — potentially a single-machine SQLite setup in their own VPC, or a dedicated Postgres with dedicated KV and queue. Same architecture, same four primitives, fully isolated. The `instance_id` is implicit (only one tenant), so it can be omitted from the schema for simplicity.

## OpenFGA and Authorization

OpenFGA fits cleanly into this architecture. Authorization tuples and models are stable data — they're managed centrally and replicated to edges like any other resource.

**Tuples are stable data.** An admin grants a role, creates a group membership, or assigns a permission. These are management API writes that go to the central OLTP. The OpenFGA store (embedded, in-process) reads from the same database.

**FGA checks happen at the edge.** During an auth flow, the edge needs to answer "does this user have access to this resource?" The FGA engine runs against the edge read replica — the same replica that serves user lookups and signing keys. No call to central.

```
Central:
  Admin grants role     → FGA tuple written to OLTP
  OLTP replicates       → tuple appears in edge read replica

Edge:
  Auth request arrives  → FGA check against local replica (sub-ms)
  No network call to central for authorization decisions
```

**OpenFGA storage is just another table in the OLTP.** The embedded OpenFGA uses the same database connection as the rest of the application (SQLite or Postgres). When the OLTP replicates to edge read replicas, FGA tuples come along. No separate replication path needed.

**Level 0 (SQLite):** OpenFGA reads and writes the same SQLite file. FGA checks are local function calls. Zero overhead.

**Level 1+ (Postgres):** OpenFGA uses the same Postgres primary for writes, same read replica at the edge for checks. The FGA storage adapter already supports both SQLite and Postgres.

**Customer-facing FGA (product feature):** If OpenFGA is exposed as a product feature (customers define their own authorization models), the same split applies. Customer-managed tuples are stable data — written via management API to central, replicated to edge. Customer applications check permissions at the edge against the local replica. The queue is not involved — FGA tuples don't flow through transient data paths.

## Central Database is Protected

The central OLTP only receives:

- **Management API writes** — low volume, human-initiated (user creation, config changes)
- **Queue consumer bulk inserts** — controlled rate, batched, asynchronous
- **Fallback reads** — rare, only when edge KV misses a session from another edge

The central OLTP never receives:

- Session creation traffic
- Token issuance traffic
- Auth request handling
- Token validation

This guarantees that auth availability is independent of central database load, maintenance, or temporary outages.

## Interface Surface

The entire data layer is defined by four interfaces. Every deployment picks implementations for each.

```
CentralDB        — SQL database for stable data + ingested transient data
                   Implementations: SQLite, Postgres

EdgeReadDB       — Read-only access to stable data at the edge
                   Implementations: same connection (SQLite), PG read replica

EdgeKV           — Transient data store at the edge
                   Interface: Get(key), Put(key, value, ttl), Delete(key)
                   Implementations: in-memory, Redis, DynamoDB, Workers KV

EdgeSink         — Ships transient events to central for ingestion
                   Interface: Emit(events...)
                   Implementations: go channel, PG table, Redis stream, S3, SQS, Kafka
```

Application code depends only on these interfaces. Deployment configuration selects implementations. The auth flow code is identical across all deployment profiles.

## Relationship to Existing ADRs

**ADR-010** (Three-Tier Data): The OLTP/OLAP split is preserved. This design refines Tier 1 by separating stable and transient write paths, and makes the queue explicit rather than implicit.

**ADR-017** (Caching Tiers): L1 (in-database) and L2 (cache.db) are subsumed by EdgeReadDB (stable replicas) and EdgeKV (transient). L3 (Redis) becomes one EdgeKV implementation. L4 (HTTP/CDN) is orthogonal.

**ADR-026** (Container-per-Tenant with D1): Cloudflare-specific deployment details become one set of implementations for the four interfaces — D1 as OLTP, Durable Objects as KV, outboundByHost as queue. The architecture itself is platform-agnostic.
