# ADR-021: Multi-Tenancy via Instance Isolation

**Status:** Accepted  
**Date:** 2026-03-30  
**Depends-on:** ADR-020 (Authorization Model), ADR-022 (Dedicated Resource Tables)

## Context

Zitadel needs to support multiple isolated customer deployments within a single infrastructure. Each customer requires their own users, orgs, schemas, and configuration without data leakage. Additionally, Zitadel staff need to manage all customer instances from a unified console.

## Decision

We adopt a **Root Instance + Sub-instance** model with **shared-database, row-level discrimination** using an `instance_id` column on all tenant-scoped tables.

### Key Design Choices

1. **Database Strategy**: Shared database with `instance_id` discrimination column.
   - All tenant-scoped tables carry `instance_id TEXT NOT NULL DEFAULT 'inst_root'`
   - Indexed for efficient filtering
   - No separate databases per tenant (simplifies ops, enables cross-instance queries for root staff)

2. **Instance Hierarchy**: 
   - `inst_root` is the Zitadel-managed root instance (marked `is_root = true`)
   - Sub-instances are customer tenants created via the API
   - Root instance cannot be deleted or deactivated

3. **FGA Strategy**: Single shared OpenFGA store
   - `instance:inst_root → parent → instance:{sub}` tuples link hierarchy
   - Root staff inherit access to sub-instances through FGA's parent chain
   - Instance-scoped checks use `instance:{id}` from request context

4. **Instance Resolution** (priority order):
   1. Nested path: `/v1/instances/{iid}/...` (proxy strips prefix, sets context)
   2. `X-Zitadel-Instance` header (explicit override)
   3. Domain lookup: `Host` → `instances.domain` (customer-facing)
   4. Default: `inst_root`

5. **Console UI**: 
   - Instance switcher dropdown in header bar
   - "All Instances" management page
   - Nested API paths for root staff drill-down

## Tables Affected

All tenant-scoped tables now carry `instance_id`:
- `schemas`, `users`, `providers`, `apps`, `actions`, `login_flows`
- `sessions`, `events`, `domains`, `settings`
- `groups`, `projects`

The `instances` table gains:
- `domain TEXT` — for domain-based routing
- `is_root BOOLEAN` — marks the root instance

## Consequences

### Positive
- Single deployment serves unlimited customers
- Root staff can manage all instances from one console
- FGA provides natural authorization boundaries
- No infrastructure provisioning needed per customer

### Negative
- All SQL queries must be instance-scoped (enforced by convention + code review)
- Noisy-neighbor risk in shared database (mitigate with rate limiting per instance)
- Schema migrations affect all instances simultaneously

### Risks
- Missed `instance_id` filter → cross-tenant data leak (mitigate: query helpers, code review)
- Performance at scale with row discrimination (mitigate: proper indexing, partitioning later)
