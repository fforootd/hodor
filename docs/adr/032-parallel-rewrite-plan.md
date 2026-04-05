# Parallel Agent Work Plan for ADR-032 Rewrite

## Key Insight

The `zitadel-app` crate is already done and compiling. It defines every contract:
- 16 repository traits with exact method signatures
- All use case structs with typed commands/results
- `DomainEvent` enum (30+ variants)
- `PolicyInterceptor` + `EffectHook` traits
- `ActorContext`, `HookContext`, `AppError`
- `Repositories` container struct
- `ApplicationServices` wiring struct

Every remaining task depends only on these trait definitions, not on each other. This means almost everything can run in parallel.

## Dependency Graph

```
                    ┌─────────────────────────────┐
                    │     zitadel-app (DONE)       │
                    │  traits, use cases, events   │
                    └──────────────┬───────────────┘
                                   │
            ┌──────────┬───────────┼───────────┬──────────┐
            ▼          ▼           ▼           ▼          ▼
      ┌──────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
      │ CODEX-1  │ │ CODEX-2│ │CLAUDE-1│ │CLAUDE-2│ │CLAUDE-3│
      │ Repo     │ │ Repo   │ │Handler │ │ Login  │ │ Hooks  │
      │ impls    │ │ impls  │ │rewrite │ │rewrite │ │+events │
      │ (entity) │ │ (auth) │ │        │ │ +OIDC  │ │        │
      └────┬─────┘ └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘
           │            │          │           │          │
           └────────────┴──────────┴───────────┴──────────┘
                                   │
                            ┌──────▼──────┐
                            │  CLAUDE-4   │
                            │  Wiring +   │
                            │  Integration│
                            │  + Tests    │
                            └─────────────┘
```

**Parallel streams (all can start NOW):** CODEX-1, CODEX-2, CLAUDE-1, CLAUDE-2, CLAUDE-3
**Depends on all above:** CLAUDE-4 (integration wiring)

---

## Stream Assignments

### CODEX-1: Entity Repository Implementations (SQLite + Postgres)

**Why Codex:** Mechanical refactoring — take existing SQL from `retained.rs`, wrap behind trait impls. Clear input (trait signature + existing SQL), clear output (struct implementing trait). No architectural decisions needed.

**Scope:**
- `UserRepository` impl — refactor `create_user()`, `get_user()`, `find_active_user_by_identifier()`, `list_users()`, `update_user()` from `retained.rs`
- `OrgRepository` impl — refactor `create_org()`, `get_org()`, `list_org_records()`, `update_org()`, `first_org_id()`
- `GroupRepository` impl — refactor group CRUD from `retained.rs` (uses `named_resources` pattern)
- `InstanceRepository` impl — refactor `create_managed_instance()`, `get_managed_instance()`, `update_managed_instance()`, `deprovision_managed_instance()`, `resolve_route()`, domain CRUD
- `ProviderRepository` impl — refactor `insert_provider()`, `get_provider()`, `list_providers()`, `update_provider()`, `delete_provider()`
- `SchemaRepository` impl — refactor schema registry CRUD from `retained.rs`
- `SearchRepository` impl — refactor `search_all()` from `retained.rs`
- `SettingsRepository` impl — refactor `get_settings()`, `upsert_settings()` with cascade logic

**Input files to read:**
- `crates/zitadel-app/src/repo.rs` — THE contract (trait signatures + record types)
- `crates/zitadel-db/src/retained.rs` — existing SQL to refactor
- `crates/zitadel-db/src/repos/mod.rs` — function index
- `crates/zitadel-db/src/scoped.rs` — `ScopedDb`, `Dialect` enum for SQL dialect switching
- `crates/zitadel-db/src/lib.rs` — `Db` enum (SqlDb vs SpannerDb)

**Output:** New file `crates/zitadel-db/src/repo_impls/entities.rs` (or multiple files under `repo_impls/`)

**Pattern for each impl:**
```rust
pub struct SqlUserRepository {
    db: Db,
}

impl UserRepository for SqlUserRepository {
    fn create(&self, instance_id: &str, user: &UserRecord) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        Box::pin(async move {
            // Use existing SQL from retained.rs, scoped by instance_id
            let scoped = self.db.scoped(instance_id.to_string());
            // ... existing SQL INSERT logic ...
            Ok(user.clone())
        })
    }
}
```

