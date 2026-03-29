# ADR-021: Login Flow Schema — Composable Bot Detection & Behavioral Telemetry

**Status:** Accepted
**Date:** 2026-03-29
**Supercedes:** Partially extends ADR-002 (Schema-Driven Login), ADR-009 (Settings Cascade), ADR-010 (Three-Tier Data)

## Context

The hosted login engine (ADR-019) needs protection against automated attacks while preserving privacy and avoiding third-party dependencies. Traditional approaches (reCAPTCHA, hCaptcha) leak user data to external services and have been blocked by privacy-first users.

We also need behavioral telemetry to distinguish genuine user interactions (mouse movements, typing cadence, page transitions) from bot scripts, and to correlate client-side signals with server-side risk decisions.

### Requirements

1. **Self-hosted bot detection** — no external service dependencies
2. **Privacy-first** — no PII, no cookies, GDPR/CCPA compliant fingerprinting
3. **Schema-driven** — flows are entities, not bespoke config
4. **Composable** — captcha, fingerprint, telemetry, rate limiting are independent signals
5. **Observable** — all signals feed into the three-tier data pipeline (ADR-010)
6. **Gradual rollout** — flows can be scoped to instance, org, project, or app (ADR-009)

## Decision

### Login Flows are Entities

Login flows are stored as entities (`schema_type=login_flow`) in the `entities` table. This means they get:
- Schema validation via `login_flow.json`
- Versioning, CRUD, FGA authorization — all for free
- Catalog entry at `/v1/login-flows`
- Settings cascade via ADR-009 (instance → org → project → app)

A user schema can reference a login flow via `x-login-flow: { flow_id: "lf_..." }`. If no flow is set, the instance default is inherited.

### Composable Signal Architecture

Each login flow composes four independent signal modules:

```
┌──────────────────────────────────────────────────┐
│                   Login Flow                      │
│                                                   │
│  ┌──────────┐ ┌──────────────┐ ┌──────────────┐  │
│  │ Captcha  │ │ Fingerprint  │ │  Telemetry   │  │
│  │ (Altcha) │ │(ThumbmarkJS) │ │ (OTel SDK)   │  │
│  └────┬─────┘ └──────┬───────┘ └──────┬───────┘  │
│       │              │                │           │
│  ┌────┴──────────────┴────────────────┴───────┐  │
│  │           Rate Limiter                      │  │
│  │  (per IP / identifier / fingerprint)        │  │
│  └─────────────────────┬──────────────────────┘  │
│                        │                          │
│              ┌─────────┴─────────┐                │
│              │   Risk Scorer     │                │
│              │ computeRiskLevel()│                │
│              └─────────┬─────────┘                │
│                        │                          │
│              ┌─────────┴─────────┐                │
│              │  Session Metadata │                │
│              │  (Tier 1 OLTP)    │                │
│              └───────────────────┘                │
└──────────────────────────────────────────────────┘
```

### Captcha: Altcha Proof-of-Work

We use **Altcha** (self-hosted, zero-dependency PoW) instead of third-party captcha services:

- **Server**: `internal/captcha/altcha.go` — SHA-256 challenge, HMAC-signed, configurable difficulty (1-5)
- **Client**: Browser solves PoW via `crypto.subtle.digest()` — no JS library needed
- **Protocol**: `GET /v1/captcha/challenge` → solve → submit `captcha_submit` action
- **Scoring**: PoW completion + solve timing → captcha score (0.0-1.0)

### Browser Fingerprinting: ThumbmarkJS + Fallback

Two-tier fingerprinting:

1. **ThumbmarkJS** (if `@thumbmarkjs/thumbmarkjs` is installed) — survives private mode, tab switches, cookie clears
2. **Built-in fallback** — canvas + WebGL + navigator + screen + timezone → SHA-256 composite hash

Fingerprint is:
- Collected automatically when a `fingerprint_collect` UINode appears
- Submitted silently to the flow engine
- Stored on the session for returning-user detection

