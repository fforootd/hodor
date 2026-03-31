# Zitadel

[![Is It Alive?](https://isitalive.dev/api/badge/github/fforootd/hodor)](https://isitalive.dev/github/fforootd/hodor)

> ⚠️ **Experimental** — This is a research/prototype project exploring a reimagined identity platform architecture. It is not production-ready and APIs may change without notice. See [Zitadel](https://github.com/zitadel/zitadel) for the production IAM system.

**Identity infrastructure for humans and AI.** Open-source identity platform with unified auth, fine-grained authorization, and schema-driven user and application management — in a single binary.

## Quick Start

```bash
# Fresh clone: install web deps, build embedded assets, start Go + Vite
make dev
```

Open `http://localhost:5173` for the hot-reload UI and `http://localhost:8080` for the Go API / OIDC endpoints.

## Local Development

### Fresh clone

```bash
make dev
```

Default local credentials:

- `admin / admin123`
- PAT: `zitadel-dev-pat-do-not-use-in-production`

### Go-only dev

```bash
make dev-go
```

This reuses embedded web assets when they already exist. If you have not built them yet, run `make webdist` or `make dev` once first.

### Frontend dev

```bash
# Start the backend in another terminal
make dev-go

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

This deletes local SQLite files under `./data/`, refuses to run against non-SQLite `ZITADEL_DATABASE_URL` values, and then boots the default frontend seed pack again.

### Reseed data

```bash
# Default pack used by make dev
make dev-seed

# Alternate packs
make dev-seed SEED=minimal
make dev-seed SEED=e2e
```

Named seed packs live in [`fixtures/seeds`](/Users/ffo/git/hodor/zitadel/fixtures/seeds). Validate one without touching the DB:

```bash
go run ./cmd/zitadel seed validate --file fixtures/seeds/frontend.yaml
```

### Standalone Go binary

The zero-config path still works:

```bash
go run ./cmd/zitadel start
```

That uses SQLite at `./data/zitadel.db` with no required config file.

More contributor detail lives in [docs/guides/local-development.md](docs/guides/local-development.md).

## Architecture

- **Single binary** — REST API, login UI (Vue), admin console (Vue), account self-service (Vue)
- **Typed resource families** — canonical family endpoints like `/v1/users` and `/v1/apps`, with `schema_id` for writes and `schema_type` for list filtering
- **Schema-driven resources** — JSON Schema with annotations (`x-claim-mapping`, `x-user-editable`, `x-sensitive`, `x-hidden`)
- **SSO Federation** — Protocol-agnostic providers (OIDC, SAML, SCIM) with `expr`-based claim mapping
- **Embedded OpenFGA** — Zanzibar-style authorization in-process
- **SQLite + Postgres** — zero-config dev, production-ready with Postgres
- **Event-sourced audit** — every mutation emitted as an event with field-level redaction
- **Import & Seed** — `POST /v1/import` for migrations, `--seed` YAML files for CI/dev

## Testing

```bash
# Run all tests
go test ./...

# Run with verbose output
go test -v ./...

# Run fuzz tests
go test -fuzz FuzzParseIDTokenClaims ./internal/login/ -fuzztime 30s
go test -fuzz FuzzMapClaims ./internal/login/ -fuzztime 30s
```

## Configuration

Three-layer precedence: `CLI flags > env vars > TOML config > defaults`

| Setting | Env Var | Flag | Default |
|---|---|---|---|
| Database URL | `ZITADEL_DATABASE_URL` | — | `sqlite://./data/zitadel.db` |
| Server port | `ZITADEL_PORT` | — | `8080` |
| Mock OIDC | `ZITADEL_MOCK_OIDC` | `--mock-oidc` | `false` |
| Seed file | `ZITADEL_SEED_FILE` | `--seed` | — |
| External domain | `ZITADEL_EXTERNAL_DOMAIN` | — | `localhost` |

## License

AGPL-3.0 — see [LICENSE](LICENSE)
