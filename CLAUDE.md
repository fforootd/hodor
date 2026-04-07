# Zitadel R&D Prototype — AI Tool Guide

## Quick Start

The frontend (Vue/Vite) and backend (Rust) are independent and can run separately.

```bash
# Backend (builds + starts with dev seed data):
cargo run -p zitadel -- start --seed fixtures/seeds/frontend.yaml
# → http://localhost:8080

# Frontend (separate terminal, Vite HMR proxies to :8080):
npm run dev -w web
# → http://localhost:5173

# Zero-config start (generates random admin password, prints it):
cargo run -p zitadel -- start
```

**Dev credentials (with frontend.yaml seed):** admin / admin123
**Dev PAT:** `zitadel-dev-pat-do-not-use-in-production`

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
  zitadel-authz/                  Built-in role catalog + permission mappings
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
  zitadel.dev.toml                Dev config (SQLite, seed data)
  seeds/frontend.yaml             Default seed pack (admin + 3 test users)
  seeds/e2e.yaml                  E2E test seed (mock OIDC providers)
docs/
  000-index.md                    ADR index
  GLOSSARY.md                     Domain vocabulary
```

## Building & Testing

```bash
# Rust tests:
cargo test --workspace

# Rust lint:
cargo fmt --check
cargo clippy --workspace -- -D warnings

# Web component tests (Vitest):
npm test -w web

# Web lint:
npm run lint -w web
npm run typecheck -w web

# E2E browser journeys (Playwright manages the server lifecycle):
npm test -w browser-tests                              # all suites
npm test -w browser-tests -- --project=journeys-admin  # one suite

# Release build:
cargo build --release
```

## Conventions

- **Pure Rust** single binary — no CGO, no external processes. Uses sqlx with SQLite (default) or Postgres.
- **REST API** with plans for OpenAPI 3.1 spec generation.
- **Frontend:** Vue 3 + shadcn-vue + Tailwind CSS. Three separate SPAs (login, console, account).
- **ADRs** in `docs/adr/` govern architectural decisions — check before proposing structural changes.
- **Domain vocabulary** in `docs/GLOSSARY.md` — use correct terminology.
- **Config cascade:** CLI flags > env vars > TOML config > defaults. Every field is optional.
- **Database:** SQLite default, Postgres supported. Migrations in both dialects.
- **Authorization:** Embedded FGA for relationship evaluation plus a built-in role-permission catalog sourced from ZITADEL `InternalAuthZ`. Platform authz uses the internal `platform` store; `operator_admin` is the only break-glass bypass.
- **Native tools only:** Use `cargo` for Rust, `npm` for frontend/tests. No task runner.

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
