# ADR-005: Unified Data Model — Schemas, Orgs, and Config Cascade

**Status**: Accepted (amended by ADR-006)  
**Date**: 2026-03-27  
**Builds on**: ADR-004 (Apps as Identities)  
**Amended by**: ADR-006 (Entity Naming Model)

## Context

As ZITADEL grows beyond identities and OIDC, we need a coherent model for how all domain objects, their relationships, and configuration fit together.

Old ZITADEL had separate tables and models for each concept (projects, actions, login policies, branding, etc.), leading to a rigid system that was hard to extend. The schema-driven approach from ADR-001 through ADR-004 gives us a foundation — this ADR extends it to cover the full domain.

## Decision

### 1. Four-Layer Architecture

```
Layer 1: Entities (schema-defined)        — what things ARE
Layer 2: Relationships (FGA graph edges)  — how things CONNECT
Layer 3: Configuration (cascading)        — how things BEHAVE
Layer 4: Runtime (ephemeral state)        — what's HAPPENING NOW
```

### 2. Layer 1: Everything is a Schema Entity

All persistent domain objects are **entities** with schemas. The term "entity" replaces "identity" as the universal noun — see ADR-006 for full naming model.

| Entity | Schema Type | Key Properties |
|---|---|---|
| Human User | `human_user` | email, phone, name, profile |
| Service Account | `service_user` | key pairs, scopes |
| AI Agent | `ai_agent` | model, capabilities |
| OIDC App | `app` | redirect_uris, grant_types (`x-oidc`) |
| SAML App | `app_saml` | entity_id, acs_url (`x-saml`) |
| Organization | `org` | branding, login_policy, notification_channels |
| Provider | `provider` | protocol, issuer, client_id, mapping |
| Group | `group` | description, membership rules |
| Rule | `rule` | triggers, conditions, actions |

All live in the `entities` table, differentiated by their schema type.

### 3. Layer 2: Relationships via FGA

Relationships are graph edges, not tables:

| Relationship | Subject | Object | Semantics |
|---|---|---|---|
| `member` | entity | org | Entity belongs to org |
| `owner` | entity | org | Administers org |
| `member` | entity | group | In a group |
| `grant` | entity/group | app/role | Authorization grant |

**Groups replace Projects. A group containing apps + users + grants IS a project.**

### 4. Layer 3: Config Cascade (Instance → Org → App)

Configuration follows CSS-like specificity:

```
Instance defaults
  └── Org overrides
      └── App overrides
```

Resolution: `app.config ?? org.config ?? instance.config`

Applies to: branding, login policy, rate limits, captcha, notification channels, rules.

### 5. Layer 4: Runtime State

Ephemeral, high-write state stays in dedicated tables (not the `entities` table):

Sessions, tokens, auth requests, events, jobs.

**These still have schemas** that describe their shape (see ADR-006 `x-storage: "dedicated"`), but data lives in optimized storage.

### 6. Organizations as Scope/Context

Orgs are the **top-level scope** (like Vercel's project switcher):

- **Topbar context switcher** with [🔽 Org ▾] dropdown + ⚙ settings
- **"All orgs" mode** for instance-level admin view
- **1:N membership**: users can belong to multiple orgs
- **Everything scoped**: when org is selected, all lists filter by `org_id`
- **Not a nav item**: org settings accessed via ⚙ gear icon in the switcher

### 7. Console Nav Structure

Nav sections are **dynamically generated** from schema `x-display` metadata (see ADR-006):

```
[🔽 Org Switcher] [⚙ Settings]

◆ Dashboard

IDENTITIES        ← group: "identities"
◇ Users           ← human_user (alias: "Users", path: "users")
◇ Service Accounts ← service_user
◇ AI Agents       ← ai_agent

APPLICATIONS      ← group: "applications"
◇ OIDC Clients    ← app

ACCESS            ← group: "access"
◇ Groups          ← group entity
◇ Authorizations  ← FGA grants

CONFIGURE         ← group: "configure"
◇ Providers       ← provider entity
◇ Rules           ← rule entity

OBSERVABILITY     ← group: "observability"
◇ Sessions        ← x-storage: "dedicated"
◇ Events          ← x-storage: "dedicated"
◇ Jobs            ← x-storage: "dedicated"

SYSTEM
◇ Schemas
```

Adding a new schema type with the right `x-display` annotations automatically adds it to the correct nav section and creates API routes.

## Terminology

| Term | Meaning |
|---|---|
| **Entity** | An instance of a schema. The universal noun for all domain objects. |
| **Schema** | A versioned JSON Schema that defines the shape, behavior, and display of an entity type. |
| **Type** | The machine identifier for a schema (e.g., `human_user`). Immutable. |
| **Alias** | The human-readable name for a type (e.g., "Users"). From `x-display`. |
| **Group** | A nav section that categorizes schema types (e.g., "identities"). From `x-display`. |
| **Path** | An API route alias (e.g., "users" → `/v1/users`). From `x-display`. |

## Consequences

- **Uniform model**: users, apps, orgs, providers, rules — all entities with different schemas
- **No separate tables**: per-concept config lives in schema `data`
- **Extensible**: new schema types add nav entries and API routes automatically
- **Org scoping**: everything filters by org context
- **Groups replace projects**: simpler model, same capability
- **Cascade config**: instance defaults with org/app overrides
