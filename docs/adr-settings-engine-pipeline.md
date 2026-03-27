# ADR-009: Hierarchical Settings & Engine Pipeline

**Status**: Proposed  
**Date**: 2026-03-27  
**Builds on**: ADR-005 (Unified Data Model), ADR-007 (Schema ↔ Engine Interaction), ADR-008 (Meta Schema Catalog)

## Context

ZITADEL needs two capabilities that are deeply connected:

1. **Hierarchical settings** — password policies, login policies, rate limits that cascade from instance → org → project → app
2. **An engine pipeline** — pluggable processing stages where rules (expr, webhooks, captcha, risk scoring) execute at defined points in the request lifecycle

Currently, settings like `x-login` and `x-branding` are baked into entity schemas. There's no way to:
- Override a password policy per-org
- Run a captcha engine before authentication
- Fire a webhook when an entity changes
- Score login risk and step up to MFA conditionally

### The insight

> **Settings are just schemas. Rules are just expr programs attached to pipeline stages.**

Both follow the same cascade: `instance → org → project → app`.  
Both are declared in the catalog.  
Both resolve at runtime via merge.

## Decision

### 1. Engine Pipeline Stages

Every request flows through a pipeline of **stages**. Each stage is a hook point where engines can execute:

```
HTTP Request
    │
    ▼
┌──────────────┐
│  on_request  │ ← Rate limiter, IP blocklist, GeoIP
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  pre_auth    │ ← Captcha, device fingerprint, bot detection
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  auth        │ ← Password, passkey, MFA (built-in)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  post_auth   │ ← Risk scoring, session binding, claim enrichment
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  on_token    │ ← Claim mapping, scope filtering, custom claims
└──────┬───────┘
       │
       ▼
   Response

       ─── async ───

┌──────────────┐
│  on_event    │ ← Webhooks, notifications, sync, audit enrichment
└──────────────┘
```

### 2. Rules as Catalog Entries

Rules are **expr programs** bound to pipeline stages:

```json
{
  "type": "rule",
  "x-rule": {
    "stage": "on_request",
    "priority": 10,
    "condition": "request.path startsWith '/v1/' && request.headers['cf-connecting-ip'] != ''",
    "engine": "rate_limit",
    "config": {
      "key": "request.headers['cf-connecting-ip']",
      "limit": "settings.rate_limit.requests_per_minute",
      "window": "1m"
    }
  }
}
```

The catalog entry:

```json
"rule": {
  "schema_file": "schemas/rule.json",
  "group": "configure",
  "alias": "Rules",
  "singular": "Rule",
  "path": "rules",
  "icon": "⚡",
  "storage": "entities",
  "components": ["x-rule"]
}
```

### 3. Hierarchical Settings

Settings schemas define **policy shapes** that cascade through scopes:

```json
{
  "type": "object",
  "x-settings": {
    "hierarchy": ["instance", "org", "project", "app"],
    "merge_strategy": "deep_merge",
    "inherit": true
  },
  "properties": {
    "min_length":     { "type": "integer", "default": 8 },
    "require_upper":  { "type": "boolean", "default": true },
    "require_number": { "type": "boolean", "default": true },
    "require_symbol": { "type": "boolean", "default": false },
    "history_count":  { "type": "integer", "default": 0 },
    "max_age_days":   { "type": "integer", "default": 0 }
  }
}
```

#### Resolution

```
effective = instance_default ← org_override ← project_override ← app_override
```

Using `deep_merge`: only the fields explicitly set at a lower scope override the parent.

#### Storage

```sql
CREATE TABLE settings (
  id         INTEGER PRIMARY KEY,
  type       TEXT NOT NULL,       -- 'password_policy', 'login_policy', ...
  scope      TEXT NOT NULL,       -- 'instance' | 'org' | 'project' | 'app'
  scope_id   TEXT DEFAULT '',     -- org_id / project_id / app_id
  data       TEXT NOT NULL,       -- JSON (overrides only, not full policy)
  created_at DATETIME,
  updated_at DATETIME,
  UNIQUE(type, scope, scope_id)
);
```

### 4. Built-in Settings Types

| Setting | Stages affected | Hierarchy | Key fields |
|---|---|---|---|
| **Password Policy** | `auth` | instance → org | min_length, complexity, history |
| **Login Policy** | `pre_auth`, `auth` | instance → org → app | MFA, registration, passwordless |
| **Lockout Policy** | `auth`, `post_auth` | instance → org | max_attempts, duration |
| **Rate Limit** | `on_request` | instance → org → app | rpm, burst, by_ip/by_user |
| **Session Policy** | `post_auth` | instance → org | max_lifetime, idle_timeout |
| **Branding** | `auth` (UI) | instance → org → app | colors, logo, fonts |
| **Notification** | `on_event` | instance → org | SMTP, SMS provider |
| **Domain** | — | org | verified domains, primary |