### OTel Telemetry: Browser → Server Correlation

Client-side telemetry is collected by a **zero-dependency fallback tracer** (`web/src/lib/telemetry.ts`):

- **Document load timing** (auto via `PerformanceNavigationTiming`)
- **Step transition spans** (`login.flow.step_transition`)
- **Form submission spans** (`login.flow.submit`)
- **Batched export** every 5s to `POST /v1/otel/traces`

#### OTel Ingest Protection

The `/v1/otel/traces` endpoint is public and requires 4-layer protection:

| Layer | Mechanism | Default |
|---|---|---|
| Rate limit | Per-IP token bucket | 100 spans/min |
| Payload cap | `http.MaxBytesReader` | 64KB |
| Flow scoping | `X-Flow-ID` header | Links to active flow |
| Tail sampling | `shouldSampleSpan()` | Keep: errors, slow >3s, page loads, flow-linked |

### Risk Scoring

`computeRiskLevel()` combines all signals into a single risk level:

| Signal | Weight | Logic |
|---|---|---|
| PoW completion | 0.3 | Solved + timing reasonable |
| Solve timing | 0.1 | < 50ms = suspicious |
| Fingerprint | 0.2 | Present = lower risk |
| Captcha score | 0.2 | From Altcha scoring |
| Trace presence | 0.1 | OTel traces = real browser |
| Document load | 0.1 | < 100ms = suspicious |

Threshold: `≥ 0.5 = low`, `≥ 0.3 = medium`, `< 0.3 = high`.

### Schema Annotations

User schemas can embed signal config via `x-` annotations:

```json
{
  "x-captcha": { "provider": "altcha", "mode": "risk_based", "difficulty": 3 },
  "x-fingerprint": { "enabled": true, "provider": "thumbmarkjs", "persist": true },
  "x-rate-limit": { "max_attempts": 5, "scope": "ip" },
  "x-login-flow": { "flow_id": "lf_prod_mfa" }
}
```

All annotation keys are validated at schema-write time (not at login time).

### Three-Tier Data Integration (ADR-010)

| Tier | What | Where |
|---|---|---|
| Tier 1 (OLTP) | `sessions.risk_level` summary | SQLite/Postgres |
| Tier 2 (OLAP) | Full signal payload (`signal.session_*` events) | Logger → cache → drain |
| Tier 2 (OLAP) | OTel traces (`signal.session_trace` events) | Logger → cache → drain |

Default retention: **7 days** (configurable via `zitadel.yaml`).

## Alternatives Considered

### Third-party Captcha Services (hCaptcha, reCAPTCHA, Turnstile)

Rejected as _defaults_ because:
- Require external API keys and network calls
- Privacy concerns (user data sent to Google/Cloudflare)
- Not self-hostable

However, the schema supports these as providers — they can be configured in `x-captcha.provider`.

### Altcha Sentinel (Managed)

Sentinel is Altcha's paid managed service. Provides spam filtering + analytics. Rejected as a default because it's paid, but could be a marketplace template.

### FingerprintJS Pro

Rejected because it's a paid service. ThumbmarkJS is open-source and provides similar accuracy for our use case (returning-user detection, not cross-device tracking).

## Consequences

### Positive
- **Zero external dependencies** for default bot detection
- **Privacy-compliant** — no PII leaves the system
- **Observable** — all signals visible in the analytics dashboard
- **Extensible** — captcha providers, fingerprint providers, and rate limit scopes are pluggable
- **Schema-driven** — flows are entities, managed through the same CRUD as everything else

### Negative
- Altcha PoW is less effective against sophisticated bots than ML-based services
- Built-in fingerprinting is less accurate than FingerprintJS Pro
- OTel ingest endpoint adds write amplification (mitigated by tail sampling)

### Migration
- No breaking changes. Existing flows without `x-captcha`/`x-fingerprint` work as before
- New flows default to `captcha: risk_based` + `fingerprint: enabled`
