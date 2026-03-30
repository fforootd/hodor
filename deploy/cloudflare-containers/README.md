# Deploy Zitadel to Cloudflare Workers + Containers + D1

Run Zitadel on [Cloudflare's edge network](https://developers.cloudflare.com/containers/) with [D1](https://developers.cloudflare.com/d1/) as the database, automatic scale-to-zero, and zero infrastructure management.

## Architecture

```
                    ┌───────────────────────────────────────────────────────┐
                    │  Cloudflare Edge                                      │
                    │                                                       │
  Client ──────▶   │  ┌──────────────┐     ┌─────────────────────────────┐ │
  (HTTPS)          │  │  Worker       │────▶│  Container (linux/amd64)    │ │
                    │  │  (edge proxy) │     │                             │ │
                    │  └──────────────┘     │  zitadel start              │ │
                    │                        │  :8080                      │ │
                    │  ┌──────────────┐     │         │                   │ │
                    │  │  D1 Database  │◀────│─────────┘                   │ │
                    │  │  (SQLite)     │     │  http://d1.local/query      │ │
                    │  │              │     │  (outboundByHost intercept)  │ │
                    │  └──────────────┘     └─────────────────────────────┘ │
                    └───────────────────────────────────────────────────────┘
```

### How it works

1. **Client** → requests hit the Cloudflare edge Worker
2. **Worker** → proxies HTTP traffic to the Zitadel container via a Durable Object
3. **Container** → runs the full Zitadel Go server. When it needs the database, the Go `d1driver` package makes HTTP calls to `http://d1.local/query`
4. **outboundByHost** → the Worker intercepts these outbound HTTP calls and translates them into `env.DB.prepare().bind().run()` calls against the D1 binding
5. **D1** → Cloudflare's serverless SQLite database stores all Zitadel data durably

This keeps the database fully managed by Cloudflare — no Postgres to provision, no connection strings to manage, and D1's automatic replication handles read scaling.

## Prerequisites

- [Cloudflare account](https://dash.cloudflare.com/) with a **Workers Paid** plan
- [Node.js 22+](https://nodejs.org/) and npm
- [Docker](https://docs.docker.com/desktop/) (must be running for `wrangler deploy`)
- [Wrangler CLI](https://developers.cloudflare.com/workers/wrangler/) (`npm i -g wrangler`)

## Quick Start

```bash
# 1. Navigate to the deploy directory
cd deploy/cloudflare-containers

# 2. Install dependencies
npm install

# 3. Authenticate with Cloudflare
npx wrangler login

# 4. Create the D1 database
npx wrangler d1 create zitadel-db
# Copy the database_id output and paste it into wrangler.jsonc

# 5. Deploy (builds Docker image + pushes to CF registry + deploys Worker)
npx wrangler deploy
```

> [!NOTE]
> The first deploy takes a few minutes to build the Docker image and provision the container. Subsequent deploys are faster due to Docker layer caching.

After deploying, check container status:

```bash
npx wrangler containers list
```

## Configuration

### D1 Database

The D1 database is created and managed via Wrangler:

```bash
# Create the database
npx wrangler d1 create zitadel-db

# View database info
npx wrangler d1 info zitadel-db

# Run a query directly
npx wrangler d1 execute zitadel-db --command "SELECT count(*) FROM users"

# Export the database
npx wrangler d1 export zitadel-db --output ./backup.sql
```

Update `wrangler.jsonc` with the `database_id` from the create command:

```jsonc
"d1_databases": [
  {
    "binding": "DB",
    "database_name": "zitadel-db",
    "database_id": "<YOUR_D1_DATABASE_ID>"
  }
]
```

### The D1 Bridge (`d1://` driver)

Zitadel connects to D1 via a custom Go SQL driver that bridges the gap between `database/sql` and D1's Worker binding API:

```
Container (Go)                Worker (JS)              D1
──────────────               ──────────               ──
d1driver.Query()  ──HTTP──▶  outboundByHost  ──▶  env.DB.prepare()
                             "d1.local"            .bind().run()
                  ◀─JSON───                   ◀──  D1Result
```

- **`internal/database/d1driver/`** — Go `database/sql/driver` that sends SQL as HTTP POST to `http://d1.local`
- **`src/index.ts` (outboundByHost)** — intercepts these HTTP calls and translates to D1 binding API calls
- **`d1://d1.local`** — connection string configured via `ZITADEL_DATABASE_URL`

Since D1 is SQLite-compatible, all of Zitadel's existing SQLite migrations and queries work unchanged.

### Image Source

The `wrangler.jsonc` supports two image strategies:

#### Option A: Build from Dockerfile (default)

```jsonc
"image": "./Dockerfile"
```

#### Option B: Pre-built GHCR image from goreleaser

```jsonc
"image": "ghcr.io/zitadel/zitadel:v1.0.0"
```

> [!IMPORTANT]
> The `Dockerfile.goreleaser` uses `FROM scratch` which lacks CA certificates. Use Option A or update the goreleaser Dockerfile.

### Instance Types

| Instance Type | vCPU | Memory | Disk | Use Case |
|---------------|------|--------|------|----------|
| `lite` | 1/16 | 256 MiB | 2 GB | Testing only |
| `basic` | 1/4 | 1 GiB | 4 GB | Dev / preview |
| `standard-1` | 1/2 | 4 GiB | 8 GB | **Production** (default) |
| `standard-2` | 1 | 6 GiB | 12 GB | High traffic |

### Secrets

For sensitive configuration, use Wrangler secrets:

```bash
# Cookie signing keys
npx wrangler secret put ZITADEL_COOKIE_SECRETS
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Deploy to Cloudflare
on:
  push:
    tags: ['v*']

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Install dependencies
        working-directory: deploy/cloudflare-containers
        run: npm ci

      - name: Deploy
        working-directory: deploy/cloudflare-containers
        run: npx wrangler deploy
        env:
          CLOUDFLARE_API_TOKEN: ${{ secrets.CLOUDFLARE_API_TOKEN }}
```

## Local Development

```bash
cd deploy/cloudflare-containers

# Start local dev (uses Docker + miniflare for D1):
npx wrangler dev
```

This starts the Worker locally with a local D1 instance, and spins up the container via Docker.

## Operational Commands

```bash
# View container status
npx wrangler containers list

# List images in Cloudflare Registry
npx wrangler containers images list

# SSH into a running container (get ID from 'list' first)
npx wrangler containers ssh <ID>

# Stream Worker logs
npx wrangler tail

# D1 database operations
npx wrangler d1 info zitadel-db
npx wrangler d1 execute zitadel-db --command "SELECT count(*) FROM users"
npx wrangler d1 export zitadel-db --output ./backup.sql
```

## File Structure

```
deploy/cloudflare-containers/
├── Dockerfile          # Multi-stage build (Node → Go → Debian slim)
├── wrangler.jsonc      # CF Workers + Containers + D1 configuration
├── package.json        # Wrangler + @cloudflare/containers dependencies
├── tsconfig.json       # TypeScript config for the Worker
├── src/
│   └── index.ts        # Edge Worker: proxy + D1 outbound bridge
└── README.md           # This file

internal/database/
├── database.go         # Unified DB interface (sqlite, postgres, d1)
└── d1driver/
    └── d1driver.go     # Go database/sql driver for D1 (HTTP proxy)
```

## How D1 Compares

| Feature | SQLite (default) | Postgres | **D1** |
|---------|-----------------|----------|--------|
| Managed | No | You provision | **Cloudflare-managed** |
| Durable | File on disk | Yes | **Yes (replicated)** |
| Migrations | `zitadel migrate` | `zitadel migrate` | **`zitadel migrate`** (same) |
| Scale | Single node | Connection pool | **Edge-distributed reads** |
| Cost | Free | Provider fees | **Workers Paid plan** |
| Best for | Dev / homelab | Multi-region prod | **Cloudflare-native deploy** |

## Limitations

- **D1 is in open beta** — production readiness is improving but check [D1 limits](https://developers.cloudflare.com/d1/platform/limits/)
- **No multi-statement transactions** — D1 auto-commits each statement. The d1driver returns a no-op `Tx`. Zitadel's SQLite code path already uses single-statement writes, so this is compatible
- **Outbound HTTP only** — the D1 bridge uses HTTP (not HTTPS) between container and Worker, but this traffic runs on the same machine and never leaves Cloudflare's network

## Further Reading

- [Cloudflare Containers docs](https://developers.cloudflare.com/containers/)
- [D1 documentation](https://developers.cloudflare.com/d1/)
- [D1 Worker Binding API](https://developers.cloudflare.com/d1/worker-api/)
- [Container outbound traffic](https://developers.cloudflare.com/containers/platform-details/outbound-traffic/)
- [Container examples](https://developers.cloudflare.com/containers/examples/)
