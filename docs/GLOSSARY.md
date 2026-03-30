# Glossary & Ontology

This document defines the core vocabulary and terminology mappings for the Zitadel R&D prototype. Because this system redefines standard IAM concepts to fit a unified schema-driven model, AI agents and human contributors **must** use these definitions instead of hallucinating standard OAuth/IAM terminology.

| Term | Meaning | Legacy / Standard Equivalent |
|---|---|---|
| **Entity** | An instance of a schema. The universal noun for all domain objects (human users, service accounts, apps, organizations). | Identity, Client, Project |
| **Schema** | A versioned JSON Schema that defines the shape, behavior, and display of an entity type. | Fixed database columns |
| **Type** | The machine identifier for a schema (e.g., `human_user`, `app`). Immutable. | Hardcoded class/model |
| **Alias** | The human-readable name for a type (e.g., "Users"). Defined via `x-display`. | - |
| **Group** | A first-class collaborative resource plus a navigation/category concept depending on context. In the current prototype, Groups and Projects both exist as distinct resources. | Group / Team |
| **Path** | An API route alias (e.g., "users" → `/v1/users`). Defined via `x-display`. | - |
| **App** | An OIDC/SAML Client. It is simply an entity with a specific schema (`app` or `app_saml`). | OIDC Client, OAuth App |
| **Provider Kind** | The marketplace/provider family identifier, such as `google`, `github`, `gitlab`, `entra`, or `custom`. | Template family / vendor preset |
| **Protocol** | The runtime federation adapter used by a provider, such as `oidc`, `oauth2`, or `saml`. | Federation protocol |
| **Catalog Ref** | Metadata linking an installed resource back to a catalog template and version. | Template origin |
| **Org** | Organization. The top-level scope/context for filtering entities. | Tenant |

## Key Paradigms

1. **Apps are Identities**: Non-human identities (Service Accounts) and Applications (OIDC Clients) use exactly the same underpinnings as human users.
2. **If it doesn't have a schema, it doesn't exist**: All persistent domain objects must be entities defined by a JSON Schema.
3. **Relationships via FGA**: Relationships between entities (e.g., membership in a group, ownership of an org) are represented as graph edges in OpenFGA, not relational tables.

## Observability

The following terms define the structure and scope of the identity intelligence layer. Note that background "Jobs" (like queues and scheduling) are managed under "System", though their execution logs are treated as Events.

| Term | Meaning | Legacy / Standard Equivalent |
|---|---|---|
| **Overview** | Dashboards and aggregations of the current system state (e.g., high-level metrics, login trends). | Dashboards / Graphs |
| **Explore** | A dual-mode query interface consisting of a Visual Query Builder and a raw SQL Editor for parsing events and telemetry. | Query / Analytics |
| **Events** | Chronological stream of all discrete observations. Categorized by prefix: `entity.*`, `auth.*`, `request.*`, `log.*`, `signal.*`. | Logs / Audits |
| **Category** | The top-level classification of an event. Stored as an indexed column, derived from the `event_type` prefix. Values: `entity`, `auth`, `session`, `token`, `request`, `log`, `signal`, `threat`, `system`. | — |
| **Traces** | Correlated causal chains of events relying on `trace_id` and `span_id`. Provides end-to-end visibility from client SDK interactions to platform actions. | Activity / APM |
| **Sessions** | Live authenticated state management. Bridges identities and historical events by providing a point of active security enforcement/revocation. Session metadata also carries auth provenance such as `auth_method`, `provider_id`, `provider_kind`, and `login_flow_id`. | Sessions / Grants |
| **Cache** | A per-process SQLite database (`./data/zitadel-cache.db`) used as a durable buffer for analytics writes, settings cache, and rate limiter state. Disposable — can be deleted or run on tmpfs. Not a source of truth. | Local WAL / Sidecar cache |

### Event Categories

| Category | Event type pattern | Examples | Written by |
|---|---|---|---|
| `entity` | `entity.*` | `entity.created`, `entity.updated` | `emitEvent()` in OLTP TX |
| `auth` | `auth.*` | `auth.login_completed`, `auth.login_failed` | `emitEvent()` in OLTP TX |
| `session` | `session.*` | `session.created`, `session.revoked` | `emitEvent()` in OLTP TX |
| `token` | `token.*` | `token.issued`, `token.revoked` | `emitEvent()` in OLTP TX |
| `request` | `request.*` | `request.api`, `request.oidc` | Logger → cache → drain |
| `log` | `log.*` | `log.error`, `log.warn`, `log.info` | Logger → cache → drain |
| `signal` | `signal.*` | `signal.ui.rendered` *(future)* | OTLP ingestion |
| `threat` | `threat.*` | `threat.detected` *(future)* | Intelligence engine |

### Three-Tier Data Architecture

| Tier | What | Failure behavior | Example |
|---|---|---|---|
| **1. OLTP** | Transactional writes (entities, sessions, domain events) | Operation fails → user gets error | `entity.created` in same TX |
| **2. OLAP** | Analytics store via local cache buffer | Data accumulates in cache, drains when backend recovers | `request.api`, `log.error` |
| **3. Fire-and-forget** | stdout, OTEL export | Drop, move on | Operator console, OTEL collector |
