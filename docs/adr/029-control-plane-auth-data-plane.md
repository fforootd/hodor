# ADR-029: Control Plane, Auth Data Plane, and Bounded Eventual Consistency

**Status:** Accepted
**Date:** 2026-04-04
**Depends-on:** ADR-010 (Three-Tier Data Architecture), ADR-017 (Process Cache Semantics), ADR-034 (Multi-Tenancy via Instance Boundaries)
**Related:** [Architecture Overview](../architecture/overview.md), [Storage Architecture](../design/storage-architecture.md)

## Context

POC note: the current runtime defaults have been simplified since this ADR was written. The shipping prototype now uses direct `storage.primary`, `storage.transient`, and `storage.analytics` stores, with optional shared cache and explicit replica reads. The older `kv + sink` continuity model remains future architectural direction rather than default runtime behavior.

The current repository already defines:

- `instance_id` as the runtime boundary
- `storage.kv` and `storage.sink` as the transient auth continuity path
- portal-managed cloud routing and backend selection
- analytics as a separate concern from transactional correctness

What is still implicit is the failure model that ties those pieces together.

In practice, the intended architecture is not:

- one globally central write path that every login must synchronously depend on
- one uniform consistency rule for every identity fact

Instead, the platform separates:

- administrative control-plane mutations
- end-user authentication continuity
- analytics and reporting flows

That separation needs to be explicit so the existing ADRs do not read as if they imply a stronger central dependency than the storage model was designed for.

## Decision

Zitadel standardizes on a three-plane model:

| Plane | Responsibility | Default consistency posture |
|---|---|---|
| **Control plane** | Customer Portal, admin and management writes, routing and placement, provider and policy authoring | authoritative, central or home-region writes |
| **Auth data plane** | Login, session creation, token issuance, token refresh, revocation checks, auth runtime state | regional continuity with bounded eventual consistency where acceptable |
| **Analytics plane** | Audit export, telemetry, reporting, observability drains | asynchronous and replayable |

The default architectural goal is:

- brief control-plane outages are acceptable
- login continuity is more important than immediate admin mutation availability
- the auth data plane may continue regionally during both planned maintenance and unplanned central outages
- regional auth runtime state is reconciled back to the authoritative plane after recovery

## Consistency Classes

The system does not apply one consistency rule to all identity data.

| Consistency class | Typical examples | Default behavior |
|---|---|---|
| **Strong / control-plane authoritative** | user creation, provider config, policy edits, placement changes | writes go to the authoritative plane; if unavailable, the mutation fails |
| **Bounded eventual / auth continuity** | session creation, login runtime state, auth request progress, regional auth projections | regional auth may continue; state lands in `storage.kv` and is reconciled via `storage.sink` |
| **Freshness-critical / priority path** | disable user, logout-all, token or session revocation, factor removal, emergency policy changes | use a priority invalidation path; if freshness cannot be proven within budget, fail closed |

This table is normative for architecture docs and future implementation work.

## Degraded-Mode Defaults

| Operation | Planned maintenance | Unplanned central outage |
|---|---|---|
| **New login** | allowed after control-plane writes are frozen and regional reads are known-good | allowed with bounded stale-data risk through regional read models plus `kv + sink` |
| **Existing session validation** | allowed regionally | allowed regionally |
| **Control-plane mutation** | blocked until the authoritative plane returns | blocked until the authoritative plane returns |
| **Revocation / disable / logout-all** | routed through the priority invalidation path; if freshness budget is not met, fail closed | routed through the priority invalidation path; if freshness budget is not met, fail closed |

The auth data plane may issue new sessions during degraded mode. Those sessions are still part of the identity system's correctness model, but they are allowed to reconcile back to the authoritative plane asynchronously.

## Storage Role Implications

For the current POC:

- `storage.primary.replica` is an explicit stale-read capability, not a separate storage role
- `storage.transient` is the direct auth-runtime authority by default
- `cache.shared` is an accelerator only

For future distributed continuity work:

- `storage.read` is not only a performance optimization; it can be part of the regional auth continuity path.
- `storage.kv` is the writable regional auth-runtime layer for transient state with TTL and consume-once semantics.
- `storage.sink` is the replay and reconciliation boundary from regional auth runtime state back into retained authoritative state.
- `storage.process_cache` is never a distributed correctness mechanism.

This ADR clarifies the intended behavior of existing storage roles. It does not change the storage-role architecture.

## Consequences

### Positive

- The repo has one explicit failure model for control plane vs auth continuity
- Existing ADRs can stay focused without re-explaining degraded-mode semantics independently
- Regional auth continuity is documented as intentional, not accidental

### Negative

- Some identity operations now explicitly have different consistency guarantees
- The implementation must distinguish between ordinary lag-tolerant auth state and freshness-critical invalidations

### Risks

- Loose wording around "login continuity" could be misread as blanket fail-open behavior
- Priority invalidation paths become security-critical infrastructure and need explicit SLOs
