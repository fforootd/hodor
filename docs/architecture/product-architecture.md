# Product Architecture — One Binary, Every Scale

> This document is the canonical description of how Zitadel deploys and scales. It synthesizes decisions from multiple ADRs into a single narrative. For deep dives, follow the links to the authoritative ADR or design doc.

## The Core Invariant

Zitadel ships as **one Rust binary** that contains all code — including the cloud-exclusive features (billing, support portal, staff admin, GCP integrations). There are no separate binaries, no plugins, no feature-flag compile targets.

Cloud features are **runtime-gated** via configuration, not compile-time conditionals. The same binary that a developer downloads for a laptop prototype is the same binary that runs Zitadel Cloud at scale. Most users will never see the cloud features — they are hidden unless `cloud.enabled = true` in the config.

**Why one binary?**

- Simplest possible distribution: download, run, done
- No version skew between "community" and "enterprise" builds
- Cloud-specific code paths get tested in every CI run
- Self-hosted operators benefit from hardening driven by cloud-scale workloads

See [ADR-031 section 7](../adr/031-instance-hierarchy-spanner-geo-placement.md) for the rationale on instance management as a product feature, not a cloud bolt-on.

## Deployment Tiers

The same binary scales from a laptop to a planet-scale managed service. What changes is the storage backend and the operational topology.

### Level 0 — Local / Dev / Homelab

```bash
zitadel start
# → SQLite at ./data/zitadel.db (auto-created)
# → Running on http://localhost:8080
```

| Aspect | Detail |
|---|---|
| **Storage** | SQLite (WAL mode, single file) |
| **Instances** | Single root instance |
| **External deps** | None |
| **Use cases** | Local dev, CI tests, homelab, edge appliance, small SaaS |

Everything runs in one process. No Postgres, no Redis, no Docker. Database auto-migrates on startup. Bootstrap creates the default org and admin user.

### Level 1 — Small Production

```toml
[storage.primary]
url = "postgres://zitadel:secret@db:5432/zitadel"
```

| Aspect | Detail |
|---|---|
| **Storage** | Postgres (single instance or managed like RDS/Cloud SQL) |
| **Instances** | Single root instance, or root + a few child instances |
| **External deps** | Postgres |
| **Use cases** | Startup in production, small enterprise, multi-env (dev/staging/prod as child instances) |

Same binary, different connection string. The public model stays `primary`, `transient`, and `analytics`, with optional explicit replica reads. Migrations run as a separate `zitadel db migrate` step in production (controlled via `storage.primary.migrate = "check"`).

### Level 2 — Enterprise

```toml
[storage.primary]
url = "postgres://..."

[storage.primary.replica]
enabled = true
mode = "explicit"
# url = "postgres://readonly@pg-replica:5432/zitadel"
```

| Aspect | Detail |
|---|---|
| **Storage** | Postgres, optionally with replica reads and separate transient / analytics DBs |
| **Instances** | Root + child instances for tenants or environments |
| **External deps** | Postgres |
| **Use cases** | High-scale self-hosted, multi-node deployments, SaaS vendors giving each customer a dedicated instance |

Postgres remains the authority. Replica reads are opt-in for stale-tolerant queries, and a separate transient DB can hold auth-runtime state directly when operators want that split.

See [Storage Architecture](../design/storage-architecture.md) for the full current model.

### Level 3 — Zitadel Cloud

```toml
[storage.primary]
url = "spanner://projects/zitadel-cloud/instances/global/databases/zitadel"

[cloud]
enabled = true
```

| Aspect | Detail |
|---|---|
| **Storage** | Google Cloud Spanner with geo-partitioned tables |
| **Instances** | Three-level hierarchy: root (staff) → customer-portal → customer instances |
| **External deps** | Spanner, GCP load balancer, Stripe, HubSpot |
| **Use cases** | Zitadel-managed cloud service |

Spanner replaces both Postgres and the regional projection machinery. Data placement is controlled by `region_key` on each instance — Spanner handles locality automatically. The Zitadel binary is deployed as stateless compute in each region, all connecting to the same Spanner database.

See [ADR-031](../adr/031-instance-hierarchy-spanner-geo-placement.md) for the full Spanner placement model.

## Instance Model

Every Zitadel deployment has at least one **instance** — the root instance. Instances are the top-level isolation boundary: each instance has its own users, orgs, settings, and data.

### Root Instance

