# ADR-037: Platform FGA Store Separation

**Status**: Proposed  
**Date**: 2026-04-06  
**Depends on**: ADR-020 (Authorization Model), ADR-031 (Instance Hierarchy), ADR-034 (Multi-Tenancy)

## Context

An IAM platform has an inherent meta-problem: it needs authorization to manage the
system that provides authorization. Zitadel has two distinct authorization concerns:

1. **Platform authorization** — who can manage what in Zitadel itself: instance
   hierarchy, org-to-instance ownership, staff roles like `system_owner`.
2. **Customer authorization** — the product Zitadel sells: per-instance FGA stores
   with core artifacts (user, org, group, project, app, session, settings) plus
   whatever custom types the customer defines.

Prior to this ADR, both concerns were served by the same store. The root instance's
customer FGA store doubled as the platform authority store. `reconcile_root_hierarchy`
wrote platform hierarchy tuples (parent/child relationships, org ownership) into the
root instance's FGA store, and `root_relation_allowed` checked that same store for
platform access decisions.

This conflation has concrete problems:

- If a root-instance operator adds custom FGA types or tuples, they coexist with
  platform structural facts in the same store.
- The root instance's customer-facing FGA admin API (`/v1/fga/*`) exposes platform
  hierarchy tuples as if they were normal customer data.
- The platform authorization model cannot evolve independently from the customer-facing
  core model.
- The conceptual boundary between "Zitadel's own authz" and "authz as a product" is
  implicit rather than structural.

## Decision

### Two Stores, Clean Boundary

Introduce a dedicated **platform FGA store** for all platform authorization decisions.
Keep **per-instance customer stores** unchanged. No cross-store references, no
namespace-qualified IDs, no cross-store resolution coordinators.

```
┌─────────────────────────────────┐
│  Platform Store (one, global)   │
│  ─────────────────────────────  │
│  instance:root  parent  inst:A  │
│  org:acme#owner  owner  inst:A  │
│  user:staff1  owner  org:acme   │
│                                 │
│  ID: _platform                  │
│  Used by: platform middleware   │
│  Owned by: Zitadel operators    │
│  Customer-visible: never        │
└─────────────────────────────────┘

┌────────────────────────────────┐  ┌────────────────────────────────┐
│  Root Instance Store           │  │  Customer Instance Store       │
│  ────────────────────────────  │  │  ────────────────────────────  │
│  Core: user,org,group,project  │  │  Core: user,org,group,project  │
│  Custom: operator additions    │  │  Custom: customer additions    │
│                                │  │                                │
│  A normal customer store.      │  │  Used by: customer's own apps  │
│  No platform tuples.           │  │  No platform tuples.           │
└────────────────────────────────┘  └────────────────────────────────┘
```

### Platform Store Identity

The platform store uses `PLATFORM_STORE_ID = "_platform"` as its identifier in the
`fga_instance_stores` table. The underscore prefix prevents collision with instance IDs
(which are UUIDs or `"default"`). No schema migration is needed — `fga_instance_stores`
accepts any string as `instance_id`.

### Platform Store Model

The platform store reuses `core_authorization_model()` — the same model that ships
with every customer store. Types unused by the platform (project, app, settings,
session) are inert: no tuples are written for them and they have zero evaluation cost.

This avoids maintaining two separate model definitions and two upgrade codepaths. If
the platform model needs to diverge in the future (e.g., adding `region` or `fleet`
types), it can be factored out then.

### Reconciliation

`reconcile_platform_hierarchy` (renamed from `reconcile_root_hierarchy`) continues to
derive the desired tuple set from database state (org memberships, instance ownership).
The only change: tuples are written to the platform store instead of the root instance's
customer store.

On upgrade from the pre-separation model, the first boot reconciliation populates the
platform store from DB state. No SQL data migration is needed — the tuples are
computed, not authored.

### What Lives Where

| Data | Store | Rationale |
|------|-------|-----------|
| Instance parent/child relationships | Platform | Structural platform facts |
| Org-to-instance ownership | Platform | Platform access control |
| Org role memberships (for platform access) | Platform | Who can manage which instances |
| Customer users, orgs, groups, projects, apps | Customer (per-instance) | Product data |
| Customer custom FGA types and tuples | Customer (per-instance) | Customer's own authz model |
| Support grants (ADR-036) | Platform | Staff access to child instances |

### Invariants

1. **No cross-store references.** The platform store and customer stores are
   independent graphs. A platform check never queries a customer store. A customer
   check never queries the platform store.
2. **No tuple replication.** Platform tuples are never materialized into customer
   stores. Customer tuples are never copied to the platform store.
3. **Platform routes fail closed.** Platform-managed routes (instance management,
   `/v1/instances/{id}/...`) must authorize through the platform store only and must
   not fall through to a customer store.
4. **Customer stores are opaque to the platform.** The FGA admin API (`/v1/fga/*`)
   operates on the current instance's customer store. It never exposes or mutates
   platform store data.
5. **Operator admin bypass is unchanged.** `operator_admin` continues to bypass all
   FGA checks (both platform and customer) as a break-glass mechanism.

## Consequences

### Positive

- Customer model customization on any instance (including root) cannot affect platform
  authorization decisions.
- Platform model can evolve independently — adding platform-only types in the future
  requires no customer store changes.
- The root instance becomes a normal customer instance with its own clean store.
- Platform hierarchy tuples are invisible to the customer FGA admin API.
- Cleaner conceptual model: "authz for the IAM" vs "authz as a product."

### Negative

- One additional store to initialize at startup (negligible cost).
- Method rename across the codebase (`root_*` → `platform_*`).

### What This Decision Does NOT Cover

- **Cross-store workload resolution.** If a future use case requires customer workload
  policies to reference platform-managed principals (e.g., "members of platform group X
  can access customer resource Y"), that would require a cross-store coordinator. This
  is deferred — no current use case requires it.
- **Multi-level hierarchy flattening.** The reconciliation function still processes one
  level of children per call. Centralizing the full hierarchy tree into the platform
  store is a future optimization.
- **Namespace-qualified IDs.** IDs are UUIDs and do not collide across stores. No ID
  format change is needed.
