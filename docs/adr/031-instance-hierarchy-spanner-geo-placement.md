# ADR-031: Instance Hierarchy with Geo-Partitioned Placement

**Status:** Proposed
**Date:** 2026-04-04
**Depends-on:** ADR-034 (Multi-Tenancy via Instance Boundaries), ADR-026 (Cloud Deployment Architecture), ADR-029 (Control Plane, Auth Data Plane, and Bounded Eventual Consistency), ADR-030 (Customer Portal, Regional Projections, and Control-Plane Integrations)
**Supersedes:** The separate portal service and regional projection machinery described in ADR-030, while retaining ADR-030's integration and failure model guidance.
**Related:** [Architecture Overview](../architecture/overview.md), [Storage Architecture](../design/storage-architecture.md)

## Context

ADR-030 described a customer portal as a separate control-plane service with its own database, projection workers, and regional synchronization. While the operating model and integration guidance in ADR-030 remain valid, the implementation approach introduces significant operational machinery:

- a separate portal binary with its own auth system
- projection workers to fan out routing state to regional databases
- server-side proxy logic to forward management requests to regional APIs
- separate regional Postgres deployments per region

Revisiting this design reveals two insights:

1. The portal's requirements — user management, org-scoped access control, session handling, login flows — are exactly what Zitadel already provides. Building a separate service for this duplicates the product.

2. With Google Cloud Spanner and geo-partitioned tables, the regional data placement problem is solved by the storage layer, not by application-level projection workers. One database handles global routing metadata and regional data locality through row-level placement policies.

These insights lead to a simpler architecture: Zitadel itself is the portal, instances form a hierarchy with parent-child relationships, and Spanner handles multi-region data placement without projection machinery.

## Decision

### 1. Instances form a parent-child hierarchy

Every instance has an optional `parent_instance_id`. A root instance has no parent. Child instances reference their parent and an `owner_org_id` that identifies the owning organization in the parent instance.

```sql
instances:
  instance_id        TEXT PRIMARY KEY
  parent_instance_id TEXT REFERENCES instances(instance_id)  -- NULL for root
  owner_org_id       TEXT     -- org in the parent instance that owns this child
  state              TEXT     -- active, suspended, deprovisioning
  primary_domain     TEXT
  placement_mode     TEXT     -- global, regional
  region_key         TEXT     -- placement hint (Spanner interprets, Postgres ignores)
  backend_key        TEXT     -- retained for backward compat with ADR-026
  kind               TEXT     -- managed, federated
  created_at         TIMESTAMP
  updated_at         TIMESTAMP
```

The hierarchy is unconstrained in the schema but practically operates at known depths:

| Deployment | Depth | Structure |
|---|---|---|
| Self-hosted single | 1 | One root instance, no children |
| Self-hosted multi-env | 2 | Root → dev, staging, prod |
| Cloud | 3 | Root (staff) → customer-portal (all customers) → customer instances |

### 2. Zitadel is the portal

There is no separate portal service. Instance management is a core Zitadel feature, available when the instance has child instances or when cloud mode is enabled.

A root instance is a standard Zitadel instance that additionally exposes instance lifecycle endpoints:

```
POST   /v1/instances                    -- create child instance
GET    /v1/instances                    -- list children (scoped by caller's org)
GET    /v1/instances/:id                -- child detail
PATCH  /v1/instances/:id                -- update placement, state, config
DELETE /v1/instances/:id                -- deprovision child
POST   /v1/instances/:id/domains        -- add domain to child
DELETE /v1/instances/:id/domains/:domain -- remove domain from child
```

These endpoints follow the same patterns as every other Zitadel resource: dedicated table, scoped by authorization, standard CRUD.

Authorization for instance management uses the existing model. `owner_org_id` on the child instance references an org in the parent. Members of that org can manage the child. Fine-grained roles (create, view, delete, suspend) are org-level roles — the same mechanism customers use for their own resources.

The console displays an "Instances" section when the current instance has children or when `cloud.enabled` is true. No separate SPA is needed.

### 3. Zitadel Cloud uses a three-level hierarchy

```
Root instance (Zitadel staff only)
  ├── Staff users, internal ops
  │
  └── Customer-portal instance (all cloud customers)
        ├── Org "Acme Corp"
        │     owner of: acme-prod (EU), acme-staging (US)
        ├── Org "Beta Inc"
        │     owner of: beta-prod (US)
        └── Org "Gamma Ltd"
              owner of: gamma-onprem (federated, self-hosted)
```

**Root instance:** Only Zitadel staff. Manages the customer-portal instance and operational infrastructure.

**Customer-portal instance:** All cloud customers. Each customer is an org. Customer team members are org members. Cross-customer collaboration (a consultant managing instances for multiple customers) is standard cross-org membership.

**Customer instances:** The actual Zitadel deployments customers use for their products. Each is a child of the customer-portal, owned by a customer org.

Staff and customer credentials never share a database. The root instance is a separate, locked-down deployment. Support access to customer instances uses federated trust (section 5), not shared credentials.

### 4. Spanner with geo-partitioning replaces regional projections

For the managed cloud, Google Cloud Spanner with geo-partitioned tables replaces the projection machinery described in ADR-030.

One Spanner database serves all regions. Data placement is controlled by `region_key`:

- Instance-level data is placed in the instance's assigned region
- Routing metadata (`instances`, `instance_domains`) is globally replicated for low-latency domain resolution everywhere
- Reads are served locally from the nearest Spanner replica
- Writes have Spanner's standard commit latency (cross-region for global tables, local for partitioned tables)

