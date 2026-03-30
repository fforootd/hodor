# ADR-007: Schema ↔ Engine Interaction Model

**Status**: Proposed  
**Date**: 2026-03-27  
**Builds on**: ADR-005 (Unified Data Model), ADR-006 (Entity Naming)

## Context

The schema defines *what* an entity is. But the system also needs to know *how* to process it:

- **Claim mapping** uses **expr** expressions to resolve OIDC claims from entity data
- **Authorization** uses **FGA** relations to check who can access what
- **Login flows** use the schema's `x-login` / `x-auth-methods` to determine available auth methods
- **Providers** use **expr** to map inbound IDP attributes to schema fields

Currently these bindings are implicit — the code just "knows" that `x-claim-mapping` values are expr expressions, and that identities have FGA relations. Nothing in the schema itself declares which engines operate on it.

### The problem

1. **No discoverability** — you can't look at a schema and know what engines process it
2. **No UI link** — the frontend hardcodes which panels to show per entity type
3. **No validation** — there's no way to check that a claim mapping expression is valid expr syntax
4. **No reuse** — provider inbound mapping and schema outbound mapping use different code paths for the same concept

## Core Principle

> **The schema declares its engine bindings. Engines are pluggable processors declared via `x-engine` annotations.**

## Decision

### 1. Engine Binding Annotations

Each schema declares which engines process it via `x-engine-*` annotations at the schema root level:

```json
{
  "type": "object",
  "x-display": { "alias": "Users", "group": "identities", "path": "users" },

  "x-engine-claim-mapping": {
    "engine": "expr",
    "direction": "outbound",
    "description": "Maps entity fields → OIDC claims"
  },

  "x-engine-authorization": {
    "engine": "fga",
    "model": "entity",
    "relations": ["member", "owner", "viewer"],
    "description": "FGA relations for access control"
  },

  "x-engine-login": {
    "engine": "built-in",
    "description": "Interactive login flow configuration"
  },

  "properties": {
    "email": {
      "type": "string",
      "x-claim-mapping": "claims.email",
      "x-auth": { "identifier": true }
    }
  }
}
```

### 2. Engine Registry

The system maintains a registry of available engines:

| Engine | Purpose | Used by |
|---|---|---|
| `expr` | Expression evaluation | Claim mapping (inbound + outbound), computed fields |
| `fga` | Fine-grained authorization | Entity access control, group membership, grants |
| `built-in` | Core system logic | Login flows, session management |

Each engine binding tells the system:
- **engine**: which processor handles this (expr, fga, built-in)
- **direction**: inbound (provider → entity), outbound (entity → claims), or bidirectional
- **model/relations**: for FGA, which authorization model and relations apply

### 3. How the Layers Connect

```
┌─────────────────────────────────────────────────────────────┐
│                        SCHEMA                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ properties:                                          │   │
│  │   email:                                             │   │
│  │     x-claim-mapping: "claims.email"      ──────────┐ │   │
│  │     x-auth: { identifier: true }         ─────┐    │ │   │
│  └──────────────────────────────────────────────────┘   │   │
│                                                    │    │   │
│  x-engine-authorization ──┐                        │    │   │
│  x-engine-claim-mapping ──┼──┐                     │    │   │
│  x-engine-login ──────────┼──┼──┐                  │    │   │
└───────────────────────────┼──┼──┼──────────────────┼────┼───┘
                            │  │  │                  │    │
                   ┌────────┘  │  │      ┌───────────┘    │
                   ▼           ▼  ▼      ▼                ▼
              ┌─────────┐ ┌────────┐ ┌──────────┐  ┌──────────┐
              │   FGA   │ │  EXPR  │ │ Login    │  │  Claims  │
              │ Engine  │ │ Engine │ │ Engine   │  │ Resolver │
              └────┬────┘ └───┬────┘ └────┬─────┘  └────┬─────┘
                   │          │           │              │
              Relations   Evaluate    Auth flow    OIDC tokens
              checked     mappings    rendered     populated
```

### 4. Claim Mapping: Three Contexts, One Model

The same `x-claim-mapping` annotation works in three contexts:

#### a) Schema (definition)

Declares what OIDC claim each field maps to:
```json
"email": {
  "type": "string",
  "x-claim-mapping": "claims.email"
}
```

#### b) Provider (inbound)

Maps IDP attributes to entity fields using expr:
```json
"x-provider-mapping": {
  "engine": "expr",
  "direction": "inbound",
  "mappings": {
    "email": "idp_claims.email",
    "display_name": "idp_claims.name ?? (idp_claims.given_name + ' ' + idp_claims.family_name)",
    "avatar_url": "idp_claims.picture ?? ''"
  }
}
```