**Prompt for Codex:**
```
Read crates/zitadel-app/src/repo.rs for the trait contracts.
Read crates/zitadel-db/src/retained.rs for the existing SQL implementations.
Read crates/zitadel-db/src/scoped.rs for ScopedDb and Dialect.

Create implementations of these repository traits from zitadel-app:
- UserRepository, OrgRepository, GroupRepository, InstanceRepository
- ProviderRepository, SchemaRepository, SearchRepository, SettingsRepository

Each implementation should:
1. Be a struct holding a Db (from zitadel-db)
2. Use existing SQL from retained.rs, adapted to return the record types from repo.rs
3. Use ScopedDb for instance-scoped queries
4. Handle dialect differences (Sqlite vs Postgres) using the existing patterns in retained.rs
5. Return BoxFuture<'_, anyhow::Result<T>> for every method

Put the implementations in crates/zitadel-db/src/repo_impls/entities.rs
Create crates/zitadel-db/src/repo_impls/mod.rs to re-export.
```

---

### CODEX-2: Auth & Session Repository Implementations

**Why Codex:** Same mechanical refactoring pattern, but focused on the auth-specific storage which has different backing stores (KvStore for sessions vs SQL for credentials).

**Scope:**
- `CredentialRepository` impl — refactor `replace_password_credential()`, `get_password_hash()` from `retained.rs`, and linked identity functions
- `PatRepository` impl — refactor PAT CRUD + `resolve_pat_token()` from `retained.rs`/`stateful.rs`
- `SessionRepository` impl — WRAP the existing `KvStore` trait (don't rewrite it). `KvStore::create_session()` → `SessionRepository::create()`
- `LoginFlowRepository` impl — WRAP the existing `KvStore` trait. `KvStore::create_login_flow()` → `LoginFlowRepository::get_flow()`
- `EventRepository` impl — refactor `append_event()` from `retained.rs`. CRITICAL: this must support transactional append (same sqlx transaction as business write)
- `OidcRepository` impl — WRAP existing OIDC traits (`ClientStore`, `AuthRequestStore`, `ClaimSource`)
- `FgaRepository` impl — WRAP existing `FgaService`
- `ActionRepository` impl — refactor action CRUD from `retained.rs`

**Input files to read:**
- `crates/zitadel-app/src/repo.rs` — trait contracts
- `crates/zitadel-db/src/retained.rs` — credential + PAT + event SQL
- `crates/zitadel-storage/src/transient/mod.rs` — `KvStore` trait (for wrapping)
- `crates/zitadel-storage/src/stateful.rs` — `DefaultStatefulStorage` (for PAT resolution)
- `crates/zitadel-oidc/src/op.rs` — OIDC traits to wrap
- `crates/zitadel-fga/src/lib.rs` — FgaService methods to wrap

**Key detail for EventRepository:**
```rust
// EventRepository::append() must be callable within a sqlx::Transaction.
// The impl needs a way to accept an optional transaction handle.
// For the POC, it can take a &Db and create its own transaction,
// BUT the use case must be able to call both repo.create() and
// events.append() within the same transaction.
//
// Solution: EventRepository impl holds a Db, and append() starts
// a transaction internally. For true transactional guarantees,
// the use case would need to pass a transaction handle.
// For the POC, separate INSERTs are acceptable with a TODO.
```

**Prompt for Codex:**
```
Read crates/zitadel-app/src/repo.rs for the trait contracts.
Read crates/zitadel-db/src/retained.rs for credential, PAT, and event SQL.
Read crates/zitadel-storage/src/transient/mod.rs for the KvStore trait.
Read crates/zitadel-storage/src/stateful.rs for stateful storage methods.
Read crates/zitadel-oidc/src/op.rs for OIDC provider traits.
Read crates/zitadel-fga/src/lib.rs for FgaService.

Create implementations of these repository traits from zitadel-app:
- CredentialRepository — uses SQL from retained.rs
- PatRepository — uses SQL from retained.rs + stateful storage
- SessionRepository — wraps KvStore::create_session/find_session_by_token/revoke_session
- LoginFlowRepository — wraps KvStore login flow methods
- EventRepository — uses append_event() SQL, must be in same transaction as state changes
- OidcRepository — wraps existing ClientStore/AuthRequestStore/ClaimSource traits
- FgaRepository — wraps FgaService::check/write/delete/read methods
- ActionRepository — uses SQL from retained.rs

Put implementations in crates/zitadel-db/src/repo_impls/auth.rs
```

---

### CLAUDE-1: API Handler Rewrite

**Why Claude Code:** Needs to understand axum routing conventions, middleware (auth_gate, InstanceContextLayer), response formatting, and how the existing handler→response translation works. Requires reading many handler files to understand patterns.

**Scope:**
1. Update `ApiState` in `crates/zitadel-api/src/lib.rs` to add `app: Arc<ApplicationServices>`
2. Rewrite every handler module to call use cases instead of DB functions:
   - `users.rs` — call `state.app.create_user.execute()`, `state.app.get_user.execute()`, etc.
   - `orgs.rs` — call org use cases
   - `groups.rs` — call group use cases
   - `instances.rs` — call instance use cases (admin-only routes)
   - `settings.rs` — call settings use cases
   - `providers.rs` — call provider use cases
   - `schemas.rs` — call schema use cases
   - `events.rs` — call event queries (EventRepository::list)
   - `analytics.rs` — unchanged (raw SQL analytics stay as-is)
   - `actions.rs` — call action use cases
   - `search.rs` — call search use case
   - `pats.rs` — call PAT use cases
3. Build `ActorContext` from the existing `Identity` in middleware
4. Map `AppError` to HTTP responses

**Each handler becomes:**
```rust
async fn create_user(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Extension(instance): Extension<InstanceContext>,
    Json(req): Json<CreateUserRequest>,
) -> Response {
    let ctx = ActorContext::from_extensions(identity, instance);
    let cmd = CreateUserCommand { ... };
    match state.app.create_user.execute(&ctx, cmd).await {
        Ok(user) => response::json_created(UserResponse::from(user)),
        Err(e) => e.into_response(),
    }
}
```

**Does NOT depend on repo impls being done** — handlers only call use cases, which are already compiled. If repos aren't ready, handlers will compile but not run.

---

### CLAUDE-2: Login Flow + OIDC Adapter Rewrite

**Why Claude Code:** Login flow is a complex state machine with UINode rendering (ADR-019), bot detection, SSO provider integration. Needs deep understanding of the flow engine and how it interacts with transient storage. OIDC has its own provider abstraction layer.

**Scope:**

**Login flow rewrite:**
1. Update `LoginState` in `crates/zitadel-login/src/lib.rs` to include `app: Arc<ApplicationServices>`
2. Rewrite `steps.rs` to call use cases:
   - `flow_create()` → calls `state.app.start_login.execute()`
   - `flow_submit()` identifier step → calls user lookup via use case
   - `flow_submit()` password step → calls `state.app.verify_password.execute()` then `state.app.issue_session.execute()`
   - `flow_submit()` SSO step → calls `state.app.link_identity.execute()`
3. Preserve UINode rendering (server-driven login per ADR-019)
4. Preserve bot detection event emission
5. Rewrite `sso.rs` to call `LinkIdentity` use case after SSO callback

**OIDC adapter rewrite:**
1. Rewrite `ZitadelOpStore` in `crates/zitadel-oidc/src/adapters.rs` to use `OidcRepository`
2. `ClientStore::find_client()` → `repos.oidc.find_client()`
3. `AuthRequestStore::create_auth_request()` → `repos.oidc.create_auth_request()`
4. `ClaimSource::load_user_claims()` → `repos.oidc.load_user_claims()`
5. Token issuance calls `IssueToken` use case (may need to be added to zitadel-app)

---

### CLAUDE-3: Hook Pipeline Engine + Event Wiring

**Why Claude Code:** Requires understanding the `actions` table schema, trigger expression evaluation, hook ordering semantics, and how events flow from use cases to workers.

**Scope:**

**Hook pipeline engine:**
1. Create `crates/zitadel-app/src/hook_engine.rs` — loads hook definitions from `ActionRepository`
2. Implement trigger expression parser/evaluator for `trigger_expr` field
3. Build `HookPipeline` at startup from DB-stored action records
4. Implement `ActionPolicyInterceptor` — a `PolicyInterceptor` impl that evaluates action rules
5. Implement `ActionEffectHook` — an `EffectHook` impl that runs action side effects
6. Wire hooks into middleware (request/auth phases) and use cases (pre-validate/pre-commit/post-commit)

**Event wiring:**
1. Ensure `EventRepository::append()` is called within use case transactions
2. Add event consumption worker to `crates/zitadel-server/src/jobs.rs`:
   - New job type: `event_consumer` — polls events table by cursor
   - For each unconsumed event, runs `post_event_effects`
3. Add `shipped_at` tracking so events aren't processed twice
4. Wire PostCommit effects into `UseCaseRunner::run()` (already partially done)

**Observability instrumentation:**
1. Verify `#[tracing::instrument]` on all use cases emits correct fields
2. Add hook execution spans
3. Verify existing `ObservabilityLayer` captures use case spans

---

### CLAUDE-4: Integration Wiring + Tests (AFTER streams 1-3 merge)

**Why last:** This assembles all the pieces and can only run once the repo impls, handlers, and login/OIDC rewrites are done.

**Scope:**

**Server wiring:**
1. Update `crates/zitadel-server/src/lib.rs` startup sequence:
   - Build repo implementations from `Db` + `StorageRuntime`
   - Build `Repositories` container
   - Build `HookPipeline` from `ActionRepository`
   - Build `ApplicationServices::new(repos, hooks)`
   - Inject into `ApiState` and `LoginState`
2. Update `crates/zitadel-server/src/routing.rs` for new ApiState shape
3. Preserve existing startup steps: migrations, bootstrap, seed, FGA init

**Tests:**
1. Use case unit tests with mock repositories (in `zitadel-app`)
2. Repository contract tests (same assertions on SQLite + Postgres)
3. Hook pipeline tests (interceptor ordering, deny, step-up)
4. Integration smoke test: startup → create user → login → session

---

## Codex Task Specs

### Codex Task 1: Entity Repository Impls

```markdown
## Task: Implement entity repository traits

### Context
The `zitadel-app` crate defines repository traits in `crates/zitadel-app/src/repo.rs`.
The existing SQL implementations live in `crates/zitadel-db/src/retained.rs`.
Your job is to create struct implementations of these traits that use the existing SQL.

### Contract files (READ THESE FIRST)
- `crates/zitadel-app/src/repo.rs` — trait definitions and record types
- `crates/zitadel-db/src/retained.rs` — existing SQL functions
- `crates/zitadel-db/src/repos/mod.rs` — function index
- `crates/zitadel-db/src/scoped.rs` — ScopedDb struct and Dialect enum
- `crates/zitadel-db/src/lib.rs` — Db enum definition

### What to implement
Create `crates/zitadel-db/src/repo_impls/mod.rs` and:
- `crates/zitadel-db/src/repo_impls/user_repo.rs` — `impl UserRepository for SqlUserRepository`
- `crates/zitadel-db/src/repo_impls/org_repo.rs` — `impl OrgRepository for SqlOrgRepository`
- `crates/zitadel-db/src/repo_impls/group_repo.rs` — `impl GroupRepository for SqlGroupRepository`
- `crates/zitadel-db/src/repo_impls/instance_repo.rs` — `impl InstanceRepository for SqlInstanceRepository`
- `crates/zitadel-db/src/repo_impls/provider_repo.rs` — `impl ProviderRepository for SqlProviderRepository`
- `crates/zitadel-db/src/repo_impls/schema_repo.rs` — `impl SchemaRepository for SqlSchemaRepository`
- `crates/zitadel-db/src/repo_impls/settings_repo.rs` — `impl SettingsRepository for SqlSettingsRepository`
- `crates/zitadel-db/src/repo_impls/search_repo.rs` — `impl SearchRepository for SqlSearchRepository`

### Pattern
```rust
use zitadel_app::repo::{BoxFuture, UserRepository, UserRecord, ListParams, ListResult};
use crate::Db;

pub struct SqlUserRepository { pub db: Db }

impl UserRepository for SqlUserRepository {
    fn create(&self, instance_id: &str, user: &UserRecord) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        Box::pin(async move {
            let scoped = self.db.scoped(instance_id.to_string());
            // Reuse SQL from retained.rs::create_user()
            Ok(user.clone())
        })
    }
    // ... implement all trait methods
}
```

### Rules
- Each struct holds `pub db: Db`
- All methods return `BoxFuture<'_, anyhow::Result<T>>`
- Reuse existing SQL from `retained.rs` — don't write new queries
- Handle Sqlite/Postgres dialect differences using `scoped.dialect()`
- Map between `retained.rs` record types and `repo.rs` record types via From impls
- Add `pub mod repo_impls;` to `crates/zitadel-db/src/lib.rs`
- Add `zitadel-app = { workspace = true }` to `crates/zitadel-db/Cargo.toml`

