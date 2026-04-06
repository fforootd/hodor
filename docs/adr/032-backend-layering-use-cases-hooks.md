# ADR-032: Backend Layering, Use Cases, and Hook Pipeline

**Status:** Proposed
**Date:** 2026-04-05
**Depends-on:** ADR-005 (Unified Data Model), ADR-010 (Three-Tier Data Architecture), ADR-019 (Server-Driven Login), ADR-020 (Authorization Model), ADR-034 (Multi-Tenancy via Instance Boundaries), ADR-029 (Control Plane and Auth Data Plane)
**Related:** [Architecture Overview](../architecture/overview.md), [Storage Architecture](../design/storage-architecture.md)

## Context

The current backend has handlers calling database functions directly. Business logic is split between the handler layer and `retained.rs` (the DB layer) with no intermediate service or domain layer. Authorization (FGA) is injected but unused in handlers. Events are emitted via `append_event()` but are not transactional with business writes. The actions table stores hook definitions but no execution machinery exists.

This creates several problems:

- **Testing requires a live database** — handlers import DB functions directly, so there is no mock boundary.
- **Logic duplication** — the same business operation (e.g., password verification, user creation) may be called from API handlers, login flows, and OIDC adapters, each with slightly different orchestration.
- **Authorization is implicit** — handlers assume that authentication implies authorization; FGA is not consulted.
- **Events are unreliable** — `append_event()` is a separate INSERT, not in the same transaction as the state change it describes.
- **No extensibility** — there is no way to attach custom policies, rate limits, or side effects to business operations.

The storage runtime (stateful/transient/analytics role model), KvStore trait, OIDC provider traits, password Swapper, and transient Sink are well-designed and do not need restructuring. The problem is the layer between transport and storage.

## Decision

### 1. Layered Architecture

The backend is organized into these layers, top to bottom:

```
Transport Adapters
  HTTP handlers, login routes, OIDC endpoints, CLI commands
  Only: parse input → build command → call use case → map result to response

Instance + Caller Context Resolution
  Middleware resolves instance_id, actor identity, capabilities

AuthN
  Token/session/cookie verification → Identity

AuthZ
  FGA permission check + operator_admin capability bypass → authorized context

Application Layer (Use Cases)
  Command handlers that orchestrate business operations
  Own all business logic, validation, and domain event emission

Domain Policies
  Invariants, defaults, feature gates
  Called by use cases, never by handlers or storage

Hook Pipeline
  Attachable policy interceptors and effect hooks at defined phases
  May deny, throttle, mutate context, or trigger side effects

Repository Ports
  Trait-based persistence contracts per domain
  Use cases depend on traits, not on sqlx or backend-specific types

Event / Outbox
  Domain events persisted in the same DB transaction as state changes
  Workers consume events for async work

Storage Runtime
  Role selection: stateful, read, kv, sink, analytics (preserved as-is)

Storage Drivers
  SQLite, Postgres, Spanner implementations of repository traits
```

**Rules:**

- No business logic in transport adapters or storage drivers.
- Use cases are the single source of orchestration for every business operation.
- Core invariants never depend on optional hooks. Hooks extend or constrain behavior; they do not own it.
- Repository traits define behavior. Drivers implement per-backend.
- `operator_admin` is a deployment capability checked in the authZ layer as a bypass, not in storage filtering or handlers.

### 2. Use Cases

Each business operation is a use case with a typed command and result:

```rust
pub trait UseCase: Send + Sync {
    type Command: Send;
    type Result: Send;
    type Error: Send;

    async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: Self::Command,
    ) -> Result<Self::Result, Self::Error>;
}
```

A use case execution follows this sequence:

1. Receive typed command from transport adapter
2. Run pre-validate interceptors (feature gates, per-instance policy)
3. Validate domain invariants and apply defaults
4. Run pre-commit interceptors (command-specific policy, step-up auth)
5. Call repository ports (within a transaction)
6. Append domain event (in the same transaction)
7. Commit transaction
8. Run post-commit effect hooks (notifications, webhooks, FGA sync)

