# ADR-008: Meta Schema as Entity Catalog

**Status**: Proposed  
**Date**: 2026-03-27  
**Builds on**: ADR-005 (Unified Data Model), ADR-007 (Schema ↔ Engine Interaction)

## Context

Built-in entity types (human_user, app, org, etc.) are currently defined as inline Go strings in `bootstrap.go`. The meta schema only validates `x-*` annotations — it doesn't know what types exist.

This causes three problems:

1. **UI can't self-bootstrap** — navigation, create forms, and component rendering all need hardcoded maps
2. **No canonical type registry** — adding a new entity type requires Go code changes
3. **Defaults scattered** — where to define "ZITADEL ships with OIDC providers" vs. "here's the provider schema" is unclear

## Decision

### The meta schema becomes a three-layer manifest:

```
┌─────────────────────────────────────────────────┐
│                META SCHEMA                       │
│                                                  │
│  1. x-catalog    → what types exist?             │
│  2. x-groups     → how are they organized?       │
│  3. $defs        → what do annotations look      │
│                    like? (existing behavior)      │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│          SCHEMA FILES  (per type)                │
│                                                  │
│  schemas/human_user.json                         │
│  schemas/app.json                                │
│  schemas/org.json                                │
│  schemas/oidc_provider.json  (future)            │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│          FIXTURES  (seed instances)              │
│                                                  │
│  fixtures/defaults.yaml                          │
│  → admin user, default org, console app          │
└─────────────────────────────────────────────────┘
```

### 1. `x-catalog` — Type Registry

Declares every built-in entity type with its display metadata, API path, and component hints:

```json
{
  "x-catalog": {
    "human_user": {
      "schema_file": "schemas/human_user.json",
      "group": "identities",
      "alias": "Users",
      "singular": "User",
      "path": "users",
      "icon": "👤",
      "sort_order": 1,
      "required": true,
      "components": ["x-login", "x-branding", "x-auth-methods", "x-engine-claim-mapping"]
    },
    "service_user": {
      "schema_file": "schemas/service_user.json",
      "group": "identities",
      "alias": "Service Accounts",
      "singular": "Service Account",
      "path": "service-accounts",
      "icon": "🤖",
      "sort_order": 2,
      "components": ["x-auth-methods"]
    },
    "app": {
      "schema_file": "schemas/app.json",
      "group": "applications",
      "alias": "OIDC Clients",
      "singular": "OIDC Client",
      "path": "apps",
      "icon": "📱",
      "sort_order": 1,
      "components": ["x-oidc", "x-engine-claim-mapping"]
    }
  }
}
```

### 2. `x-groups` — Nav Structure

Defines how groups are organized in the UI. This replaces all hardcoded nav logic:

```json
{
  "x-groups": {
    "identities": { "label": "Identities", "icon": "👥", "sort_order": 1 },
    "applications": { "label": "Applications", "icon": "📱", "sort_order": 2 },
    "providers": { "label": "Providers", "icon": "🔗", "sort_order": 3 },
    "system": { "label": "System", "nav": "hidden" }
  }
}
```

### 3. `components` — UI Component Hints

The `components` array in each catalog entry tells the UI which annotation-driven panels to render on the detail/edit view. This is **declarative**: the UI doesn't scan the schema for `x-*` keys — it reads the catalog.

| Component key | Vue component | When shown |
|---|---|---|
| `x-login` | `XLoginPanel` | Interactive entity types |
| `x-branding` | `XBrandingPanel` | Types with custom login branding |
| `x-auth-methods` | `XAuthMethodsPanel` | Types with auth configuration |
| `x-engine-claim-mapping` | `XClaimMappingPanel` | Types that emit/consume claims |
| `x-oidc` | `XOidcPanel` | OIDC client apps |
| `x-engine-authorization` | `XAuthorizationPanel` | Types with FGA relations |

### 4. Where Defaults Go

| What | Where | Example |
|---|---|---|
| Type exists | `meta_schema.json → x-catalog` | "human_user is a thing" |
| Type shape | `schemas/human_user.json` | "Users have email, phone, locale..." |
| Default instances | `fixtures/defaults.yaml` | "admin user with password X" |

### 5. How the UI Uses This

**One fetch to bootstrap everything:**

```
GET /v1/schemas/$meta → meta_schema.json
```

From this single response, the UI can:

- **Build sidebar nav** → `x-groups` for sections, `x-catalog` for items within each group
- **Route to list pages** → `x-catalog[type].path` → `/v1/{path}`
- **Render detail panels** → `x-catalog[type].components` → mount matching Vue components
- **Create form** → knows the type, fetches `GET /v1/schemas?type={type}` for field definitions
- **Labels everywhere** → `alias`, `singular`, `icon` from catalog

No hardcoded maps needed anywhere in the frontend.

## File Structure (After)

```
internal/schema/
  meta_schema.json          ← catalog + annotation validation
  schemas/
    human_user.json         ← extracted from bootstrap.go
    service_user.json
    app.json
    ai_agent.json
    org.json

internal/bootstrap/
  bootstrap.go              ← reads meta_schema.json, loads schema files, seeds DB

fixtures/
  defaults.yaml             ← admin user, default org, console app
```

## Consequences

- **UI self-bootstraps** from a single `$meta` fetch — zero hardcoded maps
- **New type = new JSON files** — no Go code changes needed
- **Clear separation**: catalog (what exists) vs. schema (shape) vs. fixture (defaults)
- **Components are declarative** — the catalog says which panels to show, not the schema scanner
