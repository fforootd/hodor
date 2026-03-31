# Bootstrap and Recovery

This guide covers the explicit self-hosted operator workflows for:

- bootstrapping the first admin on a brand-new Zitadel instance
- regaining local break-glass access when normal admin access is broken

These commands are recommended for self-hosted operations. The legacy interactive `zitadel start` bootstrap path still works for local DX, but it is not the preferred operator workflow.

## Bootstrap Checklist

1. Point Zitadel at the target database with `ZITADEL_DATABASE_URL`, a config file, or both.
2. Run `zitadel bootstrap admin`.
3. Provide the password via `--password-stdin` when possible.
4. Start the server with `zitadel start`.
5. Sign in through `/console`.

Example with SQLite:

```bash
printf '%s\n' 'super-secret-password' | \
  go run ./cmd/zitadel bootstrap admin --password-stdin

go run ./cmd/zitadel start
```

Example with Postgres:

```bash
export ZITADEL_DATABASE_URL='postgres://localhost:5432/zitadel?sslmode=disable'

printf '%s\n' 'super-secret-password' | \
  go run ./cmd/zitadel bootstrap admin \
    --username admin \
    --email admin@example.com \
    --password-stdin
```

What `bootstrap admin` does:

- loads config and resolves local storage paths
- runs pending schema migrations
- seeds built-in schemas, the default login flow, and the console client
- refuses to run if users already exist
- creates the first admin and grants instance-owner FGA access

## Recovery Checklist

1. Make sure the database schema is already migrated.
2. Run `zitadel recover admin`.
3. Target an existing admin with `--user-id` or `--identifier`.
4. Use `--create-if-missing` only when you intentionally want a new break-glass admin.
5. Prefer `--password-stdin` over `--password`.

Reset an existing admin by identifier:

```bash
printf '%s\n' 'new-secret-password' | \
  go run ./cmd/zitadel recover admin \
    --identifier admin \
    --password-stdin
```

Create a new break-glass admin only when the target does not exist:

```bash
printf '%s\n' 'new-secret-password' | \
  go run ./cmd/zitadel recover admin \
    --identifier breakglass \
    --email breakglass@example.com \
    --create-if-missing \
    --password-stdin
```

What `recover admin` does:

- performs a schema version check and fails if the database is behind the binary
- resets the password and reactivates the target user when it exists
- ensures instance-owner FGA access is present
- creates a new admin only when `--create-if-missing` is explicitly set

## Password Handling

Prefer `--password-stdin` for automation:

```bash
printf '%s\n' 'super-secret-password' | \
  go run ./cmd/zitadel bootstrap admin --password-stdin
```

The commands also support:

- `--password` for direct invocation
- hidden terminal prompting when stdin is interactive

When stdin is non-interactive and no password source is provided, the explicit commands fail instead of generating a random password.

## Relationship To `zitadel start`

`zitadel start` still supports the older interactive bootstrap path on an empty local instance. That remains useful for quick local experiments and DX.

For self-hosted operations, prefer:

1. `zitadel bootstrap admin`
2. `zitadel start`

For local break-glass access, prefer:

1. `zitadel recover admin`
2. `zitadel start`
