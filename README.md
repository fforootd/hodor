# ZITADEL

> ⚠️ **Experimental** — This is a research/prototype project exploring a reimagined identity platform architecture. It is not production-ready and APIs may change without notice. See [ZITADEL](https://github.com/zitadel/zitadel) for the production IAM system.

**Identity infrastructure for humans and AI.** Open-source identity platform with unified auth, fine-grained authorization, and schema-driven identity management — in a single binary.

## Quick Start

```bash
# Build & run with defaults (SQLite, port 8080)
go run ./cmd/zitadel serve

# With mock OIDC provider for SSO testing
go run ./cmd/zitadel serve --mock-oidc

# With seed data for local dev
go run ./cmd/zitadel serve --mock-oidc --seed fixtures/dev-seed.yaml

# Via environment variables (Docker/K8s/Workers)
ZITADEL_DATABASE_URL=postgres://... ZITADEL_MOCK_OIDC=true go run ./cmd/zitadel serve
```

## Architecture

- **Single binary** — REST API, login UI (Vue), admin console (Vue), account self-service (Vue)
- **Schema-driven identities** — JSON Schema with annotations (`x-claim-mapping`, `x-user-editable`, `x-sensitive`, `x-hidden`)
- **SSO Federation** — Protocol-agnostic providers (OIDC, SAML, SCIM) with `expr`-based claim mapping
- **Embedded OpenFGA** — Zanzibar-style authorization in-process
- **SQLite + Postgres** — zero-config dev, production-ready with Postgres
- **Event-sourced audit** — every mutation emitted as an event with field-level redaction
- **Sonyflake IDs** — time-ordered, 64-bit distributed IDs
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
| Database URL | `ZITADEL_DATABASE_URL` | — | `sqlite://./zitadel.db` |
| Server port | `ZITADEL_PORT` | — | `8080` |
| Mock OIDC | `ZITADEL_MOCK_OIDC` | `--mock-oidc` | `false` |
| Seed file | `ZITADEL_SEED_FILE` | `--seed` | — |
| External domain | `ZITADEL_EXTERNAL_DOMAIN` | — | `localhost` |

## License

AGPL-3.0 — see [LICENSE](LICENSE)