Transport adapters (API handlers, login handlers, OIDC adapters, CLI commands) only:
- Parse input into the command type
- Build actor context from middleware-provided identity and instance
- Call the use case
- Map the result or error to the transport response format

Login use cases produce UINode trees per ADR-019. OIDC adapters call use cases for token issuance and consent. The same use case is callable from any transport with identical behavior.

### 3. Hook Pipeline

Hooks are split into two contracts with different execution semantics:

**PolicyInterceptor** — synchronous, may block execution:

```rust
pub trait PolicyInterceptor: Send + Sync {
    async fn intercept(
        &self,
        phase: HookPhase,
        ctx: &HookContext,
    ) -> InterceptResult;
}

pub enum InterceptResult {
    Continue,
    Deny(DenyReason),
    RequireStepUp(StepUpKind),
    MutateContext(ContextPatch),
}
```

**EffectHook** — asynchronous, fire-after-commit, cannot block:

```rust
pub trait EffectHook: Send + Sync {
    async fn on_event(
        &self,
        phase: HookPhase,
        ctx: &HookContext,
        event: &DomainEvent,
    );
}
```

**Six hook phases:**

| Phase | Layer | Contract | Timing | Examples |
|-------|-------|----------|--------|----------|
| `Request` | Middleware | PolicyInterceptor | Before routing | HTTP rate limiting, IP blocking, geo-fencing |
| `Auth` | After authN | PolicyInterceptor | After identity resolution, before authZ | OTP attempt throttling, provider-login gating, session freshness |
| `PreValidate` | Use case entry | PolicyInterceptor | Before domain validation | Per-instance/org feature gates, billing checks |
| `PreCommit` | Use case, after validation | PolicyInterceptor | After validation, before persist | Command-specific policy, step-up auth requirements |
| `PostCommit` | Use case, after TX commit | EffectHook | After successful persist | Notifications, webhook delivery, FGA hierarchy sync |
| `PostEvent` | Worker consumption | EffectHook | After event is consumed by a worker | Downstream provisioning, analytics enrichment, external integration |

**Execution rules:**
- Interceptors run in priority order. First `Deny` or `RequireStepUp` short-circuits.
- Effect hooks run best-effort. Failures are logged but do not roll back the committed operation.
- `fail_open` on an interceptor means: if the interceptor itself errors, treat as `Continue`.
- Hook definitions are loaded from the existing `actions` table.
- Core invariants never depend on hooks being present. A use case with zero hooks must produce correct behavior.

### 4. Repository Ports

Repository traits define persistence contracts per domain. Use cases depend on traits; implementations are selected at startup based on the storage runtime configuration.

**Required ports:**

| Port | Domain | Notes |
|------|--------|-------|
| `UserRepository` | User CRUD, activation, metadata | |
| `OrgRepository` | Organization CRUD, membership | |
| `CredentialRepository` | Password set/verify, linked identity CRUD | Uses password Swapper internally |
| `SessionRepository` | Session create/find/revoke | Wraps existing KvStore |
| `InstanceRepository` | Instance lifecycle, domain resolution | |
| `ProviderRepository` | External auth provider CRUD | |
| `LoginFlowRepository` | Login flow state machine | Wraps existing KvStore |
| `OidcRepository` | Client metadata, auth requests | Wraps existing OIDC traits |
| `EventRepository` | Append event (transactional), query events | Must support `append_in_tx` |
| `SettingsRepository` | Cascading config (instance → org → app) | |
| `FgaRepository` | Permission check, relation management | Wraps existing FgaService |
| `SchemaRepository` | Schema registry CRUD | |
| `GroupRepository` | Group CRUD, membership, app association | |
| `PatRepository` | Personal access token CRUD | |
| `SearchRepository` | Cross-entity search | |

**Implementation rule:** Repository implementations for SQLite, Postgres, and Spanner are built by refactoring existing SQL from `retained.rs` into trait implementations. The SQL already exists; it moves behind trait boundaries.

