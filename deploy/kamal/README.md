# Deploy Zitadel to GCP with Kamal + Caddy

POC for fast custom domain TLS provisioning. Caddy handles on-demand TLS (Let's Encrypt, ~5 seconds per domain). Kamal manages container deployment to a GCE VM via SSH.

## Architecture

```
Customer domain (login.acme.com)
  → DNS A/CNAME to GCE VM IP
    → Caddy (:443, on-demand TLS)
      → X-Instance-Id header injection
      → Reverse proxy to Zitadel (:8080)
        → Cloud SQL (Postgres)
```

All containers run on one VM via Docker, managed by Kamal. Caddy provisions Let's Encrypt certs on first request — no pre-configuration needed per domain.

## Prerequisites

- A GCE VM (e2-small) with a static external IP, ports 80 + 443 open
- SSH access to the VM (root or sudo)
- Ruby (`gem install kamal`) or Docker (to run Kamal in a container)
- A container registry (GitHub Container Registry used by default)
- A Cloud SQL Postgres instance (or any reachable Postgres)
- `jq` installed locally (for the Caddyfile generator)

## Quick Start

### 1. Configure

```bash
cd deploy/gcp-kamal

# Set your VM IP
export POC_VM_IP=34.0.1.1

# Edit secrets (never commit this file)
vim .kamal/secrets

# Edit domain mapping
vim domains.json

# Generate Caddyfile from domains.json
./generate-caddyfile.sh
```

### 2. Deploy

```bash
# First-time setup: installs Docker, pulls images, starts everything
kamal setup

# Subsequent deploys (zero-downtime):
kamal deploy
```

### 3. Add a Customer Domain

```bash
# 1. Customer configures CNAME: login.newcustomer.com → your VM IP

# 2. Add to domains.json
vim domains.json

# 3. Regenerate and reload Caddy
./generate-caddyfile.sh --reload
```

On the next HTTPS request to `login.newcustomer.com`, Caddy provisions a Let's Encrypt cert automatically (~5 seconds).

## Operations

```bash
# Deploy new Zitadel version
kamal deploy

# View Zitadel logs
kamal app logs

# View Caddy logs
kamal accessory logs caddy

# Restart Caddy
kamal accessory reboot caddy

# SSH into the VM
kamal app exec bash

# Check container status
kamal details
```

## How It Works

**Kamal** manages three containers on the VM:
- **zitadel** — the main service, deployed via kamal-proxy for zero-downtime updates
- **caddy** — accessory, handles TLS termination and domain→instance routing
- **redis** — accessory, EdgeKV for sessions and tokens

**Caddy** is configured with one site block per customer domain (generated from `domains.json`). Each block:
1. Uses `on_demand` TLS — cert provisioned lazily on first request
2. Injects `X-Instance-Id` header based on the domain→instance mapping
3. Proxies to Zitadel on the Docker network

**Zitadel** runs with `ZITADEL_MULTI_TENANT=true`. The `InstanceGate` middleware reads `X-Instance-Id` from the header and scopes all database queries to that instance via `ScopedDB`.

## Path to Production

1. **GCS cert storage** — use Caddy's `caddy-gcs` module so certs survive VM replacement
2. **DB-backed domain lookup** — replace `domains.json` with a Caddy plugin that reads from Cloud SQL
3. **TCP Load Balancer** — add GCP L4 Network LB for stable IP + health check failover
4. **Multiple VMs** — add IPs to `deploy.yml`, Kamal deploys to all hosts
5. **Cloud Armor** — attach L3/L4 DDoS protection to the LB
6. **GLB for your domains** — keep GLB (L7, managed certs, CDN) for `*.zitadel.cloud`; Caddy handles customer custom domains only

## Files

| File | Purpose |
|------|---------|
| `config/deploy.yml` | Kamal deployment config |
| `config/Caddyfile` | Generated Caddy config (one site block per domain) |
| `.kamal/secrets` | Secrets (DB URL, registry token, etc.) |
| `domains.json` | Static domain → instance_id mapping |
| `generate-caddyfile.sh` | Generates Caddyfile from domains.json |
