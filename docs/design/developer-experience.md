# Developer Experience Philosophy

> Zitadel should be the easiest identity platform on earth to run.

## Core Principles

### 1. Zero-Config First Run

```bash
zitadel start
# → SQLite at ./zitadel.db (auto-created)
# → Schema auto-migrated
# → Admin bootstrapped
# → Running on http://localhost:8080
# → Admin console at http://localhost:8080/console
# → OIDC ready at http://localhost:8080/.well-known/openid-configuration
```

No YAML files. No Docker Compose. No database setup. One command, working in 60 seconds.

**What "zero-config" means in practice:**
- SQLite is the default database — no Postgres required for dev/homelab/edge
- Sensible defaults for everything (session TTL, token lifetime, rate limits)
- Database auto-migrates on startup (configurable: `check` or `skip` for production)
- Bootstrap creates default org, admin user, and default schema
- Development TLS auto-provisions if needed

**Root instance gets `*`:**
The root instance (`inst_root`) is the operator's own instance. Its owners bypass FGA checks entirely — they have implicit wildcard access to all resources across all instances. This means:
- No per-type FGA tuples needed for the operator's admin
- New resource types (endpoints, schemas, etc.) work immediately without model changes
- Customer instances still enforce strict FGA-based authorization
- This mirrors the `root` user convention in Unix — secure by default, powerful when needed

**Startup lifecycle** (see [ADR-018](../adr/018-startup-lifecycle.md)):

| `database.migrate` | Behavior |
|---|---|
| `"auto"` (default) | Run `goose up` before serving — consistent for all dialects |
| `"check"` | Version check only, fail if behind — opt-in for production PG |
| `"skip"` | No check — fastest cold-start for autoscaler pods |

For production Postgres: run `zitadel migrate` as a K8s Job, then `zitadel start` with `migrate=check`.

### 2. Pure Go Single Binary

One binary, ~30MB, cross-compiles to any platform. No runtime dependencies.

| Principle | Implementation |
|---|---|
| No CGO | `modernc.org/sqlite` (pure Go SQLite) |
| No external processes | OpenFGA embedded, no separate server |
| No build toolchain | No protobuf compile, no webpack |
| Embedded assets | UI, migrations, translations via `go:embed` |

### 3. No Operational Surprises

An IAM should be boring infrastructure. It should not:
- Write mysterious files to disk (no Parquet, no lake_data/)
- Spawn background processes the operator doesn't know about
- Require tuning obscure parameters to work correctly
- Fail silently and lose data

Every background job logs its name and completion time. Every startup step is logged. Every config value has a default that works.

## Configuration Cascade

```
CLI flags → Environment vars → Config file → Defaults
(highest priority)                          (lowest priority)
```

```toml
# zitadel.toml (optional — everything has defaults)

[server]
host = "0.0.0.0"
port = 8080
external_domain = "auth.example.com"

[database]
dialect = "sqlite"          # "sqlite" | "postgres"
url = "sqlite://./zitadel.db"

[session]
lifetime = "24h"
cookie_name = "zitadel_session"

[analytics]
backend = "oltp"            # default: query same DB

[observability]
otlp_endpoint = ""          # empty = no export
```

**Every field is optional.** If you don't provide a config file, defaults work. If you provide a partial config, only those values override.

## Testing Philosophy

SQLite is the default test database — no Docker, no setup:

| Layer | Database | Speed |
|---|---|---|
| Unit tests | In-memory SQLite | ~μs |
| Integration tests | In-memory SQLite + synthetic data | ~ms |
| Fuzz tests | In-memory SQLite | millions of iterations |
| Performance benchmarks | File-based SQLite (WAL) | sustained load |
| Cross-DB validation | Postgres (testcontainers) | ~s |

```go
// One-liner test database — dies with the test
db := testutil.NewTestDB(t)
```

## Library Leverage

Use battle-tested Go libraries instead of building from scratch:

| Library | What it gives us |
|---|---|
| `zitadel/oidc` | Production-grade OIDC provider |
| `openfga/openfga` | Zanzibar-grade authorization (embedded) |
| `go-webauthn/webauthn` | Passkey/FIDO2 authentication |
| `crewjam/saml` (fork) | SAML SP/IdP |
| `expr-lang/expr` | Policy engine expressions |
| `pquerna/otp` | TOTP/HOTP |
| `a-h/templ` | Type-safe Go templates |
| `modernc.org/sqlite` | Pure Go SQLite (no CGO) |
