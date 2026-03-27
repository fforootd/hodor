# ADR-006: Entity Naming Model — Schema-as-Ontology

**Status**: Proposed  
**Date**: 2026-03-27  
**Builds on**: ADR-005 (Unified Data Model)

## Context

ADR-005 established that everything is a schema-defined entity. However, the implementation still uses "identities" as the primary noun, nav sections are hardcoded, and some system concepts (providers, sessions, events) live outside the schema registry entirely.

This creates a split brain: some things are schema-driven, others aren't. We need a single principle.

## Core Principle

> **If it doesn't have a schema, it doesn't exist.**

The schema registry is the system's ontology — the complete catalog of everything Zitadel knows about. Every concept in the system, whether it's a user, an app, a session, or an event, has a schema that defines its shape, behavior, and presentation.

## Decision

### 1. Rename: Identities → Entities

The universal noun for all domain objects is **entity**. An entity is an instance of a schema.

```
identities table  →  entities table
/v1/identities    →  /v1/entities
identity_id       →  entity_id
```

### 2. Schema = Source of Truth

Every schema carries `x-display` and `x-storage` annotations that define how it appears and where it lives:

```json
{
  "type": "object",
  "x-display": {
    "alias": "Users",
    "singular": "User",
    "group": "identities",
    "group_label": "Identities",
    "path": "users",
    "icon": "👤",
    "sort_order": 1
  },
  "x-storage": "entities",
  "properties": { ... }
}
```

### 3. Complete Type Catalog

Every concept in the system has a schema:

#### CRUD Entities (`x-storage: "entities"`)

Persistent objects in the `entities` table with full CRUD.

| Type | Alias | Group | Path |
|---|---|---|---|
| `human_user` | Users | identities | `users` |
| `service_user` | Service Accounts | identities | `service-accounts` |
| `ai_agent` | AI Agents | identities | `ai-agents` |
| `app` | OIDC Clients | applications | `apps` |
| `app_saml` | SAML Clients | applications | `saml-apps` |
| `org` | Organizations | system | `orgs` |
| `provider` | Providers | configure | `providers` |
| `group` | Groups | access | `groups` |
| `rule` | Rules | configure | `rules` |

#### Runtime State (`x-storage: "dedicated"`)

High-write/append-only data in dedicated optimized tables. Schema defines shape but data doesn't live in `entities` table.

| Type | Alias | Group | Path | Storage |
|---|---|---|---|---|
| `session` | Sessions | observability | `sessions` | `sessions` table |
| `event` | Events | observability | `events` | `events` table |
| `job` | Jobs | observability | `jobs` | `jobs` table |

#### System Schemas (`x-storage: "self"`)

The schema type itself — the schema registry describes itself.

| Type | Alias | Group | Path |
|---|---|---|---|
| `schema` | Schemas | system | `schemas` |

### 4. Three Naming Layers

| Layer | Purpose | Example | Mutable? |
|---|---|---|---|
| **type** | Machine identifier | `human_user` | No |
| **alias** | Human label (UI) | `"Users"` | Yes |
| **group** | Nav section | `"identities"` | Yes |
| **path** | API route alias | `"users"` | Yes |

- **type** — set at schema creation, never changes
- **alias/group/path** — presentation metadata, can evolve

### 5. Auto-Generated API Route Aliases

On startup, the router reads all schemas and registers aliased routes from `x-display.path`:

```
GET  /v1/users              → GET /v1/entities?type=human_user
POST /v1/users              → POST /v1/entities (type: human_user)
GET  /v1/users/{id}         → GET /v1/entities/{id}
GET  /v1/service-accounts   → GET /v1/entities?type=service_user
GET  /v1/apps               → GET /v1/entities?type=app
GET  /v1/sessions           → GET /v1/sessions (dedicated table)
```

New schema type with `"path": "webhooks"` → `/v1/webhooks` exists immediately. Zero code.

### 6. Dynamic Nav

The frontend calls `GET /v1/schemas`, groups by `x-display.group`, and renders the sidebar:

```
IDENTITIES              ← group_label
  ◇ 👤 Users             ← alias + icon, links to /console/users
  ◇ 🤖 Service Accounts
  ◇ 🧠 AI Agents

APPLICATIONS
  ◇ 📱 OIDC Clients

ACCESS
  ◇ 👥 Groups

CONFIGURE
  ◇ 🔗 Providers
  ◇ ⚡ Rules

OBSERVABILITY
  ◇ 🔑 Sessions
  ◇ 📋 Events
  ◇ ⏱ Jobs

SYSTEM
  ◇ 📐 Schemas
```

No hardcoded nav. Adding a schema with `group: "configure"` automatically adds it to that section.

### 7. Group-Level Filtering

```
GET /v1/entities?group=identities    → users + service accounts + AI agents
GET /v1/entities?group=applications  → all apps
GET /v1/entities?group=configure     → providers + rules
```

### 8. Updated Seed Format

```yaml
entities:
  - identifier: jane@example.com
    display_name: Jane Doe
    type: human_user
    password: password123
    data:
      email: jane@example.com
      display_name: Jane Doe
      locale: en-US
```

`type` replaces `schema_id`. The system resolves the default schema version for that type.

## Consequences

- **Single ontology**: if it doesn't have a schema, it doesn't exist
- **One API noun**: `entities` + auto-aliased paths (`/v1/users`, `/v1/apps`)
- **Self-describing**: schemas carry all UI/API/storage metadata
- **Zero-code extensibility**: new schema → new nav entry + API route
- **Consistent**: no more "is this an identity or a config object?" — everything is an entity
- **Runtime separation**: sessions/events get dedicated storage but still have schemas
- **Breaking rename**: `identities` → `entities` across DB, API, frontend
