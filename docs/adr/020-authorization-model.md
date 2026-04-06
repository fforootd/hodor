# ADR-020: Authorization Model — Immutable Core + Custom Fragment

**Status**: Accepted  
**Date**: 2026-03-29  
**Builds on**: ADR-005 (Unified Data Model), ADR-006 (Entity Naming Model)

## Context

With OpenFGA embedded (internal/fga), we need to answer three questions:

1. How do customers manage resources like groups and projects?
2. How do customers customize FGA for their own authorization needs?
3. How do customers access it from their apps?

## Decision

### One Store. One Graph. Two Active Layers.

Every Zitadel instance gets exactly **one** OpenFGA store. The authorization model
is currently composed from two active layers:

```
compiled_model = core.fga + custom.fga
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

### Layer 2: Customer Custom Types (user-defined)

Customers define their own types via the FGA model API. Custom types
reference core types freely but cannot shadow sealed types.

### Future Work: Marketplace Modules

The storage schema still keeps `module_fragments` for forward compatibility, and
legacy rows containing module fragments are still read and rebuilt. The embedded
POC runtime does **not** actively author or manage marketplace module layers yet.
That remains future work once the module lifecycle, API surface, and Console UX
are implemented end to end.

### API Compatibility

The `/v1/fga/*` endpoints are OpenFGA-protocol-compatible.
Official OpenFGA SDKs (JS, Go, Java, .NET, Python) work directly.

## Consequences

- **Single store**: no sync, no bridging between internal/external
- **SCIM compliance**: `/Groups` always works — group is a sealed primitive
- **AI Agent boundary**: locked primitives = stable physics engine across all deployments
- **Progressive adoption**: startup (defaults) → enterprise (custom ReBAC), with module layering reserved for future work
- **Breaking rename**: `entity` FGA type removed, replaced by type-specific primitives
