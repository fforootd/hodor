# ADR-005: Unified Data Model — Schemas, Orgs, and Config Cascade

**Status**: Accepted  
**Date**: 2026-03-27  
**Builds on**: ADR-004 (Apps as Identities)

## Context

As ZITADEL grows beyond identities and OIDC, we need a coherent model for how all entities (users, apps, orgs, providers, rules), their relationships (groups, grants), and configuration (branding, login policies, notifications) fit together.

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

All persistent domain objects are identities with schemas:

| Entity | Schema Type | Key Properties |
|---|---|---|
| Human User | `human_user` | email, phone, name, profile |
| Service Account | `service_user` | key pairs, scopes |
| AI Agent | `ai_agent` | model, capabilities |
| OIDC App | `app` | redirect_uris, grant_types (`x-oidc`) |
| SAML App | `app_saml` | entity_id, acs_url (`x-saml`) |
| Organization | `org` | branding, login_policy, notification_channels |
| Provider | `provider` | protocol, issuer, client_id, mapping |
| Rule | `rule` | triggers, conditions, actions |

Apps and organizations are identities in the same `identities` table, differentiated by their schema type.

### 3. Layer 2: Relationships via FGA

Relationships are graph edges, not tables. They live in the authorization model:

| Relationship | Subject | Object | Semantics |
|---|---|---|---|
| `member` | identity | org | User/app belongs to org |
| `owner` | identity | org | Administers org |
| `member` | identity | group | In a group |
| `grant` | identity/group | app/role | Authorization grant |

**Groups replace Projects. A group containing apps + users + grants IS a project.**

### 4. Layer 3: Config Cascade (Instance → Org → App)

Configuration follows CSS-like specificity:

```
Instance defaults
  └── Org overrides
      └── App overrides
```

Resolution: `app.config ?? org.config ?? instance.config`

- **Instance config**: schema `default` values
- **Org config**: org entity's `data` field
- **App config**: app entity's `data` field

Applies to: branding, login policy, rate limits, captcha, notification channels, rules.

### 5. Layer 4: Runtime State

Ephemeral, high-write state stays in dedicated tables (not schemas):

Sessions, tokens, auth requests, events, jobs.

### 6. Organizations as Scope/Context

Orgs are the **top-level scope** (like Vercel's project switcher):

- **Topbar context switcher** with [🔽 Org ▾] dropdown + ⚙ settings
- **"All orgs" mode** for instance-level admin view
- **1:N membership**: users can belong to multiple orgs
- **Everything scoped**: when org is selected, all lists filter by `org_id`
- **Not a nav item**: org settings accessed via ⚙ gear icon in the switcher

### 7. Console Nav Structure

```
[🔽 Org Switcher] [⚙ Settings]

◆ Dashboard

IDENTITIES        (scoped to selected org)
◇ Users           ← human_user
◇ Service Accounts ← service_user
◇ AI Agents       ← ai_agent

APPLICATIONS      (scoped to selected org)
◇ OIDC Clients    ← app (x-oidc)
◇ SAML Clients    ← app_saml (future)

ACCESS
◇ Groups          ← group + membership edges
◇ Authorizations  ← FGA grants

CONFIGURE
◇ Providers       ← provider entities
◇ Rules           ← rule entities

OBSERVABILITY
◇ Sessions
◇ Events
◇ Jobs

SYSTEM
◇ Schemas
```

Nav entries under IDENTITIES and APPLICATIONS are **dynamically generated** from registered schemas (sorted by explicit `typeOrder` priority).

## Consequences

- **Uniform model**: users, apps, orgs, providers, rules — all identities with different schemas
- **No separate tables**: per-concept (branding, login policy, etc.) — config lives in schema `data`
- **Extensible nav**: new schema types automatically add nav entries
- **Org scoping**: everything filters by org context, reducing complexity
- **1:N orgs**: users can switch between organizations freely
- **Groups replace projects**: simpler model, same capability
- **Cascade config**: instance defaults with org/app overrides, no config table sprawl
