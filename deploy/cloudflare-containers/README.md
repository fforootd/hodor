# Deploy Zitadel to Cloudflare Workers + Containers + D1

This example runs the real Zitadel Go binary in a single Cloudflare Container and stores data in Cloudflare D1 through the repository's built-in `d1://` SQL driver.

It is intentionally opinionated:

- One named container instance
- Scale to zero after inactivity
- D1 as the durable database
- Cloudflare TLS terminated at the edge
- Deterministic admin bootstrap through the production seed file

That makes it much easier to share and support than the previous draft.

## What This Example Actually Does

```text
Client
  -> Worker
     -> Container (:8080, Zitadel)
        -> http://d1.local/query|exec
           -> Worker outboundByHost handler
              -> env.DB (D1)
```

Key details:

- The Worker proxies inbound HTTP traffic to one named `ZitadelContainer`.
- The container starts with runtime env vars derived from Worker secrets and the incoming request host.
- Zitadel connects to `d1://d1.local`.
- `@cloudflare/containers` `outboundByHost` intercepts those container HTTP calls and translates them into D1 binding calls.

## Why The Old Version Broke

The previous example looked like a D1 deployment, but it was not:

- D1 outbound support was commented out.
- The container silently used local SQLite instead.
- Several env var names did not match the actual Zitadel config.
- `ZITADEL_COOKIE_SECRETS` was documented but ignored by the server.
- The Worker never exported `ContainerProxy`, which outbound interception now requires.

This version fixes those issues.

## Prerequisites

- A Cloudflare account with a Workers Paid plan
- Node.js 22+
- Docker running locally for image builds
- Wrangler CLI (`npm i -g wrangler`)

## Quick Start

```bash
cd deploy/cloudflare-containers
npm install
npx wrangler login
```

Create the D1 database:

```bash
npx wrangler d1 create zitadel-db
```

Copy the returned `database_id` into [wrangler.jsonc](/Users/ffo/git/hodor/zitadel/deploy/cloudflare-containers/wrangler.jsonc).

Set the required Worker secrets:

```bash
npx wrangler secret put ZITADEL_ADMIN_PASSWORD
npx wrangler secret put ZITADEL_ADMIN_PAT
npx wrangler secret put ZITADEL_COOKIE_SECRETS
```

Deploy:

```bash
npx wrangler deploy
```

After deploy, open your Worker hostname and log in with:

- Username: `admin`
- Password: the `ZITADEL_ADMIN_PASSWORD` secret you set

## Local Development

Create a local secrets file from [.dev.vars.example](/Users/ffo/git/hodor/zitadel/deploy/cloudflare-containers/.dev.vars.example):

```bash
cp .dev.vars.example .dev.vars
```

Fill in real values, then run:

```bash
npx wrangler dev
```

For local dev, the Worker derives `ZITADEL_EXTERNAL_DOMAIN` and TLS mode from the incoming request if you do not set overrides explicitly.

## Configuration Model

This example keeps the Worker configuration surface intentionally small.

Required secrets:

- `ZITADEL_ADMIN_PASSWORD`: bootstrap admin password used by the seed file
- `ZITADEL_ADMIN_PAT`: bootstrap PAT for API access
- `ZITADEL_COOKIE_SECRETS`: stable HMAC key material for session cookies

Optional vars or secrets:

- `ZITADEL_ADMIN_EMAIL`: defaults to `admin@example.com`
- `ZITADEL_EXTERNAL_DOMAIN`: overrides the request host for issuer/public URLs
- `ZITADEL_TLS_MODE`: override inferred TLS mode (`external` for HTTPS, `off` for local HTTP)
- `ZITADEL_BASE_PATH`: optional path prefix
- `ZITADEL_DATABASE_MIGRATE`: e.g. `skip` for very fast cold starts after schema is ready
- `ZITADEL_DATABASE_BOOTSTRAP`: e.g. `skip` after first bootstrap
- `ZITADEL_ENCRYPTION_ACTIVE_KEY_ID` and `ZITADEL_ENCRYPTION_KEYS`: recommended if you do not want secrets stored in plaintext mode

## D1 Bridge Notes

The Go side already speaks `d1://` through [d1driver.go](/Users/ffo/git/hodor/zitadel/internal/database/d1driver/d1driver.go).

This example's Worker provides the other half:

- `POST http://d1.local/query` runs D1 `.raw({ columnNames: true })`
- `POST http://d1.local/exec` runs D1 `.run()`

That lets Zitadel keep using the standard `database/sql` path while the Worker translates calls into the D1 binding API.

## Operational Commands

```bash
npm run typecheck
npx wrangler tail
npx wrangler containers list
npx wrangler containers images list
npx wrangler d1 info zitadel-db
```

## Files

- [src/index.ts](/Users/ffo/git/hodor/zitadel/deploy/cloudflare-containers/src/index.ts): Worker proxy, container startup, D1 outbound bridge
- [wrangler.jsonc](/Users/ffo/git/hodor/zitadel/deploy/cloudflare-containers/wrangler.jsonc): Worker, container, and D1 binding config
- [Dockerfile.cloudflare](/Users/ffo/git/hodor/zitadel/Dockerfile.cloudflare): container image build
- [prod-seed.yaml](/Users/ffo/git/hodor/zitadel/fixtures/prod-seed.yaml): deterministic admin bootstrap

## Current Limits

- This example is single-container by design. It is meant to be correct and easy to share, not a horizontal scaling recipe.
- D1 support currently relies on HTTP outbound interception, which Cloudflare announced for Containers on March 26, 2026.
- If you want a custom domain or multiple public hostnames, set `ZITADEL_EXTERNAL_DOMAIN` explicitly so issuer URLs stay stable.

## References

- [Cloudflare Containers outbound traffic](https://developers.cloudflare.com/containers/platform-details/outbound-traffic/)
- [Cloudflare changelog: outbound Workers support for Containers](https://developers.cloudflare.com/changelog/post/2026-03-26-outbound-workers/)
- [D1 Worker API](https://developers.cloudflare.com/d1/worker-api/)
- [Developer Experience Philosophy](/Users/ffo/git/hodor/zitadel/docs/design/developer-experience.md)
