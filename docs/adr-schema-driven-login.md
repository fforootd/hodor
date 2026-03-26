# ADR: Schema-Driven Identity, Auth, and Login Flows

**Date:** 2026-03-26  
**Status:** Accepted  
**Context:** POC identity platform with flexible, customer-defined schemas

## Decision

All identity behavior — data shape, auth methods, claim mapping, redaction, and login flows — is configured through **JSON Schema annotations** (`x-*` extensions). There is no separate "login policy" table or external configuration file.

## The Annotation Model

Identity schemas are standard JSON Schema documents extended with `x-*` annotations at two levels:

### Per-field annotations (on schema properties)

| Annotation | Purpose | Example |
|---|---|---|
| `x-claim-mapping` | Map SSO/OIDC claims to this field | `"claims.email"` |
| `x-sensitive` | PII redaction in audit events | `true` |
| `x-hidden` | Hidden from non-admin API responses | `true` |
| `x-user-editable` | Self-service edit permission | `true` |
| `x-source` | Field ownership | `"admin"` |
| **`x-auth`** | Auth behavior for this field | `{"identifier": true, "verification": "email"}` |

### Schema-level annotations

| Annotation | Purpose |
|---|---|
| **`x-login`** | Login flow configuration: preset, auth methods, MFA rules |
| **`x-branding`** | Visual customization: heading, colors, texts, CSS |

### Full example

```json
{
  "type": "object",
  "x-login": {
    "preset": "identifier_first",
    "auth_methods": {
      "password":   {"enabled": true, "position": 1},
      "passkey":    {"enabled": false, "position": 0},
      "magic_link": {"enabled": true, "position": 2},
      "sso":        {"enabled": true, "position": 3}
    },
    "mfa_required": false,
    "registration_allowed": true
  },
  "x-branding": {
    "heading": "Welcome to Acme",
    "colors": {"primary": "#ff6600"}
  },
  "properties": {
    "email": {
      "type": "string",
      "format": "email",
      "x-claim-mapping": "claims.email",
      "x-auth": {"identifier": true, "verification": "email", "recovery": "email"}
    },
    "phone": {
      "type": "string",
      "x-sensitive": true,
      "x-auth": {"identifier": true, "mfa": "sms"}
    }
  }
}
```

## Rationale

1. **Single source of truth.** The schema defines what an identity looks like (data), how it authenticates (x-auth), what the login UI shows (x-login, x-branding), how claims map from SSO (x-claim-mapping), and what gets redacted (x-sensitive). No config drift between separate tables.

2. **Per-schema login flows.** Different schemas can have different login behaviors. "Employees" authenticate via SSO-only. "Customers" use identifier-first + password + magic link. The schema IS the policy.

3. **Dynamic via API.** Schemas are created and updated via `POST/PATCH /v1/schemas` — no config files, no restarts. Changes take effect immediately.

4. **Composable with the flow API.** The flow engine reads annotations at runtime and generates UI nodes. Three tiers of customization:
   - **Preset config** (80%): set `x-login.preset` + toggle methods
   - **Step array** (15%): set `preset: "custom"` + define step order
   - **Headless** (5%): call `POST /v1/login/flows` and render your own UI

5. **JSON Schema is standard.** The `x-*` extension pattern is explicitly supported by JSON Schema spec. Validators ignore unknown extensions. No custom DSL to learn.

## What was added

| File | Purpose |
|---|---|
| `internal/login/flow.go` | Schema annotation extraction, flow state machine, UI node builder |
| `internal/login/flow_handlers.go` | HTTP handlers for flow API (create, submit, get) |
| `internal/login/flow_test.go` | Tests for annotation extraction, node builder, flow store |

## What was modified

| File | Change |
|---|---|
| `internal/login/login.go` | Branding + auth settings now schema-driven; flow API routes registered |
| `internal/bootstrap/bootstrap.go` | Default human_user schema now includes `x-auth` and `x-login` annotations |

## Alternatives considered

| Approach | Why not |
|---|---|
| **Separate login_policies table** | Config drift with schemas; extra CRUD surface |
| **Custom DSL (à la Janssen Agama)** | Steep learning curve; no IDE support; Java dependency |
| **File-based config (à la Ory Kratos)** | Requires restart to apply changes; no dynamic multi-schema |
| **Hard-coded presets only** | Insufficient flexibility for power users |
