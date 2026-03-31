# Local Development

Zitadel now has one default local workflow:

```bash
make dev
```

That starts:

- the Go API on `http://localhost:8080`
- the Vite dev server on `http://localhost:5173`
- mock OIDC for local SSO testing
- stdout-backed email and SMS notification channels
- the default `frontend` seed pack

## Which Command To Use

| Command | Use it for |
|---|---|
| `make dev` | The default flow: Go API + Vite HMR + frontend seed data |
| `make dev-go` | Backend work when you do not need Vite |
| `make dev-web` | Frontend-only work against an already running API |
| `make dev-embed` | Parity mode with embedded web assets instead of Vite |
| `make dev-reset` | Wipe `./data` SQLite files and restart local dev |
| `make dev-seed SEED=<name>` | Reapply a named seed pack without wiping the DB |
| `make dev-status` | Print the current local dev paths, credentials, and seed pack |

Deprecated aliases still work:

- `make dev-hot` → `make dev`
- `make dev-full` → `make dev-embed`
- `make dev-clean` → `make dev-reset`

## Seed Packs

Seed packs live in [`fixtures/seeds`](/Users/ffo/git/hodor/zitadel/fixtures/seeds):

| Pack | Purpose |
|---|---|
| `minimal` | Backend-only flow with just the admin identity |
| `frontend` | Default for local UI work with deterministic users |
| `e2e` | Deterministic browser-testing dataset |

Examples:

```bash
make dev-seed
make dev-seed SEED=minimal
make dev-seed SEED=e2e
```

The CLI entrypoint is also available directly:

```bash
go run ./cmd/zitadel seed validate --file fixtures/seeds/frontend.yaml
go run ./cmd/zitadel seed apply -c fixtures/zitadel.dev.toml --file fixtures/seeds/frontend.yaml
```

`seed apply` is safe to use on a fresh local SQLite database because it runs migrations first, ensures built-in schemas exist, and then applies the seed file.

## Local Data Paths

Local development is standardized around repo-root `./data`:

- database: [`data/zitadel.db`](/Users/ffo/git/hodor/zitadel/data/zitadel.db)
- analytics cache: [`data/zitadel-cache.db`](/Users/ffo/git/hodor/zitadel/data/zitadel-cache.db)

`make dev-reset` removes those files plus their SQLite sidecars (`-wal`, `-shm`, `-journal`) and refuses to run when `ZITADEL_DATABASE_URL` is not SQLite.

## Credentials And Mock OIDC

Default local credentials:

- username: `admin`
- password: `admin123`
- PAT: `zitadel-dev-pat-do-not-use-in-production`

Mock OIDC is enabled in [`fixtures/zitadel.dev.toml`](/Users/ffo/git/hodor/zitadel/fixtures/zitadel.dev.toml), so local SSO provider testing is available in the standard dev workflow.

## Standalone Binary

The zero-config path is still valid:

```bash
go run ./cmd/zitadel start
```

That keeps the original DX promise: SQLite-first startup with no Docker or external services required.

For explicit self-hosted operator flows, use:

```bash
printf '%s\n' 'super-secret-password' | \
  go run ./cmd/zitadel bootstrap admin --password-stdin

printf '%s\n' 'new-secret-password' | \
  go run ./cmd/zitadel recover admin --identifier admin --password-stdin
```

That path is documented in [Bootstrap and Recovery](/Users/ffo/git/hodor/zitadel/docs/guides/bootstrap-recovery.md). The interactive `start` bootstrap flow remains in place for local DX compatibility.

## Notifications In Local Dev

Local notification delivery is zero-config by default:

- email resolves to the `dev_stdout` channel
- SMS resolves to the `dev_stdout` channel
- rendered output is written to the server log instead of requiring SMTP or an SMS gateway

You can inspect and override both through the Console at [`/console/notifications`](/Users/ffo/git/hodor/zitadel/web/src/console/views/NotificationsView.vue).

If you want to try a real integration locally, add an instance or org override for:

- `smtp` channels for providers such as SendGrid or Amazon SES
- `custom_http` channels for SMS/webhook delivery such as Twilio or an internal HTTP bridge

Mailpit or an HTTP inspector can still be useful, but they are optional. Local startup should work without any extra services.
