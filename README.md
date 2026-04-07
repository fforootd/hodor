# Zitadel

[![Is It Alive?](https://isitalive.dev/api/badge/github/fforootd/hodor)](https://isitalive.dev/github/fforootd/hodor)

> **Experimental** — This is a research/prototype project exploring a reimagined identity platform architecture. It is not production-ready and APIs may change without notice. See [Zitadel](https://github.com/zitadel/zitadel) for the production IAM system.

**Identity infrastructure for humans and AI.** Open-source identity platform with unified auth, fine-grained authorization, and schema-driven user and application management — single binary by default, edge-ready at scale.

## Quick Start

```bash
# Zero-config — generates admin password, prints it
cargo run -p zitadel -- start

# With dev seed data (admin/admin123):
cargo run -p zitadel -- start --seed fixtures/seeds/frontend.yaml

# Frontend hot-reload (separate terminal, proxies API to :8080):
npm run dev -w web
```

Open `http://localhost:5173` for the hot-reload UI and `http://localhost:8080` for the API / OIDC endpoints.

## Local Development

### Backend + Frontend

```bash
# Terminal 1: Rust API with dev seed
cargo run -p zitadel -- start --seed fixtures/seeds/frontend.yaml

# Terminal 2: Vite HMR (proxies to :8080)
npm run dev -w web
```

Default dev credentials (from `fixtures/seeds/frontend.yaml`):

- `admin / admin123`
- PAT: `zitadel-dev-pat-do-not-use-in-production`

### Backend only (embedded frontend)

```bash
npm run build -w web
cargo run -p zitadel -- start -c fixtures/zitadel.dev.toml
```

### Frontend only (against existing backend)

```bash
npm run dev -w web
# Or point at a different API:
ZITADEL_API_BASE=http://localhost:8081 npm run dev -w web
```

### Reset local DB

Delete `data/zitadel.db*` files and restart.

### Seed packs

```bash
# Apply a named seed pack
cargo run -p zitadel -- seed apply -c fixtures/zitadel.dev.toml --file fixtures/seeds/frontend.yaml

# Validate without touching the DB
cargo run -p zitadel -- seed validate --file fixtures/seeds/frontend.yaml
```

Named seed packs live in [fixtures/seeds](fixtures/seeds):
- `frontend.yaml` — dev default (admin + test users + login flows)
- `e2e.yaml` — browser tests (mock OIDC providers + deterministic users)
- `minimal.yaml` — bare minimum
- `oidc-conformance.yaml` — OIDC protocol compliance

### Standalone binary

```bash
cargo run -p zitadel -- start
```

Uses SQLite at `./data/zitadel.db` with no config file. On first run, generates a random admin password and prints it. For deterministic credentials, use `--seed fixtures/seeds/frontend.yaml`.

More contributor detail lives in [docs/guides/local-development.md](docs/guides/local-development.md).
Operator-focused examples live in [docs/guides/bootstrap-recovery.md](docs/guides/bootstrap-recovery.md).

## Architecture

> See [Product Architecture](docs/architecture/product-architecture.md) for the full deployment model — how one binary scales from SQLite on a laptop to Spanner in the cloud.

- **Single binary** — REST API, login UI (Vue), admin console (Vue), account self-service (Vue). Zero external dependencies at Level 0.
- **Typed resource families** — canonical family endpoints like `/v1/users` and `/v1/apps`, with `schema_id` for writes and `schema_type` for list filtering
- **Schema-driven resources** — JSON Schema with annotations (`x-claim-mapping`, `x-user-editable`, `x-sensitive`, `x-hidden`)
- **SSO Federation** — Protocol-agnostic providers (OIDC, SAML, SCIM) with `expr`-based claim mapping
- **Embedded OpenFGA** — Zanzibar-style authorization in-process
- **Role-based storage runtime** — `storage.stateful` is canonical, with derived `read`, `kv`, `sink`, `process_cache`, and `analytics` roles. The current POC already wires SQLite and Postgres defaults through that runtime; see [Storage Architecture](docs/design/storage-architecture.md) and [Storage Implementation Status](docs/design/storage-implementation-status.md).
- **Event-sourced audit** — every mutation emitted as an event with field-level redaction
- **Import & Seed** — `POST /v1/import` for migrations, `--seed` YAML files for CI/dev

## Testing

```bash
# Rust unit + integration tests
cargo test --workspace

# Web component tests (Vitest)
npm test -w web

# E2E browser journeys (Playwright manages server lifecycle)
npm test -w browser-tests                              # all suites
npm test -w browser-tests -- --project=journeys-admin  # admin only
npm test -w browser-tests -- --project=journeys-login  # login only
npm test -w browser-tests -- --project=journeys-login-oidc  # OIDC only

# Lint
cargo fmt --check
cargo clippy --workspace -- -D warnings
npm run lint -w web
npm run typecheck -w web
```

## Configuration

Three-layer precedence: `CLI flags > env vars > TOML config > defaults`

| Setting | Env Var | Flag | Default |
|---|---|---|---|
| Stateful storage URL | `ZITADEL_STORAGE_STATEFUL_URL` | — | `sqlite://./data/zitadel.db` |
| Server port | `ZITADEL_PORT` | — | `8080` |
| Seed file | `ZITADEL_SEED_FILE` | `--seed` | — |
| External domain | `ZITADEL_EXTERNAL_DOMAIN` | — | `localhost` |

## License

AGPL-3.0 — see [LICENSE](LICENSE)
