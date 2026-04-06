# ADR-037: FGA Store Split — Platform Store + Customer Stores

**Status:** Proposed
**Date:** 2026-04-05
**Supersedes:** ADR-020 (Authorization Model) section "One Store. One Graph."
**Depends-on:** ADR-020 (Authorization Model), ADR-031 (Instance Hierarchy), ADR-034 (Multi-Tenancy)
**Related:** ADR-036 (Staff Access and Support Grants), ADR-029 (Control Plane, Auth Data Plane)

## Context

ADR-020 established "one FGA store per instance" with a single graph containing
both platform authorization (org membership, instance hierarchy, IAM roles) and
customer-defined authorization (custom types, application-level ReBAC).

In practice this created three problems:

### 1. Cross-instance permission propagation is fragile

When a root user needs to manage a child instance, their permissions must exist
in the child's FGA store. Today `reconcile_root_hierarchy` projects org
membership tuples from the root store into every child store. This projection:

- Must run after every membership change or the child stores go stale.
- Scales linearly with the number of child instances (N reconciliations).
- Creates a window where the child store has stale tuples between mutations and
  the next reconciliation pass.
- Is not transactional — the membership write and tuple projection are separate
  operations with no atomicity guarantee.

### 2. Platform and customer authorization are entangled

A customer writing custom FGA types and tuples via `/v1/fga/stores/{id}/*`
operates in the same store where platform IAM tuples live. This means:

- A customer model change (write a new authorization model) triggers recompilation
  of the merged `core.fga + custom.fga` model, risking platform regressions.
- Debugging platform permission failures requires filtering customer tuples
  out of the same graph.
- Sealed core types prevent name collisions, but the shared evaluation context
  means customer tuples participate in platform checks (and vice versa) unless
  carefully scoped by type name.

### 3. FGA enforcement exposed the gap

When FGA was switched from audit-only to enforced mode, every use-case needed
authorization checks. The natural check objects for platform operations are
`instance:{id}` (e.g., "can this user create schemas in this instance?"), but
instance-level tuples only exist in the root store — child stores only have
projected IAM role tuples on the instance object. This forced all use-case
checks to use `org:{id}` objects instead, which is semantically wrong for
instance-scoped resources like schemas, settings, and providers.

## Decision

Split into two classes of FGA stores with distinct purposes and lifecycles.

### Platform Store (one per deployment)

A single store shared by all instances in the deployment. Contains all tuples
that govern platform behavior: who owns what, who can manage what, who can
access which instance.

```
Store ID:   "platform"
Scope:      Entire deployment (all instances)
Model:      Core sealed types only (user, instance, org, group, project, app, settings, session)
Writers:    Platform code only (bootstrap, reconciliation, membership mutations)
Readers:    Auth middleware, use-case authz checks, instance scoping middleware
API:        Not exposed to customers
```

**Tuples in the platform store:**

| Tuple | Meaning |
|-------|---------|
| `user:alice` → `owner` → `org:acme` | Alice owns the Acme org |
| `user:bob` → `viewer` → `org:acme` | Bob can read Acme resources |
| `instance:root` → `parent` → `instance:child-1` | Root is parent of child-1 |
| `user:alice` → `iam_owner` → `instance:child-1` | Alice gets child-instance IAM ownership through platform projection |
| `user:alice` → `admin` → `instance:child-1` | Alice can manage child-1 directly |

**Key property:** A single `check()` against the platform store can answer any
platform permission question without cross-store lookups or tuple projection.

### Customer Store (one per instance)

Each instance gets its own customer store for application-level authorization.
This is what customers interact with via the FGA API.

```
Store ID:   instance_id (unchanged from today)
Scope:      Single instance
Model:      Customer-defined types (referencing core types is allowed)
Writers:    Customer via /v1/fga/* API
Readers:    Customer applications via the same API
API:        Full OpenFGA-compatible surface, SDK-accessible
```

**Tuples in a customer store** (examples):

| Tuple | Meaning |
|-------|---------|
| `user:alice` → `editor` → `document:budget-2026` | App-level authz |
| `group:eng#member` → `viewer` → `folder:designs` | Group-based access |

**Key property:** Customers cannot write to or read from the platform store.
Customer stores never contain platform tuples.

### Customer FGA API Surface

The customer FGA API drops the `stores/{store_id}` path segment. The store is
always the current instance — resolved by the same instance context middleware
that scopes every other resource endpoint.

