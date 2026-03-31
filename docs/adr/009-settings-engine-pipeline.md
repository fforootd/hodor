# ADR-009: Hierarchical Settings & Engine Pipeline

**Status**: Proposed  
**Date**: 2026-03-27  
**Builds on**: ADR-005 (Unified Data Model), ADR-007 (Schema ↔ Engine Interaction), ADR-008 (Meta Schema Catalog)

## Context

Zitadel needs two capabilities that are deeply connected:

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
│  pre_auth    │ ← Captcha policy consumer, device fingerprint, bot detection
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  auth        │ ← Password, passkey, MFA (built-in)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  post_auth   │ ← Risk policy consumer, session binding, claim enrichment
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
| **Identifier Policy** | `pre_auth`, `auth` | instance → org | format, pattern, min/max length |
| **Login Policy** | `pre_auth`, `auth` | instance → org → app | MFA, registration, passwordless |
| **Lockout Policy** | `auth`, `post_auth` | instance → org | max_attempts, duration |
| **Rate Limit** | `on_request` | instance → org → app | rpm, burst, by_ip/by_user |
| **Session Policy** | `post_auth` | instance → org | max_lifetime, idle_timeout |
| **Branding** | `auth` (UI) | instance → org → app | colors, logo, fonts |
| **Notification** | `on_event` | instance → org | SMTP, SMS provider |
| **Domain** | — | org | verified domains, primary |

### 5. Validation Layering: Schema → Settings → Expr

Field validation (e.g., username/identifier format) uses three tiers. Each tier is progressively more powerful. The UI reads all three for inline validation.

#### Tier 1: JSON Schema constraints (native, fast, both sides)

Standard JSON Schema keywords on entity schema fields:

```json
"email": {
  "type": "string",
  "format": "email",
  "pattern": "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$",
  "minLength": 5,
  "maxLength": 255
}
```

- **Backend**: Go `regexp.MatchString()` — runs on every create/update
- **Frontend**: JS `new RegExp(pattern).test(value)` — instant inline validation
- **No engine needed** — standard JSON Schema that every language supports

#### Tier 2: Settings overrides (per-org, cascading)

An `identifier_policy` setting lets each org customize validation rules:

```json
{
  "x-settings": {
    "hierarchy": ["instance", "org"],
    "merge_strategy": "deep_merge"
  },
  "properties": {
    "format":           { "type": "string", "enum": ["email", "phone", "username", "any"], "default": "email" },
    "pattern":          { "type": "string", "default": ".*" },
    "min_length":       { "type": "integer", "default": 3 },
    "max_length":       { "type": "integer", "default": 255 },
    "blocked_patterns": { "type": "array", "items": { "type": "string" } },
    "case_sensitive":   { "type": "boolean", "default": false }
  }
}
```

Org A: `"format": "email"` — identifiers must be email addresses  
Org B: `"format": "username", "pattern": "^[a-z][a-z0-9_-]{2,30}$"` — lowercase usernames only

- **Backend**: Go native regexp, resolved from `effective = instance ← org`
- **Frontend**: fetches `GET /v1/settings/identifier_policy?scope=org&scope_id=123`, shows validation inline
- **No engine needed** — just regexp with cascading overrides

#### Tier 3: Expr rules (conditional, programmable)

For complex logic that can't be expressed as a static pattern:

```json
{
  "x-rule": {
    "stage": "pre_auth",
    "condition": "true",
    "engine": "expr",
    "config": {
      "expression": "not(identifier matches settings.blocked_patterns) && (settings.format == 'email' ? isEmail(identifier) : len(identifier) >= settings.min_length)"
    }
  }
}
```

Use cases:
- Block disposable email domains from a dynamic list
- Conditional format: enterprise orgs require email, free orgs allow username
- Cross-entity uniqueness: identifier must not match any existing display_name

#### Resolution order

```
effective_validation = schema.pattern       ← always applies (base constraint)
                     + settings.pattern     ← org override (if set)
                     + expr rule            ← programmable (if defined)
```

The UI fetches both the schema (`pattern`, `format`) and the effective settings, merges them, and validates client-side before submission.

### 6. Engines in the Pipeline

Each stage can have multiple engines. The engine type determines what runs:

| Engine | Stages | What it does |
|---|---|---|
| `expr` | any | Evaluate an expression, transform data, conditional logic |
| `rate_limit` | `on_request` | Token bucket / sliding window rate limiting |
| `captcha` | `pre_auth` | Challenge provider + policy consumer that gates by server-side risk result |
| `risk` | `pre_auth`, `post_auth` | Built-in evaluator returns score, reasons, and recommended next step |
| `webhook` | `on_event`, `post_auth` | HTTP POST to external URL |
| `fga` | `post_auth` | Fine-grained authorization check |
| `built-in` | `auth` | Core auth flows (password, passkey, etc.) |

Risk is special compared with other engines: the evaluator itself does **not** own the final allow/deny decision. It returns a reusable result and the hook consumer applies policy for the current stage.

For example:

- `pre_auth` may map `require_captcha` to `captcha_required=true`
- `post_auth` may persist the result into session metadata and emit follow-up observation events
- future `on_token` or async consumers may reuse the same contract without re-implementing scoring

### 7. How Rules Cascade

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

### 8. Event Stream Hooks

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

### 9. Catalog Integration

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
