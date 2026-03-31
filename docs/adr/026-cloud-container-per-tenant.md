# ADR-026: Cloud Deployment Architecture — Container-per-Tenant with D1

**Status:** Proposed
**Date:** 2026-03-31
**Depends-on:** ADR-021 (Multi-Tenancy via Instance Isolation)
**Supersedes:** ADR-021 — for cloud deployment topology only. Self-hosted retains the shared-database model.

## Context

ADR-021 adopted shared-database row-level discrimination (`instance_id` on every table) for multi-tenancy. This works for self-hosted single-binary deployments where a single database is simpler to operate.

For Zitadel Cloud on Cloudflare, we want stronger isolation guarantees: each customer gets their own container process, own database, own encryption keys, own cookie secrets. The goal is to keep the Go binary completely single-tenant while letting the Cloudflare Worker layer handle multi-tenancy, routing, and configuration.

Cloudflare's infrastructure provides the building blocks:
- **Containers** run per-tenant Go binaries with per-instance env vars
- **Durable Objects** provide strongly consistent per-instance config storage
- **D1** provides managed SQLite databases provisionable via REST API
- **outboundByHost** lets the Worker intercept container HTTP calls and bridge them to D1

## Decision

### Container-per-tenant with D1 as default database

Each customer instance gets:
- Its own Durable Object (managing container lifecycle + config)
- Its own D1 database (provisioned automatically)
- Its own container process (scale-to-zero when idle)
- Its own encryption keys, cookie secrets, admin credentials

The Go binary sees `ZITADEL_DATABASE_URL=d1://d1.local` and connects via the existing d1driver. The Worker's `outboundByHost` intercepts these HTTP calls and bridges them to the correct D1 database using the REST API.

### Architecture

```
Request (acme.zitadel.cloud)
  -> Worker fetch()
  -> ZitadelRouter DO (singleton)
     - domain -> instance lookup from DO SQLite
  -> ZitadelContainer DO (per-instance)
     - loads config from DO SQLite
     - sets container envVars
     - outboundByHost bridges d1.local -> D1 REST API
  -> Container (Go binary)
     - reads env vars, connects to d1://d1.local
     - d1driver sends HTTP to d1.local
     - outboundByHost intercepts and queries tenant's D1
```

### Two Durable Object classes

**ZitadelRouter** (singleton): Lightweight domain-to-instance routing table. Queried on every request to resolve which container handles this hostname.

**ZitadelContainer** (per-instance): Full instance configuration in a key-value table. Reads config at container boot, builds env vars, manages the D1 bridge.

### Database strategy

| Tier | Database | Provisioning | Data residency |
|------|----------|-------------|----------------|
| Free / Standard | Cloudflare D1 | Automatic via REST API | Cloudflare-managed |
| Enterprise BYODB | Customer's Turso or Postgres | Customer provides URL + token | Customer-managed |

For D1 tenants, the container URL is `d1://d1.local` and the outboundByHost bridge routes queries to the tenant's D1 database ID via Cloudflare's REST API.

For BYODB tenants, the container gets `ZITADEL_DATABASE_URL=libsql://...` or `postgres://...` directly. The outbound bridge is never hit.

Migration from D1 to BYODB: D1 export API produces a SQL dump, customer imports into their database, config is updated to point to the new URL.

### Three database connections per instance

The Go binary opens three connections at startup. All use the same D1 database (via the bridge), except the analytics cache:

| Connection | Target | Purpose |
|-----------|--------|---------|
| Main app | D1 (via bridge) | Identity, schemas, sessions, tokens, providers, events |
| OpenFGA | Same D1 (separate sql.DB handle) | Authorization tuples and models. Separate handle avoids SQLite write-lock contention. |
| Analytics cache | Local SQLite (`/data/zitadel-cache.db`) | Ephemeral ring buffer. Drains into D1 events table. Container disk is ephemeral — cache loss on restart is acceptable. |

### Version pinning (Phase 2)

Cloudflare Containers do not support per-instance image selection. All containers in one Worker deployment share the same image. Per-tenant version pinning requires:

1. Multiple Worker deployments (one per active Zitadel version)
2. Router Worker uses Service Bindings to dispatch to the correct versioned Worker
3. Instance config includes `version_channel` (stable, canary, or pinned semver)

Phase 1 ignores version pinning — all tenants run the same version.

## DO SQLite Schema

### ZitadelRouter (singleton)

```sql
CREATE TABLE domains (
    domain       TEXT PRIMARY KEY,
    instance_id  TEXT NOT NULL,
    customer_id  TEXT NOT NULL,
    is_primary   INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_domains_instance ON domains(instance_id);
CREATE INDEX idx_domains_customer ON domains(customer_id);
```

### ZitadelContainer (per-instance)

```sql
CREATE TABLE config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE instance_domains (
    domain     TEXT PRIMARY KEY,
    is_primary INTEGER NOT NULL DEFAULT 0
);
```

Config keys: `customer_id`, `name`, `database_type`, `d1_database_id`, `database_url`, `database_token`, `admin_email`, `admin_password`, `admin_pat`, `cookie_secrets`, `encryption_keys`, `encryption_key_id`, `migrate`, `bootstrap`, `log_level`, `version_channel`, `state`.

Key-value design avoids schema migrations when adding new config fields.

## D1 Bridge Protocol

The existing Go d1driver (`internal/database/d1driver/`) sends:

```
POST http://d1.local/query   -> {"sql": "...", "params": [...]}
POST http://d1.local/exec    -> {"sql": "...", "params": [...]}
```

The Worker's outboundByHost handler translates this to:

```
POST https://api.cloudflare.com/client/v4/accounts/{acct}/d1/database/{db}/query
Authorization: Bearer {token}
Body: {"sql": "...", "params": [...]}
```

Response translation: unwrap `result[0]`, extract column names from first result row for `meta.columns` (which the d1driver requires for consistent column ordering).

## Consequences

### Positive

- True process-level isolation per tenant (no `instance_id` filtering, no cross-tenant data leak risk)
- Per-tenant scale-to-zero (idle tenants cost nothing)
- BYODB unlocks enterprise data residency without any Go code changes
- D1 free tier (5M reads/day, 100K writes/day) covers small tenants at zero cost
- The Go binary is unchanged — same code runs locally on SQLite, self-hosted on Postgres, or in cloud on D1

### Negative

- D1 REST API bridge adds ~10-30ms latency vs direct binding (~1-5ms)
- One D1 database per tenant requires automated provisioning and lifecycle management
- Version pinning requires multiple Worker deployments (Phase 2)
- Analytics events drain through D1 before R2+Iceberg is ready (Phase 4)

### Risks

- D1 REST API rate limits could throttle high-traffic tenants (mitigate: monitor, upgrade to direct binding when available)
- outboundByHost is relatively new in @cloudflare/containers (mitigate: fallback to Turso if bridge fails)
- Many D1 databases per account may hit Cloudflare plan limits (mitigate: monitor, contact Cloudflare for enterprise limits)
