# ADR-020: Authorization Model — Immutable Core + Marketplace Modules

**Status**: Accepted  
**Date**: 2026-03-29  
**Builds on**: ADR-005 (Unified Data Model), ADR-006 (Entity Naming Model)

## Context

With OpenFGA embedded (internal/fga), we need to answer three questions:

1. How do customers manage resources like groups and projects?
2. How do customers customize FGA for their own authorization needs?
3. How do customers access it from their apps?

## Decision

### One Store. One Graph. Three Layers.

Every Zitadel instance gets exactly **one** OpenFGA store. The authorization model
is composed from three layers using
[OpenFGA Modular Models](https://openfga.dev/docs/modeling/modular-models):

```
compiled_model = core.fga + [enabled_modules/*.fga] + custom.fga
```

### Layer 1: Immutable Core (ships with binary)

Sealed types that Zitadel's APIs, SCIM, and console depend on.
Customers cannot modify or delete these.

| Type | Purpose |
|---|---|
| `user` | Actor (any identity) |
| `instance` | Global scope |
| `org` | Tenant boundary (SSO, branding, domains, config cascade) |
| `group` | User grouping — SCIM-compatible, usable in customer authz |
| `project` | App + role + grant containers |
| `app` | OIDC/SAML clients |
| `settings` | Cascading policies |
| `session` | Active sessions |

### Layer 2: Marketplace Modules (opt-in)

Pre-built authorization patterns that customers can install.
Each module adds FGA type definitions + API routes + Console UI.

| Module | What it adds |
|---|---|
| RBAC | `role`, `permission` types; `/v1/modules/rbac/roles` API; token claims |
| ABAC | `policy` type; condition-based evaluation; expr-lang policies |
| Teams | `team` type with hierarchical membership inheritance |

Module installation = FGA schema append + feature flag flip. No Go plugins.
Model is compiled in memory before writing (fail-fast validation).

### Layer 3: Customer Custom Types (user-defined)

Customers define their own types via the FGA model API. Custom types
reference core and module types freely but cannot shadow sealed types.

### API Compatibility

The `/v1/fga/*` endpoints are OpenFGA-protocol-compatible.
Official OpenFGA SDKs (JS, Go, Java, .NET, Python) work directly.

## Consequences

- **Single store**: no sync, no bridging between internal/external
- **SCIM compliance**: `/Groups` always works — group is a sealed primitive
- **AI Agent boundary**: locked primitives = stable physics engine across all deployments
- **Progressive adoption**: startup (defaults) → scale-up (RBAC module) → enterprise (custom ReBAC)
- **Breaking rename**: `entity` FGA type removed, replaced by type-specific primitives
