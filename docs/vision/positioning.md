# Market Positioning

> **Identity Control Plane for Humans and AI**

## Hero Message

**Headline:** Authenticate Humans. Govern Agents. Secure Everything.

**Sub-headline:** The open-source identity infrastructure built for the AI era. Drop-in B2C auth, multi-tenant B2B SSO, and first-class governance for autonomous AI agents.

## Three Pillars

### Pillar 1: Universal Identity Primitives — *For Developers*

> Stop juggling auth providers.

Build B2C login flows, deeply nested B2B SaaS organizations, and machine-to-machine auth on a single, unified API. Our schema-driven model means passkeys, magic links, and SSO are just a toggle away for any identity.

**Key differentiators:**
- One binary, runs in under 60 seconds (`zitadel start` with `sqlite://./data/zitadel.db`)
- One API — not User + Machine + App
- Embeddable Web Components (`<zitadel-login>`) or classic OIDC redirect
- SCIM + REST + CLI + MCP — every interface a developer or agent needs

### Pillar 2: AI Agent Governance — *For AI Engineers*

> Give your AI a passport and a leash.

Standard API keys weren't built for autonomous AI. Zitadel treats agents as first-class identities with user-delegated tokens, strict execution quotas, and instant kill-switches. With native MCP integration, Zitadel becomes the identity brain for your LLMs.

**Key differentiators:**
- Agent = identity with delegation + quota capabilities (not a hack on service accounts)
- Token Exchange (RFC 8693) — agent acts *on behalf of* user with scoped permissions
- Kill switch: revoke all agent sessions and delegations in one API call
- Per-agent rate limits and action quotas

### Pillar 3: Identity Intelligence — *For SecOps*

> See the invisible. Stop the malicious.

Identity is your new perimeter. Zitadel natively streams server events and SDK telemetry into your observability stack. Correlate human and agent behavior, run ad-hoc queries on your identity data, and let the Threat Engine automatically respond to anomalies.

**Key differentiators:**
- Console analytics — SQL editor queries identity data directly
- OTEL-native export — forward to Splunk, Grafana, ClickHouse, S3
- Threat Detection Engine — rules (expr) + SLM anomaly classification
- Shadow Mode — evaluate threat rules without blocking (build trust before enforcement)
- Forensics — investigate identity behavior with LLM-powered queries

## Landscape Analysis

| Capability | Auth0/Okta | Clerk | Keycloak | Zitadel |
|---|---|---|---|---|
| Self-hosted | ❌ | ❌ | ✅ | ✅ |
| Cloud | ✅ | ✅ | ❌ | ✅ |
| Single binary | ❌ | ❌ | ❌ (Java/Docker) | ✅ (Rust) |
| SQLite dev mode | ❌ | ❌ | ❌ | ✅ |
| Agent identity | ❌ (service accounts) | ❌ | ❌ | ✅ (first-class) |
| Token delegation | ❌ | ❌ | ❌ | ✅ (RFC 8693) |
| Kill switch | ❌ | ❌ | ❌ | ✅ |
| FGA (Zanzibar) | ❌ (Okta FGA separate) | ❌ | ❌ | ✅ (embedded OpenFGA) |
| Threat detection | ⚠️ (basic) | ❌ | ❌ | ✅ (rules + SLM) |
| Embeddable components | ❌ | ✅ (React only) | ❌ | ✅ (Web Components) |
| MCP / CLI for agents | ❌ | ❌ | ❌ | ✅ |
| Custom schemas | ❌ | ❌ | ✅ (limited) | ✅ (JSON Schema) |

## Architectural Advantages

1. **Event Architecture → Observability**: Zitadel records every mutation as an immutable event. Building an "Identity SIEM" is a native extension of the event pipeline, not a bolt-on.

2. **Delegation Chains → Agent Governance**: Per-user delegation tokens as a first-class primitive. This is the exact problem every AI developer is hacking together with custom DB tables.

3. **Single Binary → Developer Adoption**: One Rust binary that bundles API, UI, and migrations. Downloads and runs in under 60 seconds. Mirrors the success of Tailscale, PocketBase, and similar hyper-adopted tools.

## Target Audiences

| Audience | What They Care About | Our Message |
|---|---|---|
| **Solo dev / startup** | Speed, simplicity, free | "Download, run, authenticate in 60 seconds" |
| **Platform engineer** | Multi-tenancy, SSO, SCIM | "One API for B2C, B2B, and machines" |
| **AI/ML engineer** | Agent auth, delegation, MCP | "First-class agent identity with kill switches" |
| **SecOps / CISO** | Observability, compliance | "Identity SIEM built into your auth layer" |
