# Developer Experience Philosophy

> Zitadel should be the easiest identity platform on earth to run.

## Core Principles

### 1. Zero-Config First Run

```bash
cargo run -p zitadel -- server start
# → SQLite at ./data/zitadel.db (auto-created)
# → Schema auto-migrated
# → Default org/admin record bootstrapped
# → Running on http://localhost:8080
# → Admin console at http://localhost:8080/console
# → OIDC ready at http://localhost:8080/.well-known/openid-configuration
```

No YAML files. No Docker Compose. No database setup. One command, working in 60 seconds.

**What "zero-config" means in practice:**
- SQLite is the default database — no Postgres required for dev/homelab/edge
- Sensible defaults for everything (session TTL, token lifetime, rate limits)
- Database auto-migrates on startup (configurable: `check` or `skip` for production)
- Bootstrap creates the default org and admin user record
- Development TLS auto-provisions if needed

For deterministic local credentials such as `admin / admin123`, use a seed file like `fixtures/zitadel.dev.toml` or the `make dev` workflow.

**Root instance gets `*`:**
The root instance (`inst_root`) is the operator's own instance. Its owners bypass FGA checks entirely — they have implicit wildcard access to all resources across all instances. This means:
- No per-type FGA tuples needed for the operator's admin
- New resource types (endpoints, schemas, etc.) work immediately without model changes
- Customer instances still enforce strict FGA-based authorization
- This mirrors the `root` user convention in Unix — secure by default, powerful when needed

**Startup lifecycle** (see [ADR-018](../adr/018-startup-lifecycle.md)):

| `storage.stateful.migrate` | Behavior |
|---|---|
| `"auto"` (default) | Run the built-in migration runner before serving |
| `"check"` | Version check only, fail if behind — opt-in for production PG |
| `"skip"` | No check — fastest cold-start for autoscaler pods |

For production Postgres: run `zitadel db migrate` as a K8s Job, then `zitadel server start` with `storage.stateful.migrate=check`.

### 2. Single Rust Binary

One Rust binary, cross-compiles to common platforms, and keeps Level 0 local startup SQLite-first with no required external services.

| Principle | Implementation |
|---|---|
| SQLite-first | `sqlx` talks to SQLite by default, Postgres when configured |
| No external processes (Level 0) | local defaults and in-process services keep startup simple |
| Embedded assets | `rust-embed` serves `web/dist` from the binary |
| Self-contained CLI/config | `clap` + `figment` keep startup, config, subcommands, and remote client flows in one binary |

> **Scaling beyond a single process:** As deployments grow, the same binary connects to external infrastructure — Postgres, Redis/Valkey, and dedicated sink backends — without code changes. The runtime derives `read`, `kv`, `sink`, `process_cache`, and `analytics` roles from `storage.stateful`, and advanced deployments can override individual roles under `storage.*`. See [Storage Architecture](storage-architecture.md) for the full progression.
>
> For the current POC reality check, see [Storage Implementation Status](storage-implementation-status.md). The edge-first storage split described in the architecture docs is not fully implemented end-to-end yet.

### 3. No Operational Surprises

An IAM should be boring infrastructure. It should not:
- Write mysterious files to disk (no Parquet, no lake_data/)
- Spawn background processes the operator doesn't know about
- Require tuning obscure parameters to work correctly
- Fail silently and lose data

Every background job logs its name and completion time. Every startup step is logged. Every config value has a default that works.

## CLI Shape

`zitadel` is one binary with two execution modes:

- Local operator commands: `server`, `db`, `seed`, `config`, `openapi`
- Remote client commands: `auth`, `users`, `schemas`, `api`

Operator commands read server runtime config. Remote client commands read a separate client profile file and auth state:

- Server config: `./zitadel.toml` or `-c/--config`
- Client profiles: `$XDG_CONFIG_HOME/zitadel/client.toml`
- Client auth state: `$XDG_STATE_HOME/zitadel/<profile>.json`

Compatibility aliases such as `zitadel start` and `zitadel migrate` still exist, but the namespaced form is canonical.

## Configuration Cascade

```
Server commands: CLI flags → Environment vars → Config file → Defaults
Remote commands: CLI flags → Environment vars → Client profile → Defaults
```

```toml
# zitadel.toml (optional — everything has defaults)
# Full reference: ../../zitadel.reference.toml

[server]
port = 8080
external_domain = "auth.example.com"

[storage.stateful]
url = "sqlite://./data/zitadel.db"

[observability]
cache_path = "./data/zitadel-cache.db"

[observability.sinks.otel]
endpoint = ""               # empty = no OTEL export

[observability.streams.request]
mode = "sampled"
sample_rate = 0.01
```

`zitadel.reference.toml` is the full server-runtime reference. It is intentionally separate from remote CLI profiles so operators do not have to mix server settings, desktop login state, and tokens in one file.

**Every field is optional.** If you don't provide a config file, defaults work. If you provide a partial config, only those values override.

For remote CLI use, the raw-payload path is first-class:

- `zitadel auth login`
- `zitadel auth token set --token-value "$TOKEN"`
- `zitadel users create --json @user.json`
- `zitadel api call POST /v1/users --json @payload.json --dry-run`
- `zitadel schemas inspect --meta`
- `zitadel openapi export`

## Testing Philosophy

SQLite is the default test database — no Docker, no setup:

| Layer | Database | Speed |
|---|---|---|
| Unit tests | In-memory SQLite | ~μs |
| Integration tests | In-memory SQLite + synthetic data | ~ms |
| Fuzz tests | In-memory SQLite | millions of iterations |
| Performance benchmarks | File-based SQLite (WAL) | sustained load |
| Scheduled DB perf trends | File-based SQLite (WAL) + Postgres 18 service container | daily CI |
| Cross-DB validation | Postgres (testcontainers) | ~s |

```rust
// One-liner test database — dies with the test
let db = zitadel_db::Db::open("").await?;
```

The default fast path is `cargo nextest run --workspace` when `cargo-nextest` is installed; local `make test` and CI fall back to `cargo test --workspace` only when nextest is unavailable.

For full-router integration coverage, prefer the shared `crates/zitadel-testkit` helpers over ad hoc per-crate harnesses.

## Library Leverage

Use battle-tested Rust crates instead of building from scratch:

| Crate | What it gives us |
|---|---|
| `axum` | HTTP routing and middleware |
| `sqlx` | SQLite/Postgres access with one async query layer |
| `clap` | CLI parsing for local operator commands and remote client workflows |
| `serde` + `figment` | config loading and serialization |
| `rust-embed` | embedded frontend assets from `web/dist` |
| `jsonwebtoken` | JWT signing and verification |
| `argon2` | password hashing |