The root instance is the operator's own instance. It is authorized through the internal platform authorization model like every other instance-management path. Root staff get explicit platform roles and hierarchy relationships in the deployment-scoped `platform` FGA store; `operator_admin` remains the only break-glass bypass and is intentionally outside normal FGA relations.

### Child Instances

Any instance can have child instances. This is a product feature, not a cloud-only concern:

| Deployment | Hierarchy | Example |
|---|---|---|
| Single-instance | Root only | `zitadel start` on a laptop |
| Multi-environment | Root → children | Enterprise with dev, staging, prod |
| SaaS vendor | Root → customer instances | Each customer gets a dedicated Zitadel instance |
| Zitadel Cloud | Root → portal → customer instances | Three-level hierarchy (see below) |

Instance management endpoints (`/v1/instances`) are standard CRUD, available to any instance that has children. The console shows an "Instances" section when relevant.

See [ADR-034](../adr/034-multi-tenancy.md) for instance boundaries and [ADR-031 sections 1-3](../adr/031-instance-hierarchy-spanner-geo-placement.md) for the hierarchy model.

### Cloud Hierarchy

```
Root instance (Zitadel staff only)
  └── Customer-portal instance (all cloud customers)
        ├── Org "Acme Corp" → owns: acme-prod (EU), acme-staging (US)
        ├── Org "Beta Inc" → owns: beta-prod (US)
        └── Org "Gamma Ltd" → owns: gamma-onprem (federated, self-hosted)
```

Staff and customer credentials never share a database. The root instance is a separate, locked-down deployment. Support access uses federated OIDC trust — staff tokens from the root instance are validated by child instances via standard provider federation.

## Cloud Features (`zitadel-cloud` crate)

Cloud features live in the dedicated `zitadel-cloud` crate. The code is in the public repository under AGPL-3.0 (source available, auditable by anyone) but requires a **valid license key** to activate at runtime.

```toml
[cloud]
enabled = true
license_key = "eyJ..."   # JWT issued by Zitadel
```

The license key is a signed JWT encoding the licensee's entitlements — which features are enabled, how many managed instances are allowed, and when the license expires. This allows feature-gating per customer without compile-time splits.

| Feature | Integration | Purpose |
|---|---|---|
| **Billing** | Stripe | Subscription management, usage metering, invoicing |
| **Support portal** | HubSpot | Customer support tickets, contact management |
| **Staff admin** | Root instance | Internal ops, customer instance management, incident response |
| **Load balancer** | GCP Cloud Load Balancing | TLS termination, domain routing to regional backends |
| **Instance placement** | Spanner geo-partitioning | Automatic data locality per instance `region_key` |
| **Federated registration** | Heartbeat + OIDC trust | Self-hosted instances register with the cloud hierarchy |
| **Usage metering** | Internal | Per-instance request counts, active users, billable metrics |

These integrations follow the pattern from [ADR-030 section 6](../adr/030-customer-portal-regional-projections-integrations.md): persist desired state → enqueue job → perform side effects with retries → record observed state.

Self-hosted operators never encounter these features. Without a license key, cloud code paths are inert — no config means no activation, no overhead.

## Three-Plane Model

The runtime separates concerns into three planes with different consistency and availability requirements:

| Plane | Responsibility | Availability priority |
|---|---|---|
| **Control plane** | Admin writes, policy authoring, instance management, routing | Brief outages acceptable |
| **Auth data plane** | Login, sessions, token issuance, revocation checks | Must stay up — regional continuity during control-plane outages |
| **Analytics plane** | Audit export, telemetry, observability drains | Asynchronous and replayable |

The key design goal: **login never stops because an admin operation is unavailable.** Regional auth runtime continues independently during control-plane maintenance or outages.

See [ADR-029](../adr/029-control-plane-auth-data-plane.md) for the full consistency model and failure semantics.

## Storage Topology (Summary)

The binary exposes three operator-facing stores plus optional accelerators:

| Store | Purpose | Default |
|---|---|---|
| `primary` | Durable read/write authority | SQLite, Postgres, or Spanner |
| `transient` | Auth-runtime authority | inherits `primary` |
| `analytics` | Analytics and observability target | inherits `primary` |
| `cache.shared` | Safe metadata acceleration | disabled |
| `primary.replica` | Explicit stale reads | disabled |

At Level 0, all three stores collapse into one SQLite file. At larger levels, operators can split transient and analytics DBs or add Postgres replicas without changing the application code.

See [Storage Architecture](../design/storage-architecture.md) for the full model and [Storage Implementation Status](../design/storage-implementation-status.md) for what is implemented today.
