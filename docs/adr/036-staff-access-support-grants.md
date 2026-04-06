# ADR-036: Staff Access and Support Grants

**Status:** Proposed
**Date:** 2026-04-05
**Depends-on:** ADR-020 (Authorization Model), ADR-031 (Instance Hierarchy), ADR-034 (Multi-Tenancy via Instance Boundaries)
**Related:** ADR-029 (Control Plane, Auth Data Plane)

## Context

ADR-031 establishes that staff credentials live exclusively in the root instance and that child instances trust the root's OIDC issuer via federated provider links. ADR-020 defines the FGA authorization model with an immutable core layer.

Today, `operator_admin` is the only cross-instance privilege. It bypasses normal platform authorization checks — providing no scoping, no time-limiting, and no audit context beyond the standard request log. A support engineer debugging an SSO connection has the same access as an incident responder suspending a compromised account.

This ADR adds scoped, time-limited staff access to the authorization model.

## Decision

### 1. Support access is modeled as platform roles on instances

The core FGA model gains support-specific relations on the `instance` type in the internal deployment-scoped `platform` store:

```
type instance
  relations
    define owner:    [user, org#member]
    define admin:    [user, org#admin] or owner
    define viewer:   [user, org#viewer] or admin
    define support_read:   [user]
    define support_write:  [user] or support_read
    define support_config: [user] or support_write
    define support_admin:  [user] or support_config
```

| Role | Permitted operations |
|---|---|
| `support_read` | Read events, sessions, settings, provider config, user profiles |
| `support_write` | Above plus: reset passwords, revoke sessions, suspend/unsuspend users |
| `support_config` | Above plus: modify provider configuration, edit settings |
| `support_admin` | Full access including impersonation |

These are standard embedded-FGA relations in the platform model. They compose with the existing `owner/admin/viewer` hierarchy but are a separate branch — customer-facing roles do not imply support access.

### 2. Support grants are durable role assignments projected into platform tuples

A support grant is a durable role assignment that binds a staff principal to a support role on a customer instance. For managed children, the assignment is projected into the `platform` store as a tuple on `instance:{child}`. For federated children, the assignment remains authoritative in the root deployment and is also materialized as a short-lived support token.

```
principal:root:staff_alice -> support_read -> instance:acme-prod
```

Grant metadata such as `grant_id`, `reason`, issuer, and expiry lives in the authoritative assignment record. Managed instances enforce the grant through platform-store checks. Federated instances validate the support-grant token against root trust and enforce the role from the signed claims.

### 3. Grant lifecycle

**Issuance.** Staff requests a grant through the root instance:

```
POST /v1/support/grants
{
  "instance_id": "acme-prod",
  "role": "support_read",
  "reason": "SUPPORT-456: customer reports login failures",
  "duration_secs": 3600
}
```

The root instance validates that the requesting staff user has permission to issue grants at the requested role level, then writes the role assignment. Managed children get the derived platform tuple immediately; federated children receive a signed support token on issuance.

**Access.** Staff navigates to the child instance in the console — same UX as any user with cross-instance access. The child instance's auth middleware resolves the staff identity via federated OIDC trust and checks FGA for the support role. Standard `/v1/*` routes, no special prefix.

**Expiry.** Grants expire via the authoritative assignment metadata. Managed platform-tuple projection skips expired grants; federated support tokens use short expirations.

**Revocation.** For early revocation, revoke the role assignment. Takes effect immediately for managed instances because the next platform projection removes the tuple. Federated instances reject revoked grants when validating the support token against the root-side assignment state.

### 4. Audit trail

Every action performed via a support role produces events in the child instance's event stream. The staff identity and grant context are part of the actor metadata:

```
event_type: entity.updated
actor: staff:alice (root instance, federated)
grant_id: grant-123
reason: SUPPORT-456
role: support_write
action: password_reset
target: user:bob
```

Customers see support access events in their audit log. There is no hidden access path.

### 5. Federated instances (future extension)

For managed cloud instances, support access is enforced entirely through the internal `platform` store. No tuple is written into the child customer store.

For federated (on-prem) instances, the FGA check needs a different path because the federated instance has its own FGA store. Two approaches are available:

The root instance issues a support grant JWT that encodes the role and grant metadata. The federated instance validates the JWT against root's JWKS and enforces the role from the token claims. The platform-side role assignment remains the source of truth; there is no replicated tuple in the federated customer's store.

For both options, CORS for the staff console is handled by including the root origin in the federated instance's allowed origins — a one-time configuration as part of the federation trust setup.

### 6. `operator_admin` remains as break-glass

The existing `operator_admin` capability is retained as an emergency bypass. It continues to skip normal platform authorization checks. This is the break-glass path for incidents where the grant issuance flow itself is unavailable.

Organizations that want to enforce grant-only access can disable the `operator_admin` bypass via configuration.

## Consequences

### Positive

- Staff access is scoped, time-limited, and auditable through platform authorization
- Customers see every support action in their event stream
- No special route prefix or proxy — staff navigates child instances the same way customers do
- Support roles compose with the existing platform FGA model without modifying customer-facing roles
- `operator_admin` remains as a break-glass path

### Negative

- Managed support grants require synchronous projection from the authoritative assignment record into the platform store
- Grant issuance adds a step to the support workflow
- Federated instance support requires additional design work (deferred)

### Risks

- If support-token or assignment revocation checks drift from the authoritative assignment state, federated instances may accept stale grants until token expiry
- Scope creep in support roles: pressure to add finer-grained roles beyond the four tiers
- Federated instances that cache root JWKS may accept expired or revoked grants within the cache TTL
