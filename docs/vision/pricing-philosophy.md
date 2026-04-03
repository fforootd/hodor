# Pricing Philosophy

> This document describes our thinking on pricing and monetization. These are design guidelines, not commitments.

## Model: Open Core + Cloud + Consumption

Three tiers, each for a different buyer.

### Tier 1: Community (Free, OSS, Self-Hosted)

Everything needed to run a production identity platform:

- Full auth stack (password, passkey, SSO, SAML, OIDC, magic links)
- Agent identity + delegation + kill switches
- FGA authorization (embedded OpenFGA)
- Policy engine (expr)
- Login UI + Console UI (embedded)
- SQLite + Postgres
- CLI + MCP server
- Rust + TS SDKs (with OTel traces)
- SCIM API, rate limiting, CAPTCHA

**Price:** $0 forever.

**Why free?** Basic auth is a commodity. If you charge for it, developers pick Keycloak. If you give it away — with agent governance nobody else has — you capture the bottom of the funnel.

### Tier 2: Enterprise (Annual)

For teams that need compliance and operational guarantees:

- Spanner support (planet-scale)
- SAML IdP + LDAP
- SOC2 / ISO 27001 compliance artifacts
- SLA guarantees (99.95%+)
- Dedicated support, priority patches

**Buyer:** CISO, Head of Engineering. They're paying for risk transfer, not features.

### Tier 3: Identity Intelligence (Consumption-Based)

This is the growth engine. Human logins are flat. AI agent actions will explode.

- Cloud-hosted analytics (managed)
- Observability dashboard (user timelines, agent activity, audit)
- Threat Detection Engine (rules + SLM anomaly detection)
- Forensics (LLM-powered investigation)
- Alerting + automated response
- SDK telemetry ingestion

**Pricing thinking:**

| Meter | Rationale |
|---|---|
| Events analyzed | Scales with usage, not user count |
| Threat rules evaluated | Scales with security posture |
| Forensics queries | LLM inference cost pass-through |

> [!IMPORTANT]
> **Do not tax customers for having users.** Tax them for the exhaust their agents create. A customer with 10 users and 1M agent actions/month should pay more than a customer with 100K users and zero agents. This aligns revenue with where the market is growing.

## Design Principles

1. **Community tier must be complete** — not a crippled demo. Every self-hoster, every homelab, every startup prototype runs the full platform.

2. **Consumption beats per-seat** — per-seat pricing punishes growth. Consumption pricing aligns our revenue with customer success.

3. **Intelligence is the cloud upsell** — self-hosters CAN run analytics locally, but managing retention + the threat engine is operational overhead most teams don't want. The cloud version is turnkey.

4. **Circuit breakers prevent bill shock** — hard caps with clear alerting before a customer hits unexpected costs.
