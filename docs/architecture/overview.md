# System Architecture

Zitadel is a single Go binary (~30MB) that bundles authentication, authorization, identity management, and observability into one process.

## High-Level Architecture

```mermaid
graph TB
    subgraph Clients
        Browser["Browser / Login UI"]
        SPA["SPA / Web Components"]
        SDK_GO["Go SDK + OTel"]
        SDK_TS["TS SDK + OTel"]
        CLI["CLI / MCP"]
        Agent["AI Agent"]
        SCIM_C["Enterprise IdP<br/>(SCIM Push)"]
    end

    subgraph Core["Zitadel Binary (~30MB)"]
        direction TB
        
        subgraph Ingress["Ingress"]
            DomainResolver["Domain Resolver<br/>(tenant routing)"]
            RateLimit["Rate Limiter<br/>(token bucket)"]
            SessionMW["Session Middleware<br/>(cookie / bearer)"]
        end

        subgraph APIs["API Layer (REST+JSON)"]
            SessionAPI["Session API"]
            IdentityAPI["Identity Service"]
            OIDC_EP["OIDC Provider<br/>(zitadel/oidc)"]
            SCIM_EP["SCIM API"]
            MgmtAPI["Management API"]
            AnalyticsAPI["Analytics API"]
        end

        subgraph Auth["Auth Engine"]
            Password["Password<br/>(argon2id)"]
            Passkey["Passkeys<br/>(go-webauthn)"]
            TOTP_E["TOTP<br/>(pquerna/otp)"]
            MagicLink["Magic Links"]
            Captcha["CAPTCHA"]
        end

        subgraph AuthZ["Authorization"]
            FGA["OpenFGA<br/>(embedded, in-process)"]
            PolicyEngine["Policy Engine<br/>(expr-lang)"]
        end

        subgraph UI["UI Layer"]
            LoginUI["Login UI<br/>(Vue)"]
            ConsoleUI["Console UI<br/>(Vue)"]
        end

        subgraph Events["Event Pipeline"]
            EventWriter["Event Writer<br/>(append-only)"]
            EventBus["Event Bus<br/>(notify channel)"]
        end

        subgraph Notify["Notifications"]
            NotifyEngine["Notification Engine"]
            SMTP_C["SMTP Channel"]
            Webhook_C["Webhook Channel"]
        end
    end

    subgraph Storage["Storage"]
        SQLite["SQLite<br/>(dev / edge / homelab)"]
        Postgres["PostgreSQL<br/>(production)"]
    end

    subgraph Export["Export (OTEL)"]
        OTELCollector["Customer's OTEL Collector"]
        Sinks["Splunk / Grafana / ClickHouse / S3"]
    end

    subgraph Intelligence["Intelligence (Future)"]
        ThreatEngine["Threat Engine<br/>(expr rules + shadow mode)"]
        SLM["SLM Endpoint<br/>(Qwen via Ollama)"]
    end

    Browser --> DomainResolver
    SPA --> DomainResolver
    SDK_GO --> DomainResolver
    SDK_TS --> DomainResolver
    CLI --> DomainResolver
    Agent --> DomainResolver
    SCIM_C --> SCIM_EP

    DomainResolver --> RateLimit --> SessionMW

    SessionMW --> SessionAPI
    SessionMW --> IdentityAPI
    SessionMW --> OIDC_EP
    SessionMW --> MgmtAPI
    SessionMW --> AnalyticsAPI

    SessionAPI --> Auth
    OIDC_EP --> Auth

    IdentityAPI --> FGA
    MgmtAPI --> FGA
    SessionAPI --> PolicyEngine

    LoginUI --> SessionAPI
    ConsoleUI --> MgmtAPI

    SessionAPI --> EventWriter
    IdentityAPI --> EventWriter
    MgmtAPI --> EventWriter

    EventWriter --> SQLite
    EventWriter --> Postgres

    EventWriter --> NotifyEngine
    NotifyEngine --> SMTP_C
    NotifyEngine --> Webhook_C

    EventWriter --> OTELCollector --> Sinks

    EventWriter --> ThreatEngine
    ThreatEngine --> SLM
```

## Three Domains

Zitadel serves three distinct domains, each with different data requirements:

| Domain | Purpose | Latency | Examples |
|---|---|---|---|
| **Transactional** | CRUD, auth, authz | <10ms | Create user, authenticate, check permissions |
| **Analytical** | Aggregation, audit, traces | <1s | Login trends, audit trail, "what did user X do?" |
| **Intelligence** | Detection, alerting, response | Async | Anomaly detection, automated rate limiting |

See [ADR-010](../adr/010-three-tier-data.md) for the full data flow across these domains.

## Request Pipeline

```mermaid
sequenceDiagram
    participant C as Client
    participant DR as Domain Resolver
    participant RL as Rate Limiter
    participant SM as Session MW
    participant FGA as OpenFGA
    participant H as Handler
    participant DB as Database
    participant EW as Event Writer

    C->>DR: HTTP Request<br/>Host: tenant.auth.example.com
    DR->>DR: Resolve org from domain<br/>(DB lookup, cached)
    DR->>RL: Request + org context
    RL->>RL: Check bucket<br/>(per-IP, per-org)
    alt Rate limited
        RL-->>C: 429 + Retry-After
    end
    RL->>SM: Request + org context
    SM->>SM: Extract session<br/>(cookie or Bearer token)
    SM->>FGA: Batch pre-fetch<br/>authz context
    FGA-->>SM: Permissions map
    SM->>H: Request + session + authz map
    H->>DB: Read/Write
    H->>EW: Append event
    EW->>DB: INSERT event
    H-->>C: Response
```

### Pipeline Rules

1. **Single transaction** — entity write + event append in ONE database transaction. If either fails, both roll back.
2. **FGA is pre-fetched** — authorization context batch-loaded BEFORE handler execution. Zero live FGA calls during request.
3. **Response before async** — client gets a response immediately after DB commit. Notifications and threat evaluation happen asynchronously.
4. **Events drive everything** — notifications, OTEL export, and threat detection all consume from the events table.

## External Domain Handling

```mermaid
graph LR
    subgraph Customer_DNS["Customer DNS"]
        D1["login.acme.com<br/>CNAME → proxy"]
        D2["auth.bigcorp.io<br/>CNAME → proxy"]
    end

    subgraph Proxy["TLS Termination"]
        AutoTLS["Auto TLS<br/>(Let's Encrypt / CF)"]
    end

    subgraph Zitadel
        DR["Domain Resolver"]
        DomainTable["domains table<br/>domain → org_id"]
    end

    D1 --> AutoTLS
    D2 --> AutoTLS
    AutoTLS --> DR
    DR --> DomainTable
```

**Resolution priority** (from request Host header):
1. Exact match in `domains` table → org found
2. Subdomain matching: `acme.zitadel.cloud` → strip suffix → look up org
3. Header-based: `X-Zitadel-Org: org_id` → direct (API clients)
4. Default org (single-tenant mode)

## Deployment Topologies

| Target | Database | Storage |
|---|---|---|
| **Dev / Homelab** | SQLite (WAL mode) | Local filesystem |
| **Production** | PostgreSQL (primary + replicas) | Postgres |
| **Cloud** | PostgreSQL (per-region) | Postgres |
