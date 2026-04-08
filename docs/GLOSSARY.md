# Glossary & Ontology

This document defines the core vocabulary and terminology mappings for the Zitadel R&D prototype. Because this system redefines standard IAM concepts to fit a unified schema-driven model, AI agents and human contributors **must** use these definitions instead of hallucinating standard OAuth/IAM terminology.

| Term | Meaning | Legacy / Standard Equivalent |
|---|---|---|
| **Entity** | An instance of a schema. This is an internal architecture/model term for schema-backed domain objects such as users, apps, organizations, and groups. | Identity, Client, Project |
| **Schema** | A versioned JSON Schema that defines the shape, behavior, and display of an entity type. | Fixed database columns |
| **Type** | The machine identifier for a schema (e.g., `human_user`, `app`). Immutable. | Hardcoded class/model |
| **Alias** | The human-readable name for a type (e.g., "Users"). Defined via `x-display`. | - |
| **Group** | A first-class collaborative resource plus a navigation/category concept depending on context. In the current prototype, Groups and Projects both exist as distinct resources. | Group / Team |
| **Path** | The canonical public API family path for a schema-backed resource (e.g., "users" → `/v1/users`). Defined via `x-display`. | - |
| **App** | An OIDC/SAML Client. It is simply an entity with a specific schema (`app` or `app_saml`). | OIDC Client, OAuth App |
| **Provider Kind** | The marketplace/provider family identifier, such as `google`, `github`, `gitlab`, `entra`, or `custom`. | Template family / vendor preset |
| **Protocol** | The runtime federation adapter used by a provider, such as `oidc`, `oauth2`, or `saml`. | Federation protocol |
| **Catalog Ref** | Metadata linking an installed resource back to a catalog template and version. | Template origin |
| **Instance** | The top-level ZITADEL runtime boundary. An instance contains organizations, users, apps, providers, settings, and authorization state. | Tenant / Environment |
| **Org** | Organization inside an instance. Used for business structure, ownership, and policy, but not for infrastructure routing. | Organization / Workspace |
| **Customer** | Cloud control-plane account that can own one or more instances. Not a data-access discriminator. | Billing account / Subscription owner |
| **Placement Mode** | Cloud placement policy for an instance: `global` or `regional`. | Data residency / home region policy |
| **Backend Key** | Logical binding from an instance to an operator-managed backend configuration, such as a regional managed backend. | Backend alias / connection profile |
| **Control Plane** | Portal and management side of the system: routing, placement, admin mutations, provider and policy authoring. | Management plane |
| **Auth Data Plane** | End-user authentication runtime: login, session and token handling, auth runtime state, revocation checks. | Auth serving plane |

## Public Naming Model

- **Public API families** use concrete nouns such as `users`, `apps`, `orgs`, `groups`, and `providers`.
- **`orgs`** are domain resources inside the currently resolved instance.
- **`instances`** are primarily a runtime/control-plane concern. Self-hosted deployments normally run exactly one instance; cloud resolves the current instance before API handlers run.
- **Users** is a typed family that currently covers `human_user`, `service_user`, and `ai_agent`.
- **Apps** is a typed family for application schemas.
- **`schema_id`** is the canonical write-time discriminator in request bodies.
- **`schema_type`** is the canonical read/filter discriminator on family list endpoints.
- **`identity`** is reserved for industry phrases such as identity provider and identity management, rather than being the default CRUD noun.
- **`entity`** remains useful for internal architecture, storage, and schema-engine discussions, but it is not the default public API noun.

## Key Paradigms

1. **Apps and users share the same schema-driven model**: Service accounts, AI agents, and applications are all backed by schemas even when they live behind different public API families.
2. **If it doesn't have a schema, it doesn't exist**: All persistent domain objects must be entities defined by a JSON Schema.
3. **Relationships via FGA**: Relationships between entities (e.g., membership in a group, ownership of an org) are represented as graph edges in OpenFGA, not relational tables.
4. **Instances contain orgs**: `instance_id` is the infrastructure and routing boundary; `org_id` stays inside that boundary as product/domain data.
5. **Control plane and auth data plane are separate**: brief control-plane outages are acceptable when regional auth continuity can continue through `read`, `kv`, and `sink`.

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
| **Process Cache** | Local read acceleration for stable data such as schemas, settings, and provider metadata. In the current POC this role is memory-only and never a source of truth. | Local in-memory cache |
| **Analytics Cache** | A per-process SQLite database (`./data/zitadel-cache.db`) used as the durable buffer for observability drains. Disposable — can be deleted or run on tmpfs. Not a source of truth. | Local WAL / Sidecar cache |

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

### Backend Architecture (ADR-032)

| Term | Meaning | Legacy / Standard Equivalent |
|---|---|---|
| **Repository port** | Trait in `zitadel-app` defining a persistence contract. Use cases depend on ports, not storage implementations. | Interface / Port (hexagonal) |
| **Repository adapter** | Concrete implementation of a repository port, typically in `zitadel-db`. Backed by SQL, KvStore, or external services. | Adapter / Driver |
| **Transport adapter** | HTTP handler, login route, OIDC endpoint, or CLI command that parses input, calls use cases, and maps responses. | Controller / Handler |
| **Use case** | Single business operation with a typed command and result. Owns validation, authorization, and domain event emission. | Service method / Command handler |
| **Wiring** | Server startup assembly that connects repository adapters to ports. Lives in `zitadel-server/src/wiring.rs`. | Composition root / DI container |
| **Primary storage** | Authoritative durable database for product state (`storage.primary`). | Main DB |
| **Transient storage** | Authoritative database for auth-runtime state such as sessions and login flows (`storage.transient`). | Session store |
| **Analytics storage** | Database for observability and analytical queries (`storage.analytics`). | Analytics DB / OLAP store |

### Three-Tier Data Architecture

| Tier | What | Failure behavior | Example |
|---|---|---|---|
| **1. OLTP** | Transactional writes (entities, sessions, domain events) | Operation fails → user gets error | `entity.created` in same TX |
| **2. OLAP** | Analytics store via local cache buffer | Data accumulates in cache, drains when backend recovers | `request.api`, `log.error` |
| **3. Fire-and-forget** | stdout, OTEL export | Drop, move on | Operator console, OTEL collector |
