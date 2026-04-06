# Local Development

Zitadel now has one default local workflow:

```bash
just dev
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
| `just dev` | The default flow: Rust API + Vite HMR + frontend seed data |
| `just dev-embed` | Backend work when you do not need Vite |
| `just dev-web` | Frontend-only work against an already running API |
| `just dev-embed` | Parity mode with embedded web assets instead of Vite |
| `just dev-reset` | Wipe `./data` SQLite files and restart local dev |
| `just dev-seed <name>` | Reapply a named seed pack without wiping the DB |
| `just dev-status` | Print the current local dev paths, credentials, and seed pack |

## Seed Packs

Seed packs live in [fixtures/seeds](../../fixtures/seeds):

| Pack | Purpose |
|---|---|
| `minimal` | Backend-only flow with just the admin identity |
| `frontend` | Default for local UI work with deterministic users |
| `e2e` | Deterministic browser-testing dataset |

Examples:

```bash
just dev-seed
just dev-seed minimal
just dev-seed e2e
```

The CLI entrypoint is also available directly:

```bash
cargo run -p zitadel -- seed validate --file fixtures/seeds/frontend.yaml
cargo run -p zitadel -- seed apply -c fixtures/zitadel.dev.toml --file fixtures/seeds/frontend.yaml
```

`seed apply` is safe to use on a fresh local SQLite database because it runs migrations first, ensures built-in schemas exist, and then applies the seed file.

## Local Data Paths

Local development is standardized around repo-root `./data`:

- database: `data/zitadel.db`
- analytics cache: `data/zitadel-cache.db`

`just dev-reset` removes those files plus their SQLite sidecars (`-wal`, `-shm`, `-journal`) and refuses to run when `ZITADEL_STORAGE_STATEFUL_URL` is not SQLite.

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
