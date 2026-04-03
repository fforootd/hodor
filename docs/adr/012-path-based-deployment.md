# ADR-012: Path-Based Deployment & Production Hardening

- **Status**: Accepted
- **Date**: 2026-03-28
- **Authors**: Zitadel Architecture Team

## Context

Zitadel must support diverse deployment topologies:

1. **Root-level deployment** — Zitadel at `https://auth.example.com/`
2. **Sub-path deployment** — Zitadel at `https://example.com/auth/`
3. **CDN/WAF fronted** — Behind Cloudflare, AWS CloudFront, Akamai, etc.
4. **Multi-proxy chains** — Load balancers, reverse proxies, K8s ingresses

Each topology introduces challenges around routing, client IP resolution, and security headers that must be addressed at the infrastructure level.

## Decision

### 1. Path Resolution Architecture

We adopt a **two-tier path model**:

- **Global `base_path`** — A single prefix (e.g., `/auth`) prepended to all routes.
- **Per-app overrides** (`path_overrides`) — Individual apps (OIDC, SAML, Console, Login, etc.) can diverge from the global prefix.

```
┌─────────────────────────────────────────────┐
│         ServerConfig.BasePath = "/auth"      │
│                                             │
│  ┌─────────┐ ┌─────────┐ ┌──────────────┐  │
│  │ OIDC: / │ │ SAML: / │ │ Console:     │  │
│  │ (root)  │ │ (root)  │ │ /auth/console│  │
│  └─────────┘ └─────────┘ └──────────────┘  │
│                                             │
│  ┌──────────────┐ ┌─────────────────────┐   │
│  │ Login:       │ │ API:                │   │
│  │ /auth/login  │ │ /auth/v1/*          │   │
│  └──────────────┘ └─────────────────────┘   │
└─────────────────────────────────────────────┘
```

**Key Design Decision**: OIDC and SAML default to root (`/`) when a base path is configured. This preserves OIDC Discovery compatibility (`/.well-known/openid-configuration` at the domain root) without requiring clients to know the deployment path.

### 2. Proxy Trust & Client IP Resolution

Correctly resolving the client's real IP address is critical for:
- Rate limiting
- Audit logging
- Geo-based access policies
- Session binding (IP pinning)

We implement a **rightmost-untrusted** strategy for X-Forwarded-For:

```
Client (1.2.3.4) → CDN (CF) → LB (10.0.0.1) → Zitadel

X-Forwarded-For: spoofed.ip, 1.2.3.4, 10.0.0.1
                   └── attacker   └── CDN set   └── LB set

Trusted CIDRs: [10.0.0.0/8]
Result: 1.2.3.4 (rightmost untrusted = correct)
```

Three header modes are supported:
- `standard` — X-Forwarded-For → X-Real-IP → True-Client-IP
- `cloudflare` — CF-Connecting-IP → X-Forwarded-For
- `custom` — User-specified header → X-Forwarded-For

**Security**: Headers are ONLY trusted when the direct connection originates from a configured trusted proxy CIDR.

### 3. Security Response Headers

Every response includes production-grade security headers:

| Header | Default | Purpose |
|---|---|---|
| `Strict-Transport-Security` | `max-age=63072000; includeSubDomains` | HSTS (TLS only) |
| `Content-Security-Policy` | Restrictive default | XSS prevention |
| `X-Frame-Options` | `DENY` | Clickjacking prevention |
| `X-Content-Type-Options` | `nosniff` | MIME type sniffing prevention |
| `Referrer-Policy` | `strict-origin-when-cross-origin` | Information leak prevention |
| `Permissions-Policy` | `camera=(), microphone=()...` | Feature restriction |
| `Cross-Origin-Opener-Policy` | `same-origin` | Cross-origin isolation |

All headers are configurable via TOML or environment variables.

### 4. Per-App Access Control (AppGate)

Individual apps can be:
- **Disabled** — Returns 404 for all requests (hide console from public)
- **IP-restricted** — Only accessible from specific CIDR ranges

This enables deployments where the admin console is restricted to internal networks while the authentication endpoints remain public.

### 5. Frontend Base Path Injection

The frontend uses `window.__ZITADEL_BASE_PATH__` injected at runtime by the Rust server. This means:
- A single frontend build works across all deployment paths
- No build-time configuration needed
- The Vue router and API client dynamically construct correct URLs

## Middleware Pipeline

```
Request → RealIP → SecurityHeaders → AppGate → AuthGate → OTel → Router
```

The order ensures:
1. Real IP is available for all downstream decisions
2. Security headers are set before any response is sent
3. App access is checked before authentication
4. Authentication happens before route dispatch

## Configuration

See [`zitadel.reference.toml`](../../zitadel.reference.toml) for the complete configuration reference.

### Environment Variable Overrides

| Variable | Description |
|---|---|
| `ZITADEL_BASE_PATH` | Global route prefix |
| `ZITADEL_TRUSTED_PROXIES` | Comma-separated CIDR ranges |
| `ZITADEL_PROXY_HEADER_MODE` | `standard`, `cloudflare`, or `custom` |
| `ZITADEL_REAL_IP_HEADER` | Custom header name for client IP |

## Consequences

### Positive
- Zitadel can be deployed on any sub-path without rebuilding
- Correct client IP resolution prevents spoofing attacks
- Security headers are enforced by default
- Console/admin access can be restricted to internal networks

### Negative
- Slightly more configuration surface area
- OIDC clients configured before path migration may need issuer URL updates
- Additional middleware in the hot path (minimal performance impact)

### Risks
- Misconfigured `trusted_proxies` could allow IP spoofing (documented with warnings)
- Overly restrictive CSP could break third-party integrations (configurable override)
