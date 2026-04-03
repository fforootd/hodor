# Zitadel R&D Prototype — AI Tool Guide

## Quick Start

```bash
# Full-stack with hot reload (recommended):
npm ci && make dev
# → Vite on http://localhost:5173 (HMR) + Rust on http://localhost:8080
# → Open http://localhost:5173 for the UI

# Backend-only (no frontend needed):
cargo build && ./target/debug/zitadel start -c fixtures/zitadel.dev.toml
```

**Dev credentials:** admin / admin123
**Dev PAT:** `zitadel-dev-pat-do-not-use-in-production`
(Defined in `fixtures/zitadel.dev.toml`)

## Project Structure

```
Cargo.toml                        Workspace root
crates/
  zitadel/src/main.rs             CLI entry point (start, migrate, seed, openapi-export)
  zitadel-server/                 HTTP mux — all route registration happens here
  zitadel-api/                    REST API handlers with auth middleware
  zitadel-db/                     DB layer (SQLite + Postgres via sqlx)
  zitadel-oidc/                   OIDC provider (discovery, token, authorize)
  zitadel-login/                  Login flow, password auth, branding
  zitadel-authn/                  Password hashing (argon2id), session cookies
  zitadel-crypto/                 AES-256-GCM envelope encryption
  zitadel-config/                 TOML config loading with env var overrides
  zitadel-authz/                  Cedar authorization model (POC)
  zitadel-catalog/                Template catalog
  zitadel-schema/                 JSON Schema validation (stub)
migrations/
  sqlite/                         SQLite migration files (00001_initial.sql, ...)
  postgres/                       Postgres migration files
web/
  src/login/                      Vue SPA — login/auth UI
  src/console/                    Vue SPA — admin console
  src/account/                    Vue SPA — account self-service
  vite.config.ts                  Vite config with backend proxy (:5173 → :8080)
fixtures/
  zitadel.dev.toml                Dev config (SQLite, mock OIDC, seed data)
  seeds/frontend.yaml             Default seed pack (admin + 3 test users)
docs/
  000-index.md                    ADR index
  GLOSSARY.md                     Domain vocabulary
```

## Key Make Targets

| Command | Purpose |
|---------|---------|
| `make dev` | Vite HMR + Rust server (recommended for development) |
| `make dev-embed` | Embedded assets + Rust server (production-like) |
| `make dev-web` | Frontend-only on :5173 (needs backend at :8080) |
| `make dev-reset` | Wipe DB and restart fresh |
| `make test` | Rust tests (`cargo test --workspace`) |
| `make test-web` | Vitest unit tests |
| `make test-e2e` | Playwright E2E tests |
| `make quality` | Full CI gate (fmt, clippy, test, typecheck, vitest) |
| `make build` | Release build (`cargo build --release`) |
| `make ensure-webdist` | Create placeholder web/dist for compilation |

## Building & Testing

```bash
# Rust tests:
cargo test --workspace

# Rust with clippy:
cargo clippy --workspace -- -D warnings

# Web (Vitest):
npm test -w web

# TypeScript typecheck:
npm run typecheck -w web

# Full CI-equivalent gate:
make quality
```

## Conventions

- **Pure Rust** single binary — no CGO, no external processes. Uses sqlx with SQLite (default) or Postgres.
- **REST API** with plans for OpenAPI 3.1 spec generation.
- **Frontend:** Vue 3 + shadcn-vue + Tailwind CSS. Three separate SPAs (login, console, account).
- **ADRs** in `docs/adr/` govern architectural decisions — check before proposing structural changes.
- **Domain vocabulary** in `docs/GLOSSARY.md` — use correct terminology.
- **Config cascade:** CLI flags > env vars > TOML config > defaults. Every field is optional.
- **Database:** SQLite default, Postgres supported. Migrations in both dialects.
- **Authorization:** Cedar-based (POC) replacing OpenFGA. Root instance has implicit wildcard access.

## Common Patterns

**Adding a REST endpoint:**
1. Add handler function in `crates/zitadel-api/src/`
2. Register route in the module's `routes()` function
3. Register module in `crates/zitadel-api/src/lib.rs`

**Adding a DB migration:**
1. Create next numbered `.sql` file in `migrations/sqlite/`
2. Create matching file in `migrations/postgres/`
3. Add `include_str!` entry in `crates/zitadel-db/src/migrate.rs`
4. Migrations run automatically on startup (default `migrate = "auto"`)

**Adding a frontend route:**
1. Add route to the relevant SPA router: `web/src/{login,console,account}/router.ts`
2. Create Vue component in the corresponding directory
3. If it's a new top-level path, add SPA fallback in `crates/zitadel-server/src/assets.rs`
