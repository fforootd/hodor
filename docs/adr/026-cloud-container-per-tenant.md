# ADR-026: Cloud Deployment Architecture — Control-Plane Routing and Regional Backends

**Status:** Proposed
**Date:** 2026-04-04
**Depends-on:** ADR-034 (Multi-Tenancy via Instance Boundaries)
**Supersedes:** The earlier container-per-tenant D1 proposal in this file.

## Context

Zitadel Cloud needs:
- many customer instances in shared infrastructure
- instance-level routing owned by the Customer Portal
- optional regional placement for data residency
- backend selection that stays independent from the request-routing contract
- a model that works whether the managed backend is shared-schema SQL or a regional primary/replica topology

The container-per-tenant + D1 proposal optimized for process isolation, but it makes fleet-wide schema evolution and control-plane operations much harder at cloud scale.

## Decision

We adopt **portal-managed control-plane routing plus regional backend selection** for Zitadel Cloud.

This ADR is intentionally backend-neutral. It defines:

- how instances are resolved
- how placement chooses a backend via `backend_key`
- how the cloud control plane owns routing and placement state

It does not choose Spanner, Postgres, or AlloyDB as the only valid managed-cloud interpretation.

### Canonical cloud routing data

Authoritative tables:

`instances`
- `instance_id`
- `customer_id`
- `state`
- `primary_domain`
- `placement_mode` with values `global | regional`
- `region_key` nullable for `global`
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

The Customer Portal is the only writer for these tables. The runtime is read-only.

### Runtime routing flow

For each request:
1. Self-hosted mode returns the configured/default local instance immediately
2. Trusted `X-Zitadel-Instance` may override only from configured trusted proxies
3. Otherwise `Host` resolves through `instance_domains`
4. The runtime caches the routing result in-process with positive and negative TTLs

The resolved request context includes:
- `instance_id`
- `customer_id`
- `placement_mode`
- `region_key`
- `backend_key`
- `host`
- `source`

### Backend registry and secrets

`backend_key` is the binding from an instance to a row in `cloud_backends`.

The binary uses a small bootstrap config, `cloud.control_plane`, to reach the control-plane database. Backend and region metadata are then read from `cloud_backends` at runtime instead of being statically configured in TOML.

### Placement model

`placement_mode = global`
- uses the default/global cloud backend defined by the control plane and runtime config

`placement_mode = regional`
- pins the instance to one customer-selected region
- `region_key` identifies the placement region
- `backend_key` identifies the regional backend binding

v1 does not support one instance writing to multiple regional backends at the same time.

## Failure Model Alignment

Cloud routing and backend selection are control-plane concerns. Login continuity is an auth data-plane concern.

During planned maintenance or unplanned central outages:

- request routing still resolves to `instance_id`
- regional auth continuity may continue from replicated reads plus `storage.kv` and `storage.sink`
- control-plane mutations are allowed to pause while auth continues
- freshness-critical invalidations still require a stricter path and fail closed if freshness cannot be proven

ADR-029 is the canonical source for those degraded-mode semantics.

## Shared-Schema Notes

If a shared-schema backend is used, cloud tenant-scoped tables use `instance_id` as the leading scoping key for tenant data and indexes.

This keeps cloud schema updates to:
- once per backend
- not once per instance

`org_id` remains subordinate to `instance_id`.

## Backend-Specific Migration Policy

Migration semantics belong to the selected managed backend. This ADR does not assume Postgres or Spanner DDL behavior.

Backend-specific deployment docs may require:

- additive expand/contract migrations only
- backend-specific DDL runners
- compatibility across adjacent application and schema versions

Cross-backend moves are explicit control-plane migrations, not transparent request routing.

## Implementation Notes

Current implementation work in the prototype includes:
- control-plane `instances` and `instance_domains` tables
- control-plane `cloud_backends` table
- request-scoped instance resolution before auth/session middleware
- in-process route caching with trusted proxy support
- placement and backend metadata on the resolved instance context

Still planned:
- multiple live backend pools selected by `backend_key`
- backend-specific transport or driver integration
- dedicated migration runners where required by the selected backend

## Consequences

### Positive
- One routing and placement contract regardless of the managed backend behind `backend_key`
- Regional placement fits naturally into the routing model
- Customer Portal owns cloud placement and routing state
- Self-hosted stays simple while cloud gains richer routing behavior

### Negative
- Shared-schema topologies still depend on correct `instance_id` scoping everywhere
- Premium hard-isolation offerings require explicit migration paths later
- Regional backend operations become part of the cloud control plane

### Risks
- Missing `instance_id` filters remain the primary safety risk in shared-schema topologies
- Poor backend-key hygiene could couple control-plane mistakes to live traffic
- Backend-specific rollout discipline still matters, but is intentionally outside this ADR's routing contract
