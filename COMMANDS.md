# Commands

## Develop

```bash
# Backend (with dev seed data: admin/admin123)
cargo run -p zitadel -- start --seed fixtures/seeds/frontend.yaml

# Frontend (separate terminal, Vite HMR on :5173, proxies API to :8080)
npm run dev -w web

# Zero-config start (generates random admin password, prints it)
cargo run -p zitadel -- start
```

## Test

```bash
# Rust
cargo test --workspace

# Web components
npm test -w web

# E2E browser journeys (Playwright starts the server automatically)
npm test -w browser-tests
npm test -w browser-tests -- --project=journeys-admin
npm test -w browser-tests -- --project=journeys-login
npm test -w browser-tests -- --project=journeys-login-oidc

# E2E with a custom seed
ZITADEL_E2E_SEED=fixtures/seeds/minimal.yaml npm test -w browser-tests
```

## Lint

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
npm run lint -w web
npm run typecheck -w web
```

## Build

```bash
cargo build --release
```

## Database

```bash
# Run migrations manually
cargo run -p zitadel -- db migrate -c fixtures/zitadel.dev.toml

# Check migration status
cargo run -p zitadel -- db status -c fixtures/zitadel.dev.toml

# Apply a seed pack
cargo run -p zitadel -- seed apply -c fixtures/zitadel.dev.toml --file fixtures/seeds/frontend.yaml

# Validate a seed file (no DB needed)
cargo run -p zitadel -- seed validate --file fixtures/seeds/frontend.yaml

# Reset local DB
rm -f data/zitadel.db*
```

## Seed Packs

| File | Purpose |
|------|---------|
| `fixtures/seeds/frontend.yaml` | Dev default — admin/admin123 + test users + login flows |
| `fixtures/seeds/e2e.yaml` | Browser tests — mock OIDC providers + deterministic users |
| `fixtures/seeds/minimal.yaml` | Bare minimum — just admin |
| `fixtures/seeds/oidc-conformance.yaml` | OIDC protocol compliance |
