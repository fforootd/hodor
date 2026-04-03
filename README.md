# Zitadel

[![Is It Alive?](https://isitalive.dev/api/badge/github/fforootd/hodor)](https://isitalive.dev/github/fforootd/hodor)

> ⚠️ **Experimental** — This is a research/prototype project exploring a reimagined identity platform architecture. It is not production-ready and APIs may change without notice. See [Zitadel](https://github.com/zitadel/zitadel) for the production IAM system.

**Identity infrastructure for humans and AI.** Open-source identity platform with unified auth, fine-grained authorization, and schema-driven user and application management — single binary by default, edge-ready at scale.

## Quick Start

```bash
# Fresh clone: install web deps, build embedded assets, start Rust + Vite
make dev
```

Open `http://localhost:5173` for the hot-reload UI and `http://localhost:8080` for the Rust API / OIDC endpoints.

## Local Development

### Fresh clone

```bash
make dev
```

Default local credentials:

- `admin / admin123`
- PAT: `zitadel-dev-pat-do-not-use-in-production`

### Backend-only dev

```bash
make dev-embed
```

This builds `web/dist` and serves the embedded frontend directly from the Rust binary on `http://localhost:8080`.

### Frontend dev

```bash
# Start the backend in another terminal
make dev-embed

# Then run Vite HMR
make dev-web
```

To point Vite at a different API:

```bash
ZITADEL_API_BASE=http://localhost:8081 make dev-web
```

### Reset local DB

```bash
make dev-reset
```

This deletes local SQLite files under `./data/`, refuses to run against non-SQLite `ZITADEL_STORAGE_STATEFUL_URL` values, and then boots the default frontend seed pack again.

### Reseed data

```bash
# Default pack used by make dev
make dev-seed

# Alternate packs
make dev-seed SEED=minimal
make dev-seed SEED=e2e
```

Named seed packs live in [fixtures/seeds](fixtures/seeds). Validate one without touching the DB:

```bash
cargo run -p zitadel -- seed validate --file fixtures/seeds/frontend.yaml
```

### Standalone binary

The zero-config path still works:

```bash
cargo run -p zitadel -- server start
```

That uses SQLite at `./data/zitadel.db` with no required config file. For deterministic local credentials such as `admin / admin123`, prefer `make dev` or run with `fixtures/zitadel.dev.toml`, which applies the frontend seed pack on startup.

For an explicit migration + bootstrap pass before serving, use:

```bash
cargo run -p zitadel -- migrate -c fixtures/zitadel.dev.toml --bootstrap
```

The current Rust CLI does not yet expose the old `bootstrap admin` or `recover admin` subcommands. The current operator flow and its limits are documented in [docs/guides/bootstrap-recovery.md](docs/guides/bootstrap-recovery.md).

More contributor detail lives in [docs/guides/local-development.md](docs/guides/local-development.md).
Operator-focused examples live in [docs/guides/bootstrap-recovery.md](docs/guides/bootstrap-recovery.md).

## Architecture

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
# Run Rust tests
make test

# Run the local Rust quality gate
make rust-check

# Run web unit tests
make test-web

# Run browser smoke or full suites
make e2e-smoke
make test-e2e
```

## Configuration

Three-layer precedence: `CLI flags > env vars > TOML config > defaults`

| Setting | Env Var | Flag | Default |
|---|---|---|---|
| Stateful storage URL | `ZITADEL_STORAGE_STATEFUL_URL` | — | `sqlite://./data/zitadel.db` |
| Server port | `ZITADEL_PORT` | — | `8080` |
| Mock OIDC | `ZITADEL_MOCK_OIDC` | `--mock-oidc` | `false` |
| Seed file | `ZITADEL_SEED_FILE` | `--seed` | — |
| External domain | `ZITADEL_EXTERNAL_DOMAIN` | — | `localhost` |

## License

AGPL-3.0 — see [LICENSE](LICENSE)