### 5. Engines in the Pipeline

Each stage can have multiple engines. The engine type determines what runs:

| Engine | Stages | What it does |
|---|---|---|
| `expr` | any | Evaluate an expression, transform data, conditional logic |
| `rate_limit` | `on_request` | Token bucket / sliding window rate limiting |
| `captcha` | `pre_auth` | hCaptcha / reCAPTCHA challenge |
| `risk` | `post_auth` | Risk score (device, geo, velocity) → step-up MFA |
| `webhook` | `on_event`, `post_auth` | HTTP POST to external URL |
| `fga` | `post_auth` | Fine-grained authorization check |
| `built-in` | `auth` | Core auth flows (password, passkey, etc.) |

### 6. How Rules Cascade

Rules follow the same cascade as settings, but **additive** instead of override:

```
Instance rules:  [rate_limit, audit_log]
    +
Org rules:       [captcha_on_login, webhook_slack]
    +
App rules:       [custom_claim_enrichment]
    =
Effective:       [rate_limit, audit_log, captcha_on_login, webhook_slack, custom_claim_enrichment]
```

Priority ordering within each stage determines execution order.

### 7. Event Stream Hooks

The `on_event` stage is special — it's **asynchronous** and fires after the event is persisted:

```json
{
  "x-rule": {
    "stage": "on_event",
    "condition": "event.type == 'entity.created' && event.schema_type == 'human_user'",
    "engine": "webhook",
    "config": {
      "url": "https://hooks.slack.com/...",
      "method": "POST",
      "body": "json({ text: 'New user: ' + event.identifier })"
    }
  }
}
```

Use cases:
- **Webhooks** — notify Slack, sync to CRM, trigger provisioning
- **Notifications** — send welcome email on user creation
- **Audit enrichment** — add GeoIP data to events
- **Sync** — push changes to external SCIM directory

### 8. Catalog Integration

```json
{
  "x-catalog": {
    "password_policy":  { "group": "settings", "storage": "settings", "nav": "hidden", ... },
    "login_policy":     { "group": "settings", "storage": "settings", "nav": "hidden", ... },
    "lockout_policy":   { "group": "settings", "storage": "settings", "nav": "hidden", ... },
    "rate_limit":       { "group": "settings", "storage": "settings", "nav": "hidden", ... },
    "session_policy":   { "group": "settings", "storage": "settings", "nav": "hidden", ... },
    "rule":             { "group": "configure", "storage": "entities", ... }
  }
}
```

Settings are `nav: "hidden"` — they appear inside org/app detail pages, not as top-level nav items.

## The Full Picture

```
                        META SCHEMA
                    ┌─────────────────┐
                    │  x-catalog      │ ← what types exist
                    │  x-groups       │ ← how to navigate
                    └───────┬─────────┘
                            │
              ┌─────────────┼─────────────┐
              │             │             │
         ┌────▼────┐  ┌─────▼─────┐  ┌───▼────┐
         │ Schemas │  │ Settings  │  │ Rules  │
         │ (shape) │  │ (cascade) │  │ (expr) │
         └─────────┘  └───────────┘  └────────┘
              │             │             │
              └─────────────┼─────────────┘
                            │
                    ┌───────▼───────┐
                    │   PIPELINE    │
                    │               │
                    │  on_request ──┼── rate_limit, geo
                    │  pre_auth  ──┼── captcha, bot
                    │  auth      ──┼── password, passkey
                    │  post_auth ──┼── risk, fga, webhook
                    │  on_token  ──┼── claims, scopes
                    │  on_event  ──┼── webhook, notify
                    └───────────────┘
```

## Consequences

- **Settings are schema-driven** — password policy shape is defined by `schemas/password_policy.json`, inheritable through scopes
- **Rules are pluggable** — any `expr` program can be attached to any pipeline stage
- **Cascading is uniform** — settings deep-merge, rules are additive, same hierarchy for both
- **Pipeline is extensible** — new engines (risk scoring, captcha providers) = new catalog entries + `x-rule.engine` value
- **Event stream is first-class** — webhooks, notifications, sync all use the same `on_event` rule model
- **UI is declarative** — settings appear in context (org detail → Policies tab), rules appear under Configure → Rules
