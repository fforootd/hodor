# Bootstrap and Recovery

This guide covers the current operator flow in the Rust prototype:

- bootstrapping a fresh instance through `zitadel db migrate --bootstrap`
- understanding the current recovery limitations in the Rust binary

The older `zitadel bootstrap admin` and `zitadel recover admin` commands described in [ADR-025](../adr/025-explicit-bootstrap-and-local-recovery.md) are not implemented in the current Rust CLI yet.

## Bootstrap Checklist

1. Point Zitadel at the target stateful store with `ZITADEL_STORAGE_STATEFUL_URL`, a config file, or both.
2. Run `zitadel db migrate --bootstrap`.
3. Start the server with `zitadel server start`.
4. If you need deterministic local credentials, apply a seed file or use `fixtures/zitadel.dev.toml`.

Example with SQLite:

```bash
cargo run -p zitadel -- db migrate -c fixtures/zitadel.dev.toml --bootstrap
cargo run -p zitadel -- server start -c fixtures/zitadel.dev.toml
```

Example with Postgres:

```bash
export ZITADEL_STORAGE_STATEFUL_URL='postgres://localhost:5432/zitadel?sslmode=disable'

cargo run -p zitadel -- db migrate --bootstrap
cargo run -p zitadel -- server start
```

What `migrate --bootstrap` does today:

- loads config and resolves local storage paths
- runs pending schema migrations
- ensures the default org and `admin` user record exist
- is safe to run repeatedly

What it does **not** do today:

- prompt for or set an admin password
- expose a dedicated break-glass recovery flow
- replace seed packs for deterministic local credentials

## Deterministic Local Access

For local development, the supported path to a known working admin is a seed pack:

```bash
make dev
# or
make dev-embed
# or
cargo run -p zitadel -- server start -c fixtures/zitadel.dev.toml
```

That path applies the frontend seed pack, which creates:

- `admin / admin123`
- the deterministic development PAT
- additional demo identities for login and console testing

## Recovery Status

The current Rust CLI does not expose `zitadel recover admin` yet.

For now:

- `zitadel db migrate --bootstrap` is the only supported explicit bootstrap primitive
- seed files are the supported path to deterministic local credentials
- true break-glass password recovery on an existing deployment still requires out-of-band database intervention until ADR-025 is implemented

## Relationship To `zitadel server start`

`zitadel server start` still auto-migrates and auto-bootstraps on an empty DB, which keeps the local-first workflow working, but it does not replace a future dedicated recovery command.

For self-hosted operations today, prefer:

1. `zitadel db migrate --bootstrap`
2. `zitadel server start`