### Verification
- `cargo check -p zitadel-db` must pass
```

### Codex Task 2: Auth & Session Repository Impls

```markdown
## Task: Implement auth-path repository traits

### Context
Same as Task 1, but these repos wrap different storage backends:
- Credentials + PATs + Events + Actions use SQL (`retained.rs`)
- Sessions + LoginFlows wrap the existing `KvStore` trait (`transient/mod.rs`)
- OIDC wraps existing OIDC provider traits (`op.rs`)
- FGA wraps existing `FgaService` (`fga/lib.rs`)

### Contract files (READ THESE FIRST)
- `crates/zitadel-app/src/repo.rs` — trait definitions
- `crates/zitadel-db/src/retained.rs` — credential + PAT + event SQL
- `crates/zitadel-storage/src/transient/mod.rs` — KvStore trait definition
- `crates/zitadel-storage/src/stateful.rs` — stateful storage (PAT resolution)
- `crates/zitadel-oidc/src/op.rs` — OIDC provider traits
- `crates/zitadel-fga/src/lib.rs` — FgaService

### What to implement
- `crates/zitadel-db/src/repo_impls/credential_repo.rs` — SQL-backed
- `crates/zitadel-db/src/repo_impls/pat_repo.rs` — SQL-backed
- `crates/zitadel-db/src/repo_impls/event_repo.rs` — SQL-backed, transactional append
- `crates/zitadel-db/src/repo_impls/action_repo.rs` — SQL-backed
- `crates/zitadel-db/src/repo_impls/session_repo.rs` — wraps KvStore
- `crates/zitadel-db/src/repo_impls/login_flow_repo.rs` — wraps KvStore
- `crates/zitadel-db/src/repo_impls/oidc_repo.rs` — wraps OIDC traits
- `crates/zitadel-db/src/repo_impls/fga_repo.rs` — wraps FgaService

