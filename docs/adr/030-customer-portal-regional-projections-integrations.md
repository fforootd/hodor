# ADR-030: Customer Portal, Regional Projections, and Control-Plane Integrations

**Status:** Proposed
**Date:** 2026-04-04
**Depends-on:** ADR-034 (Multi-Tenancy via Instance Boundaries), ADR-026 (Cloud Deployment Architecture — Control-Plane Routing and Regional Backends), ADR-029 (Control Plane, Auth Data Plane, and Bounded Eventual Consistency)
**Related:** [Architecture Overview](../architecture/overview.md), [Storage Architecture](../design/storage-architecture.md)

## Context

Zitadel Cloud needs one customer-facing control-plane experience that can:

- show all customer instances in one UI
- manage placement, domains, lifecycle, and configuration across regions
- keep end-user auth traffic out of the central portal path
- keep regional blast radius small
- integrate with external commercial and infrastructure systems such as Stripe, HubSpot, and Google Cloud load balancer / TLS provisioning

The current ADRs already establish:

- `instance_id` as the top-level runtime boundary
- portal-managed `instances`, `instance_domains`, and `cloud_backends`
- regional auth continuity as a deliberate property of the architecture

What is still missing is the operating model for the customer portal itself:

- where the portal stores authoritative control-plane state
- how that state reaches regional runtimes
- which traffic should go through the portal versus directly to instance domains
- how third-party control-plane side effects are coordinated

## Decision

Zitadel Cloud adopts a **central customer portal with regional projections**.

### 1. The portal is the cloud control plane

The portal consists of:

- a customer-facing Portal UI
- a Portal API / backend-for-frontend
- an authoritative control-plane database
- asynchronous projectors and integration workers

The portal owns customer/account-level cloud concerns:

- customer and membership management
- instance lifecycle
- placement and backend binding
- domain ownership and routing
- billing and commercial metadata
- CRM and go-to-market synchronization
- external infrastructure orchestration for domain and TLS lifecycle

The portal is the only writer for cloud control-plane state.

### 2. The control-plane database is authoritative

The central portal database is the source of truth for cloud routing and placement state.

Canonical routing tables remain:

`instances`
- `instance_id`
- `customer_id`
- `state`
- `primary_domain`
- `placement_mode`
- `region_key`
- `backend_key`
- `updated_at`

`instance_domains`
- normalized `domain`
- `instance_id`
- `is_primary`
- `state`
- `updated_at`

`cloud_backends`
- `backend_key`
- `kind`
- `url`
- `secret_ref`
- `region_key`
- `state`
- `global_default`
- `updated_at`

Portal-specific tables may additionally track:

- customer accounts and memberships
- desired instance configuration
- provisioning jobs and integration state
- billing and entitlement state
- CRM/account linkage
- projection version and health by region

### 3. Regional runtimes consume projections, not direct portal writes

The portal does not synchronously fan out writes into every regional backend as part of the customer request path.

Instead:

1. the Portal API writes authoritative state to the portal database
2. a projector or reconciliation worker emits regional updates
3. each target region applies a local projection of the subset it needs
4. the runtime reads the regional projection plus in-process cache

The default first implementation for regional projection is **regional Postgres tables**.

Optional shared KV may be added later as a hot lookup acceleration layer, but KV is not the only source of truth for domain routing.

This means:

- portal DB = authoritative control-plane truth
- regional PG projection = regional runtime source for routing/config reads
- process cache = local acceleration
- shared KV = optional acceleration only

### 4. End-user auth traffic stays direct to instance domains

The customer portal is the single control-plane UI, but it is not the permanent hop for end-user auth traffic.

The traffic split is:

- **Portal/control-plane traffic**: central Portal UI and Portal API
- **Regional admin/runtime traffic**: instance console and management/runtime APIs in the assigned region
- **End-user auth traffic**: direct to the instance domain such as `login.acme.com`

The portal may broker selected management operations into regions for a better UX, but it must not become a mandatory data-plane proxy for all login, session, token, or callback traffic.

### 5. Domain resolution is two-layered

Cloud routing resolves:

`domain -> instance`

Inside the resolved instance, product-level login context may additionally resolve:

`domain/subdomain/path/parameter -> org`

This keeps infrastructure routing and product context separate:

- `instance_id` remains the infrastructure and runtime boundary
- `org_id` stays subordinate to the resolved instance
- optional org-domain mappings must never escape the resolved `instance_id`

### 6. External integrations are control-plane workers

The portal app must interface with external systems as part of cloud operations.

Initial integration families include:

- **Stripe** for subscriptions, entitlements, invoices, plan state, and billing-triggered provisioning gates
- **HubSpot** for customer/account lifecycle, CRM synchronization, lead-to-customer transitions, and operator context
- **Google Cloud load balancing / TLS infrastructure** for domain onboarding, certificate attachment, host rules, and related domain serving lifecycle

These integrations are asynchronous control-plane responsibilities, not auth runtime responsibilities.

The general pattern is:

1. persist the desired state in the portal DB
2. enqueue or emit a provisioning/integration job
3. perform external side effects with retries and auditability
4. record observed state and expose it in the portal UI/API

Examples:

- custom domain added in portal -> verify ownership -> provision LB/TLS resources -> mark domain active -> project to target region
- billing state changes in Stripe -> update entitlements in portal -> gate create/upgrade/suspend flows
- CRM synchronization with HubSpot -> update customer metadata visible to support and operations without coupling it to auth correctness

### 7. Regional Postgres clusters are the default cloud data-plane backend

For the first managed-cloud implementation, a regional Postgres cluster may host many instances dynamically.

Creating a new instance should usually require:

1. creating the instance in the portal DB
2. assigning a `backend_key` / region
3. projecting the route and runtime metadata into the region
4. lazily or asynchronously seeding any instance-local data required by the region

This avoids per-instance cluster bootstrapping while preserving regional isolation and a manageable blast radius.

## Failure Model

This ADR follows ADR-029 and applies it to the portal architecture.

### Control-plane outage

If the central portal or portal DB is unavailable:

- new cloud mutations fail or pause
- billing and provisioning jobs pause
- existing regional runtimes continue from their local projections and runtime stores
- direct end-user auth traffic remains on the instance domains

### Regional outage

If one regional backend fails:

- only instances assigned to that region are directly affected
- the portal remains available for status and operator actions
- other regions remain unaffected

### Projection lag

If a regional projection is stale:

- existing domains and routing may continue with bounded staleness
- destructive or freshness-critical changes such as emergency disable, domain removal, or revocation-sensitive operations should wait for confirmed propagation or fail closed

### Integration failure

If Stripe, HubSpot, or Google Cloud control-plane integration fails:

- desired state remains durable in the portal DB
- retries and operator-visible status drive recovery
- auth runtime correctness must not depend on synchronous success of those external APIs

## Consequences

### Positive

- customers get one cloud portal UI across all their instances
- the control plane has one authoritative source of truth
- regional runtimes stay local and keep their blast radius bounded
- end-user auth does not depend on the portal request path
- external billing, CRM, and infrastructure automation fit naturally into the control plane

### Negative

- the platform now needs projection/reconciliation machinery in addition to the authoritative DB
- operators must reason about central truth versus regional projected state
- portal UI consistency for just-updated state depends on projection observability and status reporting

### Risks

- overusing the Portal API as a proxy for all instance traffic would enlarge the blast radius and increase latency
- stale or partially applied projections could confuse operators unless status is explicit
- external infrastructure automation for custom domains and TLS becomes security-critical operational code
- domain-to-org context must not reintroduce ambiguity about the `instance_id` boundary
