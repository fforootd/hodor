# Deploy Zitadel to Cloudflare Workers + Containers + Turso

This example runs the real Zitadel Rust binary in a single Cloudflare Container and stores data in a remote Turso database over `libsql://`.

It is intentionally opinionated:

- One named container instance
- Scale to zero after inactivity
- Turso as the durable database
- Cloudflare TLS terminated at the edge
- Deterministic admin bootstrap through the production seed file

That makes it much easier to share and support than the previous draft.

## What This Example Actually Does

```text
Client
  -> Worker
     -> Workers Assets (/assets/*)
     -> Container (:8080, Zitadel for HTML, API, OIDC)
        -> libsql://<db>.turso.io
```

Key details:

- The Worker serves hashed frontend bundles from Workers Assets.
- The Worker proxies HTML shells, API traffic, OIDC traffic, and dynamic login assets to one named `ZitadelContainer`.
- The container starts with runtime env vars derived from Worker secrets and the first incoming request host.
- Zitadel connects directly to Turso using `ZITADEL_STORAGE_STATEFUL_URL=libsql://...`.
- The Rust runtime keeps the Cloudflare side thin by treating the stateful store as a normal runtime URL.

## Why This Version Exists

The old draft tried to be too clever with container lifecycle and database proxying.

This version keeps the Worker close to Cloudflare's stateless example:

- One stable Durable Object identity
- Let the SDK start the container on demand
- Treat Turso as a normal external database
- Keep the Worker focused on config and proxying

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

Set the required Worker secrets:

```bash
npx wrangler secret put ZITADEL_STORAGE_STATEFUL_URL
npx wrangler secret put ZITADEL_DATABASE_AUTH_TOKEN
npx wrangler secret put ZITADEL_ADMIN_PASSWORD
npx wrangler secret put ZITADEL_ADMIN_PAT
npx wrangler secret put ZITADEL_COOKIE_SECRETS
```

Recommended values:

- `ZITADEL_STORAGE_STATEFUL_URL`: `libsql://<db>.turso.io`
- `ZITADEL_DATABASE_AUTH_TOKEN`: output of `turso db tokens create <db-name>`

Deploy:

```bash
npx wrangler deploy
```

For production debugging and any deploy where you need to prove the
container image changed, use the immutable deploy flow instead:

```bash
npm run deploy:immutable
```

That script:

- Uses Cloudflare's supported `wrangler containers build -p -t ...` flow to build and push a fresh image
- Tags the image uniquely from git SHA and UTC time
- Creates a temporary repo-root `Dockerfile` from [Dockerfile.cloudflare](/Users/ffo/git/hodor/zitadel/Dockerfile.cloudflare) so Wrangler can build from the repo root context
- Generates a temporary Wrangler config that points at that exact registry image
- Deploys with `--containers-rollout=immediate`

This avoids the ambiguity of the implicit Dockerfile-path image flow, where
Cloudflare versions can appear to roll forward while the active image
reference stays unchanged.

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

For local dev, the Worker derives `ZITADEL_EXTERNAL_DOMAIN` and TLS mode from the first incoming request if you do not set overrides explicitly.

## Configuration Model

This example keeps the Worker configuration surface intentionally small.

Required secrets:

- `ZITADEL_STORAGE_STATEFUL_URL`: remote libSQL/Turso database URL, for example `libsql://test-ffozitadel.aws-us-west-2.turso.io`
- `ZITADEL_DATABASE_AUTH_TOKEN`: Turso database auth token
- `ZITADEL_ADMIN_PASSWORD`: bootstrap admin password used by the seed file
- `ZITADEL_ADMIN_PAT`: bootstrap PAT for API access
- `ZITADEL_COOKIE_SECRETS`: stable HMAC key material for session cookies

For Turso-hosted databases, `ZITADEL_DATABASE_AUTH_TOKEN` is effectively required unless you embed a token in the database URL query string.

Optional vars or secrets:

- `ZITADEL_ADMIN_EMAIL`: defaults to `admin@example.com`
- `ZITADEL_EXTERNAL_DOMAIN`: overrides the request host for issuer/public URLs
- `ZITADEL_TLS_MODE`: override inferred TLS mode (`external` for HTTPS, `off` for local HTTP)
- `ZITADEL_BASE_PATH`: optional path prefix
- `ZITADEL_STORAGE_STATEFUL_MIGRATE`: e.g. `skip` for very fast cold starts after schema is ready
- `ZITADEL_STORAGE_STATEFUL_BOOTSTRAP`: e.g. `skip` after first bootstrap
- `ZITADEL_ENCRYPTION_ACTIVE_KEY_ID` and `ZITADEL_ENCRYPTION_KEYS`: recommended if you do not want secrets stored in plaintext mode

## Database Notes

- The Go runtime now accepts `libsql://`, `https://`, `ws://`, and `wss://` URLs through the standard database layer.
- If `ZITADEL_DATABASE_AUTH_TOKEN` is set, Zitadel uses the libSQL connector instead of embedding the token in the URL.
- Startup logs redact database passwords and `authToken` query params so secrets do not spill into Cloudflare logs.

This example no longer includes the old D1 outbound bridge. If you want D1 again later, it should live as a separate example rather than sharing this Turso-first worker.

## Operational Commands

```bash
npm run typecheck
npm run deploy:immutable
npx wrangler tail
npx wrangler containers list
npx wrangler containers images list
```

## Files

- [src/index.ts](/Users/ffo/git/hodor/zitadel/deploy/cloudflare-containers/src/index.ts): Worker proxy and minimal container startup wiring
- [wrangler.jsonc](/Users/ffo/git/hodor/zitadel/deploy/cloudflare-containers/wrangler.jsonc): Worker and container config
- [Dockerfile.cloudflare](/Users/ffo/git/hodor/zitadel/Dockerfile.cloudflare): container image build
- [prod-seed.yaml](/Users/ffo/git/hodor/zitadel/fixtures/prod-seed.yaml): deterministic admin bootstrap

## Current Limits

- This example is single-container by design. It is meant to be correct and easy to share, not a horizontal scaling recipe.
- Turso commit latency depends on your Turso plan and region placement.
- If you want a custom domain or multiple public hostnames, set `ZITADEL_EXTERNAL_DOMAIN` explicitly so issuer URLs stay stable.

## Troubleshooting

- If Cloudflare tail shows `The container is not listening` and then `The container is not running`, the container process usually exited before Zitadel bound `:8080`.
- For Turso, the most common cause is a missing or invalid `ZITADEL_DATABASE_AUTH_TOKEN`.
- A missing token fails with Turso `401 Unauthorized: empty JWT token`.
- A bad token fails with Turso `400 JWT error: InvalidToken`.

## References

- [Turso Quickstart (Go)](https://docs.turso.tech/sdk/go/quickstart)
- [Turso durability guarantees](https://docs.turso.tech/cloud/durability)
- [Developer Experience Philosophy](/Users/ffo/git/hodor/zitadel/docs/design/developer-experience.md)