This eliminates:

- Regional projection workers
- Separate regional Postgres databases for cloud
- Projection lag and staleness semantics
- Per-region deployment coordination for schema changes

The Zitadel binary is deployed as stateless compute in each region. Every instance connects to the same Spanner database. Spanner handles data locality.

**What `region_key` means per backend:**

| Backend | `region_key` behavior |
|---|---|
| Spanner | Row-level geo-partitioning and placement policy |
| Postgres | Informational only (no automatic placement) |
| SQLite | Ignored (single-node) |

For Postgres-based multi-region deployments (self-hosted or non-GCP cloud), ADR-030's projection model remains the fallback path. The hierarchy and instance management features work identically — only the data placement mechanism differs.

### 5. Federated trust between parent and child instances

A parent instance can establish a trust link with its child instances. This enables:

- **Support access:** Staff authenticated against the root instance present their token to a child instance. The child validates the token against the root's OIDC discovery endpoint.
- **Cross-instance federation:** A customer authenticated in the customer-portal can access their child instance's management API without re-authenticating.
- **Self-hosted registration:** A self-hosted instance registers with a parent and optionally trusts the parent's token issuer for support or federation.

Trust links use standard OIDC provider federation — the child instance adds the parent's issuer as a trusted identity provider. The child controls what scopes and roles the parent's tokens are allowed to carry. No custom protocol is needed.

For managed children, the trust link is established automatically during instance creation. For federated (self-hosted) children, the customer configures the trust explicitly.

### 6. Federated instances for self-hosted registration

Customers running self-hosted Zitadel can register their instance as a federated child of the cloud hierarchy:

```sql
instances:
  instance_id: gamma-onprem
  parent_instance_id: customer-portal
  owner_org_id: gamma-org
  kind: federated
  registration_token: (used once during registration)
  last_heartbeat: 2026-04-04T10:00:00Z
```

A federated instance is not managed by the parent — the parent cannot create, destroy, or modify it. The parent can:

- Track its existence and health (via heartbeat)
- Offer a unified view across managed and self-hosted instances
- Federate identity via the trust link (if the customer opts in)
- Aggregate metrics for billing or support

The federated instance stays fully autonomous. The customer controls their own data, deployment, and configuration.

### 7. Instance management is a product feature, not a cloud-only concern

Instance management is part of the Zitadel product, not a cloud-specific bolt-on. Any Zitadel deployment can have child instances:

- An enterprise manages dev/staging/prod as children of their root
- A SaaS vendor gives each of their own customers a dedicated Zitadel instance
- A consultancy manages instances for multiple clients from one root

The console shows instance management when the current instance has children. The API endpoints are available to any instance. The cloud offering is "we run the root for you" — not a separate product.

### 8. External integrations remain control-plane workers

ADR-030's guidance on external integrations remains valid. Stripe, HubSpot, and Google Cloud load balancer / TLS integrations are asynchronous control-plane responsibilities attached to the root or customer-portal instance.

These integrations are implemented as background workers or jobs within the Zitadel binary, not as separate services. They follow the pattern from ADR-030 section 6:

1. Persist desired state in the database
2. Enqueue a provisioning or integration job
3. Perform external side effects with retries and auditability
4. Record observed state and expose it in the UI/API

The integration workers run alongside the Zitadel binary that hosts the root or customer-portal instance. They are not part of the auth runtime and do not affect regional child instances.

### 9. Per-user regional placement as a future extension

The `region_key` concept can extend from instances to individual resources:

```sql
users:
  user_id     TEXT
  instance_id TEXT
  region_key  TEXT  -- NULL = inherit from instance
```

A customer's instance is in EU, but a specific user's data must stay in Singapore due to local data residency requirements. With Spanner, this is a row-level placement policy change — no application code needed.

This is a future extension. The initial implementation places data at the instance level only.

## Failure Model

### Root or customer-portal instance outage

- Instance management and portal operations are unavailable
- Existing child instances continue to serve auth traffic independently
- Trust-linked support access continues if the child has cached the parent's JWKS
- External integration jobs pause until the control-plane instance recovers

### Regional Spanner partition outage

- Instances assigned to the affected region are unavailable
- Instances in other regions are unaffected
- Routing metadata (globally replicated) remains available everywhere
- The portal can show status but cannot proxy management operations to the affected region

### Federated instance unreachable

- The parent marks the instance as unhealthy after missed heartbeats
- The federated instance continues to operate independently
- Trust links continue if the parent's JWKS is cached

## Consequences

### Positive

- No separate portal service — Zitadel is the portal
- No projection workers or regional synchronization code
- Instance management is a product feature usable by all deployment types
- Spanner handles multi-region data placement without application-level machinery
- Self-hosted and cloud use the same instance hierarchy feature
- Staff and customer credentials are cleanly separated by instance boundary
- Cross-customer collaboration works through existing org membership

### Negative

- Spanner is Google Cloud only — Postgres multi-region still needs the ADR-030 projection fallback
- Three-level hierarchy in cloud adds conceptual complexity for operators
- Trust links between instances require careful scoping to prevent privilege escalation
- The root instance becomes a critical single point for cloud operations

### Risks

- `owner_org_id` scoping on instance management endpoints is the security boundary — a missed scope check exposes cross-customer data
- Federated instances are not controlled by the parent — a malicious self-hosted instance could claim false health status
- Spanner's pricing model (per-node, per-operation) may require careful capacity planning for high-instance-count deployments
- Per-user regional placement increases query complexity and may require careful index design
