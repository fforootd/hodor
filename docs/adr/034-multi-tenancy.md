# ADR-034: Multi-Tenancy via Instance Boundaries

> *Renumbered from ADR-021 to resolve duplicate numbering.*

**Status:** Accepted
**Date:** 2026-04-04
**Depends-on:** ADR-020 (Authorization Model), ADR-022 (Dedicated Resource Tables)
**Related:** ADR-029 (Control Plane, Auth Data Plane, and Bounded Eventual Consistency)

## Context

The prototype drifted into treating organizations as the infrastructure tenant, while upstream ZITADEL terminology and earlier product thinking use an **instance** as the top-level environment that contains one or more organizations.

We need one model that works across:
- SQLite for low-friction single-instance self-hosting
- Postgres for enterprise self-hosting
- Shared cloud multitenancy without turning every organization into its own infrastructure unit

## Decision

We standardize on **instance-first isolation**:

1. `instance_id` is the top-level runtime and storage boundary.
2. `org_id` is always a resource inside an instance, never the host-routing key.
3. `customer_id` is a cloud control-plane/account concept only. One customer can own multiple instances.
4. Request routing resolves to `instance_id` before auth/session middleware runs.

## Backend Matrix

| Operating mode | Backend | Instance shape |
|---|---|---|
| Small self-hosted | SQLite | One instance per deployment |
| Enterprise self-hosted | Postgres | One instance per deployment |
| Zitadel Cloud | Managed cloud backend selected by `backend_key` | Many instances routed by the control plane |

Self-hosted deployments default to one local instance and should not force operators to understand the cloud routing model.

## Storage and Query Model

All tenant-scoped cloud tables use `instance_id` as the primary scoping key. In shared-schema cloud backends, `instance_id` should be the leading component in primary keys and secondary indexes for tenant-scoped tables.

Examples of tenant-scoped resources:
- `users`, `orgs`, `apps`, `providers`, `schemas`
- `sessions`, `tokens`, `settings`, `events`
- `groups`, `projects`, `login_flows`

## Routing Model

Runtime resolution order:
1. Self-hosted single-instance mode returns the configured/default local `instance_id`
2. Trusted `X-Zitadel-Instance` header may override when the request came through configured trusted proxies
3. Cloud host lookup resolves `Host -> instance_id`
4. Unknown cloud host is rejected

The resolved instance context is request-scoped and available before auth/session middleware executes.

ADR-029 defines the degraded-mode behavior once an instance has been resolved. This ADR only defines the routing and isolation boundary.

## Consequences

### Positive
- Terminology matches upstream ZITADEL: instances contain organizations
- Self-hosted deployments stay simple
- Cloud can share infrastructure while preserving a single runtime boundary
- Query helpers and middleware can enforce one canonical isolation key

### Negative
- Shared-schema cloud storage still requires every tenant-scoped query to be instance-aware
- Schema changes hit all cloud instances in a backend together
- Premium hard-isolation offerings require explicit migration to a different backend strategy later

### Risks
- Missed `instance_id` scoping in request paths can leak data across instances
- Shared backends still carry noisy-neighbor risk without rate limits and capacity controls
- Terminology drift can reappear unless docs and APIs consistently keep orgs nested inside instances
