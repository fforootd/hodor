# Zitadel R&D Prototype — AI Tool Guide

## Quick Start

```bash
# Full-stack with hot reload (recommended):
npm ci && make dev-hot
# → Vite on http://localhost:5173 (HMR) + Go on http://localhost:8080
# → Open http://localhost:5173 for the UI

# Go-only (no frontend needed):
go run -tags devweb ./cmd/zitadel start -c fixtures/zitadel.dev.toml
```

**Dev credentials:** admin / admin123
**Dev PAT:** `zit_pat_zitadel-dev-pat-do-not-use-in-production`
(Defined in `fixtures/zitadel.dev.toml`)

## The `-tags devweb` Build Tag

The Go binary embeds the built frontend via `//go:embed all:webdist` in `internal/server/webdist_prod.go`. The `internal/server/webdist/` directory is gitignored and only produced by `make webdist` (requires npm ci + full Vite build).

**For local Go commands, always use `-tags devweb`** to skip the embed requirement:

```bash
go build -tags devweb ./...
go test -tags devweb ./...
go test -tags devweb -v ./internal/api/...
go vet -tags devweb ./...
```

Without the tag, you need `make webdist` first (or `make ensure-webdist` for placeholders).

## Project Structure

```
cmd/zitadel/main.go          Cobra CLI entry point (start, migrate, openapi-export)
internal/
  server/server.go            HTTP mux — all route registration happens here
  server/webdist_prod.go      Production: embeds built frontend assets
  server/webdist_dev.go       Dev: empty FS (Vite serves assets instead)
  api/api.go                  REST API handlers with OpenAPI annotations
  oidcop/                     OIDC provider (discovery, token, authorize)
  login/                      Login flow, SSO, password auth
  database/                   DB layer (SQLite + Postgres)
    migrations/sqlite/        SQLite migration files (00001_initial.sql, ...)
    migrations/postgres/       Postgres migration files
  fga/                        OpenFGA authorization model and middleware
  session/                    Cookie/session management
  config/config.go            TOML config loading with env var overrides
  catalog/                    Template catalog (actions, providers, schemas)
  auth/                       Password hashing (argon2id)
  schema/                     JSON Schema validation
web/
  src/login/                  Vue SPA — login/auth UI
  src/console/                Vue SPA — admin console
  src/account/                Vue SPA — account self-service
  vite.config.ts              Vite config with backend proxy (:5173 → :8080)
fixtures/
  zitadel.dev.toml            Dev config (mock OIDC, seed data, SQLite)
  dev-seed.yaml               Seed data loaded on startup
docs/
  000-index.md                ADR index — check before proposing structural changes
  GLOSSARY.md                 Domain vocabulary (Projects=Groups, Apps=Identity Schemas)
  design/developer-experience.md  Zero-config philosophy
```

## Key Make Targets

| Command | Purpose |
|---------|---------|
| `make dev-hot` | Vite HMR + Go server (uses `-tags devweb`) |
| `make dev-full` | Same as dev-hot + mock OIDC + seed data |
| `make dev-clean` | Wipe DB and restart fresh |
| `make dev` | Run with embedded assets (requires `make webdist` first) |
| `make test` | Go tests (requires webdist) |
| `make test-web` | Vitest unit tests |
| `make test-e2e` | Playwright E2E tests |
| `make quality` | Full CI gate (fmt, vet, lint, typecheck, tests) |
| `make ensure-webdist` | Create placeholder webdist for Go compilation |
| `make generate` | Regenerate TypeScript SDK from OpenAPI spec |

## Testing

```bash
# Go unit tests (fast, no webdist needed):
go test -tags devweb -v ./internal/api/...
go test -tags devweb -v ./internal/server/...

# All Go tests:
go test -tags devweb ./...

# Go tests with race detector (CI parity):
go test -tags devweb -race ./...

# Web (Vitest):
npm test -w web

# TypeScript typecheck:
npm run typecheck -w web

# ESLint:
npm run lint -w web

# Full CI-equivalent gate:
make quality
```

## Conventions

- **Pure Go** single binary — no CGO, no external processes by default (Level 0). Uses `modernc.org/sqlite` for local/dev; scales to Postgres + Redis + queues at higher deployment profiles. See `docs/design/storage-architecture.md`.
- **REST API** with OpenAPI 3.1 spec generated from Go type annotations. Not gRPC.
- **Frontend:** Vue 3 + shadcn-vue + Tailwind CSS. Three separate SPAs (login, console, account).
- **ADRs** in `docs/adr/` govern architectural decisions — check before proposing structural changes.
- **Domain vocabulary** in `docs/GLOSSARY.md` — use correct terminology.
- **Config cascade:** CLI flags > env vars > TOML config > defaults. Every field is optional.
- **Database:** SQLite default, Postgres supported. Migrations in both dialects.
- **Authorization:** Embedded OpenFGA (Zanzibar-style). Root instance has implicit wildcard access.

## Common Patterns

**Adding a REST endpoint:**
1. Add handler function in `internal/api/`
2. Register route in `internal/api/api.go` (or relevant sub-file)
3. Add OpenAPI annotation struct for auto-generated docs

**Adding a DB migration:**
1. Create next numbered `.sql` file in `internal/database/migrations/sqlite/`
2. Create matching file in `internal/database/migrations/postgres/`
3. Migrations run automatically on startup (default `migrate = "auto"`)

**Adding a frontend route:**
1. Add route to the relevant SPA router: `web/src/{login,console,account}/router.ts`
2. Create Vue component in the corresponding directory
3. If it's a new top-level path, add SPA fallback in `web/vite.config.ts` and handler in `server.go`