```
Direct access (customer hits their own instance):
    customer.example.com/v1/fga/check
    customer.example.com/v1/fga/tuples
    customer.example.com/v1/fga/model
    → store resolved from Host header / domain routing

Hierarchical access (root admin manages child instance):
    root.example.com/v1/instances/{id}/fga/check
    root.example.com/v1/instances/{id}/fga/tuples
    root.example.com/v1/instances/{id}/fga/model
    → store resolved from path param (existing dual-mount pattern)
```

This is the same pattern used for users, orgs, groups, and every other
resource: flat routes for your own instance, `/v1/instances/{id}/` prefix
for cross-instance access. No FGA-specific routing required.

**OpenFGA SDK compatibility shim:** The official OpenFGA SDKs (JS, Go, Java,
.NET, Python) construct paths as `/stores/{store_id}/check`, etc. To let
customers use these SDKs unmodified, we mount a compatibility shim:

```
/v1/fga/stores/{store_id}/check    → validates store_id matches instance, delegates to /v1/fga/check
/v1/fga/stores/{store_id}/read     → same
/v1/fga/stores/{store_id}/write    → same
/v1/fga/stores/{store_id}/expand   → same
/v1/fga/stores/{store_id}/list-objects  → same
/v1/fga/stores/{store_id}/list-users    → same
/v1/fga/stores/{store_id}/changes       → same
/v1/fga/stores/{store_id}/authorization-models      → same
/v1/fga/stores/{store_id}/authorization-models/{id} → same
```

The shim extracts `store_id`, verifies it equals the current instance ID
(returns 403 otherwise), and forwards to the canonical handler. Customers
configure their SDK with `store_id = instance_id` and `api_url =
https://customer.example.com/v1/fga` — everything works.

The shim also dual-mounts under `/v1/instances/{id}/fga/stores/{store_id}/*`
for hierarchical access, where `store_id` must match the path `{id}`.

### Store Resolution

```
Platform check (use-case layer, middleware):
    store = "platform"
    ─── always queries the single platform store

Customer check (FGA API endpoints):
    store = current_instance_id()
    ─── resolved by instance context middleware, same as all other endpoints
```

### POC Rollout

1. Create the `"platform"` store row in `fga_instance_stores`.
2. Seed the built-in role catalog from the vendored `InternalAuthZ.RolePermissionMappings` snapshot.
3. Rebuild platform tuples from authoritative retained tables during bootstrap/startup.
4. Update `reconcile_root_hierarchy` / parent reconciliation to write to the platform store only.
5. Update `authz::require_permission` and scoped instance middleware to always check the platform store.
6. Replace `/v1/fga/stores/{id}/*` routes with `/v1/fga/*` as the canonical customer API and keep `/stores/{id}` only as a compatibility shim.
7. Dual-mount customer FGA routes under `/v1/instances/{id}/fga/*` for hierarchical access.
8. Drop legacy child-store projection code paths.

For this POC there is no tuple backfill or migration from legacy customer stores. The `platform` store is treated as fresh derived state rebuilt from authoritative DB tables.

## Consequences

### Positive

- **No more tuple projection**: Platform permissions live in one place. Membership
  changes are immediately visible to all instance-scoped checks. No reconciliation
  delay, no stale tuples.
- **Use-case checks can use natural objects**: `instance:{id}` for instance-scoped
  resources, `org:{id}` for org-scoped resources — both resolve in the platform
  store.
- **Customer isolation**: A customer breaking their own FGA model cannot affect
  platform authorization. The platform store model is immutable (sealed core types
  only).
- **Simpler debugging**: Platform permission issues → inspect platform store.
  Customer app permission issues → inspect customer store. No cross-contamination.
- **Supports support grants**: ADR-036's staff access model becomes a tuple in the
  platform store (`user:staff → support_viewer → instance:child`), queryable
  without touching the child's customer store.

### Negative

- **Two stores to manage**: Bootstrap must initialize both platform and customer
  stores. Model upgrades apply to both (core model for platform, customer model
  untouched).
- **ADR-020 superseded**: The "single store, single graph" principle is replaced.
  The FGA API docs and SDK examples need to clarify which store customers interact
  with.
- **Self-hosted simplicity**: Single-instance SQLite deployments now have two FGA
  stores. Functionally invisible (the platform store is internal), but adds a row.

### Neutral

- **OpenFGA SDK compatibility**: Customer-facing API stays the same. SDKs continue
  to work against the customer store. The platform store is internal and not
  SDK-accessible.
- **Model composition**: `core.fga + custom.fga` merging only applies to customer
  stores. The platform store always uses the pure core model.
