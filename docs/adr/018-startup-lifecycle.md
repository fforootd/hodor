# ADR-018: Startup Lifecycle & Schema Migration Strategy

**Status**: Proposed  
**Date**: 2026-03-28  
**Builds on**: ADR-010 (Three-Tier Data), ADR-017 (Caching Tiers)  
**Related**: [Developer Experience](../design/developer-experience.md), [Architecture Overview](../architecture/overview.md)

## Context

Zitadel's single startup flow did everything: load config → open DB → run migrations → bootstrap admin → start server. This works for dev but breaks in production:

- **K8s rolling deploy**: Multiple pods race to run DDL simultaneously
- **Managed Postgres**: App user may lack DDL privileges
- **Autoscaler**: Cold-start includes unnecessary migration/bootstrap checks
- **Large migrations**: Long-running DDL blocks all pod startups

## Decision

### 1. Two CLI Commands

```
zitadel server start      # Start server (migration behavior driven by config)
zitadel db migrate        # Run migrations only, then exit
zitadel db migrate status # Print schema version info
```

### 2. Config-Driven Lifecycle

```toml
[storage.primary]
url = "sqlite://./data/zitadel.db"
migrate = "auto"       # "auto" | "check" | "skip"
bootstrap = "auto"     # "auto" | "skip"
```

| Mode | Behavior |
|---|---|
| `auto` (default) | Run migrations before serving — consistent for all dialects |
| `check` | Read-only version check, fail if schema is behind target |
| `skip` | No check, no migration — fastest cold-start for autoscaling |

### 3. SQLite is THE Default

SQLite is the default storage engine. For single-server deployments (dev, homelab, edge, small SaaS), Postgres is not required. Docker Compose + PG bootstrapping is eliminated from the quickstart.

Postgres is explicitly positioned as the **scale-out option** — added when you need multi-instance, managed backups, or enterprise compliance.

### 4. Postgres Advisory Locks

`zitadel db migrate` uses Goose's `WithSessionLocker()` for Postgres to acquire a session-level advisory lock. This makes concurrent migration runs safe — only one process runs DDL at a time, others block.

### 5. Two-User Model (Optional)

For managed Postgres, operators can use separate database users:

| User | Purpose | When |
|---|---|---|
| Migration user | DDL (CREATE, ALTER, DROP) | `zitadel db migrate` |
| Application user | DML only (SELECT, INSERT, UPDATE, DELETE) | `zitadel server start` |

This is optional — a single user with full privileges works fine.

## Deployment Recipes

### Dev (SQLite — zero config)

```bash
zitadel server start
# → SQLite at ./data/zitadel.db, auto-migrated, admin bootstrapped
```

### Dev (Postgres)

```bash
ZITADEL_STORAGE_STATEFUL_URL=postgres://localhost:5432/zitadel zitadel server start
# → Postgres auto-migrated (same as SQLite — consistent defaults)
```

### Production (Managed Postgres)

```yaml
# K8s: Job runs migration, Deployment runs server
apiVersion: batch/v1
kind: Job
spec:
  template:
    spec:
      containers:
        - name: migrate
          command: ["zitadel", "db", "migrate", "--bootstrap"]
          env:
            - name: ZITADEL_STORAGE_STATEFUL_URL
              valueFrom: { secretKeyRef: { name: db, key: migrate-url } }
---
apiVersion: apps/v1
kind: Deployment
spec:
  template:
    spec:
      containers:
        - name: zitadel
          command: ["zitadel", "server", "start"]
          env:
            - name: ZITADEL_STORAGE_STATEFUL_URL
              valueFrom: { secretKeyRef: { name: db, key: app-url } }
            - name: ZITADEL_STORAGE_STATEFUL_MIGRATE
              value: "check"
            - name: ZITADEL_STORAGE_PRIMARY_BOOTSTRAP
              value: "skip"
```

### DevOps / Autoscaling

```toml
[storage.primary]
migrate = "skip"      # fastest cold-start
bootstrap = "skip"
```

## Consequences

- **Zero-config preserved**: `zitadel server start` with no config → SQLite, everything works
- **Consistent defaults**: `migrate=auto` for all dialects — no surprising per-dialect behavior
- **Safe concurrency**: Postgres advisory locks prevent DDL races
- **Production control**: `check` and `skip` modes for hardened deployments
- **Managed PG documented**: Grants, TLS, DSN formats, pool settings all documented
- **No Docker Compose**: SQLite-first eliminates PG as a dev dependency