**Preserved as-is:**
- `StorageRuntime` and role derivation
- `KvStore` trait and all implementations
- `Sink` trait and implementations
- OIDC `ClientStore` / `AuthRequestStore` / `ClaimSource` / `KeyStore` traits
- Password `Swapper`
- `ScopedDb` and dialect helpers

### 5. Domain Events

Domain events are typed and emitted within the same database transaction as the state change they describe (per ADR-010):

```rust
pub enum DomainEvent {
    UserCreated { user_id, org_id, schema_type, actor_id },
    UserUpdated { user_id, fields_changed, actor_id },
    UserDeactivated { user_id, actor_id },
    PasswordSet { user_id, actor_id },
    IdentityLinked { user_id, provider_id, external_id, actor_id },
    SessionStarted { session_id, user_id, auth_methods },
    SessionRevoked { session_id, actor_id },
    LoginFlowCompleted { flow_id, user_id, outcome },
    TokenIssued { token_id, client_id, subject, grant_type },
    InstanceCreated { instance_id, parent_instance_id, owner_org_id },
    InstanceUpdated { instance_id, fields_changed },
    InstanceDeprovisioned { instance_id, actor_id },
    OrgCreated { org_id, name, actor_id },
    GroupCreated { group_id, org_id, actor_id },
    AppCreated { app_id, group_id, protocol, actor_id },
    SettingsUpdated { scope, key, actor_id },
    ProviderConfigured { provider_id, protocol, actor_id },
    SchemaRegistered { schema_id, schema_type, actor_id },
    // ... one variant per business operation
}
```

`EventRepository::append_in_tx()` inserts the event into the existing `events` table within the same `sqlx::Transaction` as the business write. The events table schema is unchanged.

**Same table, different write paths:** Domain events are written transactionally by use cases. Observability events are written by the existing analytics pipeline (ObservabilityLayer → SQLite cache → drainer). Both end up in the `events` table. The `event_type` naming convention distinguishes them (e.g., `user.created` for domain vs `log.info` for observability).

### 6. Observability Integration

Use cases are instrumented with `#[tracing::instrument]` to participate in the existing observability pipeline:

- Each use case invocation creates a tracing span with structured fields (event_type, category, aggregate_type)
- The existing `ObservabilityLayer` captures these spans as `StructuredRecord`s
- Records flow to stdout, analytics, and OTEL sinks via the existing pipeline
- Use case spans are children of request spans (automatic trace context propagation)
- Hook execution produces its own tracing spans for auditability

The existing analytics pipeline (SQLite cache → drainer → analytics DB), stream routing, PII redaction, and OTEL log export are preserved. OTEL is extended to include trace export alongside log export.

### 7. Authorization in Use Cases

FGA authorization checks move from "absent in handlers" to "explicit in use cases":

- Use cases call `FgaRepository::check()` before performing mutations
- The three-layer FGA model (core, modules, custom per ADR-020) is preserved
- Root-management use cases authorize against the root FGA store
- Child-instance use cases authorize against the child FGA store
- `operator_admin` is checked as a capability bypass in the authZ layer before the use case is entered

### 8. Workers and Reconcilers

The existing job runtime (`job_runtime.rs`) is preserved. New job types are added:

- **Notification worker** — consumes PostCommit events, delivers via configured channels
- **Webhook dispatcher** — consumes PostCommit events, delivers to registered endpoints
- **FGA reconciler** — consumes hierarchy-affecting events, updates FGA relations
- **PostEvent effect runner** — consumes events from the outbox, runs PostEvent effect hooks

Workers use cursor-based event consumption from the `events` table.

## Consequences

**Positive:**
- Business logic is centralized in use cases, testable with fake repositories
- Authorization is explicit and enforced
- Events are transactional and reliable
- Hooks provide extensibility without coupling
- The same use case serves API, login, OIDC, and CLI transports identically

**Negative:**
- Significant restructuring of handler and login code
- Repository trait explosion (15+ traits)
- Additional indirection compared to the current handler → DB model

**Neutral:**
- Storage runtime, KvStore, OIDC traits, password Swapper, and observability pipeline are unchanged
- The events table schema is unchanged
- The FGA model is unchanged
- Wire-format compatibility is not a goal for this POC