### Key details
- SessionRepository wraps KvStore — the struct holds `Arc<DefaultTransientStorage>`
- EventRepository must include a TODO comment about transactional append
- FgaRepository wraps FgaService.check(), .write(), .delete(), .read()
- OidcRepository wraps the existing ClientStore/AuthRequestStore/ClaimSource traits

### Verification
- `cargo check -p zitadel-db` must pass
```

---

## Execution Order

```
Time ──────────────────────────────────────────────────────►

Phase 1 (parallel, no dependencies):
  CODEX-1 ████████████████████  Entity repo impls
  CODEX-2 ████████████████████  Auth repo impls
  CLAUDE-1 ███████████████████  Handler rewrite
  CLAUDE-2 ███████████████████  Login + OIDC rewrite
  CLAUDE-3 ███████████████████  Hook engine + events

Phase 2 (after Phase 1 merges):
  CLAUDE-4 ████████████████████  Server wiring + integration + tests
```

All Phase 1 streams work against the frozen `zitadel-app` trait contracts.
They don't need each other's output to compile — only to run end-to-end.

Phase 2 assembles all pieces, wires them together, and verifies the system runs.

---

## Branch Strategy

```
rust-migration (current)
  ├── codex/repo-impls-entities     (CODEX-1)
  ├── codex/repo-impls-auth         (CODEX-2)
  ├── claude/handler-rewrite        (CLAUDE-1)
  ├── claude/login-oidc-rewrite     (CLAUDE-2)
  ├── claude/hook-engine            (CLAUDE-3)
  └── claude/integration-wiring     (CLAUDE-4, after merging above)