#### c) App (outbound)

Filters/transforms which claims appear in tokens for this app:
```json
"x-oidc": {
  "claim_scopes": {
    "profile": ["name", "family_name", "given_name", "picture", "locale"],
    "email": ["email", "email_verified"],
    "phone": ["phone_number", "phone_number_verified"]
  }
}
```

**The flow:**
```
IDP attributes ──[expr]──→ Entity fields ──[x-claim-mapping]──→ OIDC claims ──[x-oidc scope filter]──→ Token
     Provider               Schema                Schema                    App
     (inbound)              (storage)             (outbound)               (filtering)
```

### 5. FGA Integration

The schema declares which FGA relations apply to entities of this type:

```json
"x-engine-authorization": {
  "engine": "fga",
  "relations": ["member", "owner", "viewer", "editor"],
  "checks": {
    "read": "viewer | editor | owner",
    "write": "editor | owner",
    "delete": "owner",
    "admin": "owner"
  }
}
```

This drives:
- **API authorization**: middleware reads `x-engine-authorization.checks` and validates
- **UI**: shows/hides edit buttons based on current user's FGA relations
- **Access panel**: the "Authorizations" UI renders FGA relations defined here

### 6. Schema Refs (Component Linking)

Each `x-engine-*` annotation also serves as a **component ref** — it tells the UI which panel component to render:

| Annotation | UI Component | Engine |
|---|---|---|
| `x-engine-login` | `XLoginPanel` | built-in |
| `x-engine-claim-mapping` | `XClaimMappingPanel` | expr |
| `x-engine-authorization` | `XAuthorizationPanel` | fga |
| `x-oidc` | `XOidcPanel` | built-in |
| `x-branding` | `XBrandingPanel` | built-in |
| `x-auth-methods` | `XAuthMethodsPanel` | built-in |
| `x-provider-mapping` | `XProviderMappingPanel` | expr |

The convention: **annotation key → component name**. The `SchemaAnnotationRenderer` scans the schema for these keys and renders matching components. No hardcoded panel list.

### 7. Validation via Engine

Because the schema declares its engine, validation can be engine-aware:

```json
"x-claim-mapping": "claims.email"
```

The system knows this field's mapping is an **expr** expression (from `x-engine-claim-mapping.engine: "expr"`). So:
- On save, it can **syntax-check** the expr
- On preview, it can **evaluate** it against real entity data
- On diff, it can **simulate** what changes to the mapping would do

### 8. Complete human_user Schema Example

```json
{
  "type": "object",
  "x-display": {
    "alias": "Users", "singular": "User",
    "group": "identities", "path": "users",
    "icon": "👤", "sort_order": 1
  },
  "x-storage": "entities",
  "x-engine-claim-mapping": {
    "engine": "expr",
    "direction": "outbound"
  },
  "x-engine-authorization": {
    "engine": "fga",
    "relations": ["member", "owner", "viewer"]
  },
  "x-engine-login": {
    "engine": "built-in"
  },
  "x-login": {
    "strategy": "identifier_first",
    "mfa_required": false,
    "registration_allowed": true
  },
  "x-auth-methods": {
    "password": { "enabled": true, "interactive": true },
    "magic_link": { "enabled": true, "interactive": true },
    "passkey": { "enabled": false, "interactive": true },
    "sso": { "enabled": true, "interactive": true }
  },
  "x-branding": {
    "heading": "Welcome back",
    "colors": { "primary": "#6366f1" }
  },
  "properties": {
    "email": {
      "type": "string", "format": "email",
      "x-claim-mapping": "claims.email",
      "x-auth": { "identifier": true, "recovery": "email", "verification": "email" }
    },
    "display_name": {
      "type": "string",
      "x-claim-mapping": "claims.name ?? (claims.given_name + ' ' + claims.family_name)"
    },
    "avatar_url": {
      "type": "string", "format": "uri",
      "x-claim-mapping": "claims.picture ?? ''"
    }
  }
}
```

## Consequences

- **Self-describing**: look at a schema → know exactly what engines process it
- **Pluggable**: new engine = new `x-engine-*` annotation, no core code change
- **Unified claim mapping**: one model for provider (inbound) + schema (outbound) + app (filtering)
- **UI driven by annotations**: `x-engine-*` keys map 1:1 to UI panel components
- **Validatable**: engine-aware validation of expressions, mappings, FGA relations
- **Introspectable**: the schema registry is a complete capability manifest
