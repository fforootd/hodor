# Design Patterns

Lessons learned from Zitadel v1 and design solutions for v2.

## 1. Unified Identity: One Table, Capabilities Over Types

### The Problem
Separate `Human`, `Machine`, `App` concepts with separate code paths, events, and API endpoints. An AI agent is neither "machine" nor "human." The distinction creates artificial complexity.

### The Solution

**One `entities` table. Behavior is defined by the schema, not a type flag.**

Schemas define what an entity looks like (JSON Schema), how it authenticates (`x-auth-methods`), what the login UI shows (`x-login`), and what gets redacted (`x-sensitive`).

| Old Concept | Schema |
|---|---|
| Human user | `human_user_v1` with password, passkey, email_verify |
| Service user | `service_user_v1` with PAT, API key, client credentials |
| AI agent | `ai_agent_v1` with delegation, client credentials |
| OIDC App | `app_v1` with OIDC client config |

**Any combination is possible.** A human with an API key. An agent with a password for admin access. The schema defines the capabilities.

See [ADR-002](../adr/002-schema-driven-login.md) and [ADR-006](../adr/006-entity-naming-model.md).

## 2. I18n: Three Simple Rules

### Rule 1: API errors are NEVER translated
Return structured error codes. Let the client translate.

```json
{
    "code": "AUTH_INVALID_CREDENTIALS",
    "message": "Invalid credentials",
    "details": { "field": "password" }
}
```

### Rule 2: Login UI uses server-side i18n with binary-embedded assets
JSON translation files embedded in the binary. Language determined by: `Accept-Language` → user preference → org default → instance default.

### Rule 3: Notification templates are per-org, per-language
Customers override defaults per-org, per-language. No override? Fall through to instance default. No instance default? Fall through to built-in English.

## 3. Notification Channels: Pluggable, BYOC

```go
type NotificationChannel interface {
    Type() string                          // "email", "sms", "webhook"
    Send(ctx context.Context, msg Message) error
}
```

**Built-in channels:** SMTP, webhook, log (dev mode).

**BYOC (Bring Your Own Channel):** Configure a webhook channel, receive full event context as JSON, route to Twilio/SendGrid/Slack/whatever. Templates use plain text templating primitives.

## 4. Magic Links = Just Another Auth Method

Magic links are a capability on the identity, not a special flow.

```
1. User enters email
2. Server creates single-use token, stores in DB
3. Server sends email with link
4. User clicks → server validates → session created
5. Redirect with OIDC code
```

~1 day of work. The login UI shows "Email me a link" when the identity's schema has this capability.

## 5. Session API as Single UI Contract

```
              REST Session + Management API
              (source of truth, one contract)
                  ↑                    ↑
    Layer A: Vue SPA              Layer B: Web Components
    (Console, rust-embed)         (@zitadel/elements, npm)
```

**Layer A** — Console UI and Login UI ship in the binary. This is a Vue SPA built with `shadcn-vue` and Tailwind for its component system. Same server, same session, FGA-gated.

**Layer B** — Web Components published as `@zitadel/elements` on npm (~30KB, Lit). Work in React, Vue, Svelte, Angular, vanilla HTML.

### Three Auth Modes

| Mode | How | Who |
|---|---|---|
| **Redirect** | OIDC redirect → hosted login page | Standard apps |
| **Embedded** | `<zitadel-login>` Web Component inline | SPAs |
| **Headless** | Custom UI calls Session API directly | Full-custom |

## 6. Schema Registry for Custom Fields

Customers register JSON Schemas that validate entity data on every write:

```json
{
    "target": "identity.profile",
    "schema": {
        "type": "object",
        "properties": {
            "department": { "type": "string" },
            "employee_id": {
                "type": "string",
                "pattern": "^EMP-[0-9]{6}$"
            }
        },
        "required": ["department", "employee_id"]
    }
}
```

**When a schema is registered:**
- Data is validated against it on every write
- The Console UI dynamically renders form fields
- The `expr` policy engine can reference schema fields
- `x-indexed: true` creates DB indexes on JSON paths for query performance
