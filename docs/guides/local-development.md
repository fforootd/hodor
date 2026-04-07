# Local Development

Zitadel's default local workflow runs the backend and frontend in parallel:

```bash
# Terminal 1 — Rust API:
cargo run -p zitadel -- start --seed fixtures/seeds/frontend.yaml

# Terminal 2 — Vite dev server:
npm run dev -w web
```

That starts:

- the Rust API on `http://localhost:8080`
- the Vite dev server on `http://localhost:5173`
- mock OIDC for local SSO testing
- stdout-backed email and SMS notification channels
- the default `frontend` seed pack

## Which Command To Use

| Command | Use it for |
|---|---|
| `cargo run -p zitadel -- start --seed fixtures/seeds/frontend.yaml` + `npm run dev -w web` | The default flow: Rust API + Vite HMR + frontend seed data |
| `npm run build -w web && cargo run -p zitadel -- start -c fixtures/zitadel.dev.toml` | Parity mode with embedded web assets instead of Vite |
| `npm run dev -w web` | Frontend-only work against an already running API |
| Delete `data/zitadel.db*` files, then restart | Wipe `./data` SQLite files and restart local dev |

## Seed Packs

Seed packs live in [fixtures/seeds](../../fixtures/seeds):

| Pack | Purpose |
|---|---|
| `minimal` | Backend-only flow with just the admin identity |
| `frontend` | Default for local UI work with deterministic users |
| `e2e` | Deterministic browser-testing dataset |

Examples:

```bash
cargo run -p zitadel -- seed apply -c fixtures/zitadel.dev.toml --file fixtures/seeds/frontend.yaml
cargo run -p zitadel -- seed apply -c fixtures/zitadel.dev.toml --file fixtures/seeds/minimal.yaml
cargo run -p zitadel -- seed apply -c fixtures/zitadel.dev.toml --file fixtures/seeds/e2e.yaml
```

Validation:

```bash
cargo run -p zitadel -- seed validate --file fixtures/seeds/frontend.yaml
```

`seed apply` is safe to use on a fresh local SQLite database because it runs migrations first, ensures built-in schemas exist, and then applies the seed file.

## Local Data Paths

Local development is standardized around repo-root `./data`:

- database: `data/zitadel.db`
- analytics cache: `data/zitadel-cache.db`

To reset, delete those files plus their SQLite sidecars (`-wal`, `-shm`, `-journal`) and restart. Only do this when `ZITADEL_STORAGE_STATEFUL_URL` is SQLite (the default).

## Credentials And Mock OIDC

Default local credentials:

- username: `admin`
- password: `admin123`
- PAT: `zitadel-dev-pat-do-not-use-in-production`

Mock OIDC is enabled in [fixtures/zitadel.dev.toml](../../fixtures/zitadel.dev.toml), so local SSO provider testing is available in the standard dev workflow.

## Standalone Binary

The zero-config path is still valid:

```bash
cargo run -p zitadel -- server start
```

That keeps the original DX promise: SQLite-first startup with no Docker or external services required. The server auto-migrates and ensures the default org/admin record exists. Deterministic local credentials such as `admin / admin123` still come from seed packs such as `fixtures/zitadel.dev.toml`.

For the current explicit bootstrap pass, use:

```bash
cargo run -p zitadel -- migrate -c fixtures/zitadel.dev.toml --bootstrap
```

The current bootstrap/recovery status is documented in [Bootstrap and Recovery](bootstrap-recovery.md). Dedicated `recover admin` commands are planned separately and are not part of the current Rust binary.

## Spanner Emulator Certification

The native Spanner PR-wall lane is reproducible locally by running the contracts, invariants, subsystems, and resilience family suites in sequence.

If you want the emulator-backed suites to run instead of skipping, start the Cloud Spanner emulator and export the stable test env contract:

```bash
docker run --rm -p 9010:9010 -p 9020:9020 gcr.io/cloud-spanner-emulator/emulator

export ZITADEL_TEST_SPANNER_EMULATOR_HOST=127.0.0.1:9010
export ZITADEL_TEST_SPANNER_PROJECT=local-project
export ZITADEL_TEST_SPANNER_INSTANCE=test-instance
export ZITADEL_TEST_SPANNER_DATABASE_PREFIX=zitadel

# Then run the family suites (contracts, invariants, subsystems, resilience)
cargo test -p zitadel-server --test contracts_http_router \
  --test contracts_management_root_instance \
  --test contracts_spanner_http_router
cargo test -p zitadel-server --test invariants_tenant_instance_isolation \
  && cargo test -p zitadel-fga --test invariants_authorization_hierarchy
# ... plus subsystems and resilience suites
cargo test -p zitadel-storage --test resilience_storage_spanner_transient
```

The PR CI wall includes the same emulator-backed certification lane. If those env vars are absent locally, the Spanner suites skip cleanly instead of blocking the zero-config SQLite workflow.

## Notifications In Local Dev

Local notification delivery is zero-config by default:

- email resolves to the `dev_stdout` channel
- SMS resolves to the `dev_stdout` channel
- rendered output is written to the server log instead of requiring SMTP or an SMS gateway

You can inspect and override both through the Console notifications route, wired in [web/src/console/router.ts](../../web/src/console/router.ts).

If you want to try a real integration locally, add an instance or org override for:

- `smtp` channels for providers such as SendGrid or Amazon SES
- `custom_http` channels for SMS/webhook delivery such as Twilio or an internal HTTP bridge

Mailpit or an HTTP inspector can still be useful, but they are optional. Local startup should work without any extra services.