```

Each branch should be mergeable independently since they touch different files:
- CODEX-1+2: `crates/zitadel-db/src/repo_impls/` (new directory)
- CLAUDE-1: `crates/zitadel-api/src/*.rs`
- CLAUDE-2: `crates/zitadel-login/src/*.rs` + `crates/zitadel-oidc/src/adapters.rs`
- CLAUDE-3: `crates/zitadel-app/src/hook_engine.rs` + `crates/zitadel-server/src/jobs.rs`
- CLAUDE-4: `crates/zitadel-server/src/lib.rs` + `crates/zitadel-server/src/routing.rs`

Merge conflicts should be minimal since each stream owns different files.

---

## Risk Mitigation

**Risk: Record type mismatches**
The `repo.rs` record types were defined fresh, not copied from `retained.rs`.
Fields might not align perfectly.
**Mitigation:** CODEX tasks include creating `From<RetainedRecord> for AppRecord` impls.

**Risk: EventRepository transactional append**
True same-TX event append requires passing a transaction handle through the use case.
The current trait signature doesn't support this.
**Mitigation:** POC uses separate INSERTs with a TODO. The EventRepository impl
wraps `append_event()` as a standalone call. Transactional guarantee is deferred
to a follow-up that adds a `TransactionalEventRepository` variant.

**Risk: KvStore wrapper mismatch**
The `SessionRepository` and `LoginFlowRepository` record types in `repo.rs` may not
match the KvStore's `SessionRecord` and `LoginFlowRuntimeState` exactly.
**Mitigation:** CODEX-2 includes `From` trait impls to map between the two.

**Risk: Merge conflicts in Cargo.toml**
Multiple branches adding dependencies to `zitadel-db/Cargo.toml`.
**Mitigation:** Only CODEX branches modify `zitadel-db/Cargo.toml` (adding `zitadel-app`
dependency). CLAUDE branches add `zitadel-app` to their own crate Cargo.tomls.
