# ADR-002: Schema-Driven Identity, Auth, and Login Flows

**Status**: Accepted  
**Date**: 2026-03-26  
**Amended by**: [ADR-003](003-auth-methods-meta-schema.md) (Unified Auth Methods)  
**Amended**: 2026-03-30 — `x-login` and `x-branding` moved from user schemas to login flows

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

| Annotation | Purpose | Where |
|---|---|---|
| **`x-auth-methods`** | Auth methods available for this user type (narrower override) | User schema |
| ~~`x-login`~~ | ~~Login flow configuration~~ | **Moved to login flow** (see Amendment below) |
| ~~`x-branding`~~ | ~~Visual customization~~ | **Moved to login flow** (see Amendment below) |

### Full example

```json
{
  "type": "object",
  "x-auth-methods": {
    "password":   {"enabled": true,  "interactive": true,  "position": 1},
    "passkey":    {"enabled": false, "interactive": true,  "position": 0},
    "magic_link": {"enabled": true,  "interactive": true,  "position": 2},
    "sso":        {"enabled": true,  "interactive": true,  "position": 3}
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

> **Note:** `x-login` and `x-branding` are no longer on user schemas. They live on the **login flow** entity (`login_flows` table). See Amendment below.

## Rationale

1. **Single source of truth.** The user schema defines what an identity looks like (data), how it authenticates per-field (x-auth), and which auth methods are available as narrower overrides (x-auth-methods). The login flow defines the UX: branding, preset, captcha, registration config.

2. **Per-flow login experiences.** Different login flows can target different audiences. "Enterprise" flow uses SSO-only. "Consumer" flow uses identifier-first + password + magic link. Audience targeting (org, schema, app, user) determines which flow is served.

3. **Dynamic via API.** Schemas are created and updated via `POST/PATCH /v1/schemas` — no config files, no restarts. Changes take effect immediately.

4. **Composable with the flow API.** The flow engine reads annotations at runtime and generates UI nodes. Three tiers of customization:
   - **Preset config** (80%): set `x-login.preset` + toggle methods
   - **Step array** (15%): set `preset: "custom"` + define step order
   - **Headless** (5%): call `POST /v1/login/flows` and render your own UI

5. **JSON Schema is standard.** The `x-*` extension pattern is explicitly supported by JSON Schema spec. Validators ignore unknown extensions. No custom DSL to learn.

## Meta-Schema: The Canonical Zitadel Identity Schema

All `x-*` annotations are defined in a single **meta-schema** (`internal/schema/meta_schema.json`). This is not just an internal validation tool — it is **THE** schema definition for Zitadel identity schemas. It defines the allowed structure and vocabulary of `x-auth-methods`, `x-login`, `x-branding`, per-field `x-auth`, `x-claim-mapping`, `x-sensitive`, `x-hidden`, `x-user-editable`, and `x-source`.

See [ADR-003: Unified Auth Methods and Meta-Schema Validation](./003-auth-methods-meta-schema.md) for the full decision on `x-auth-methods` and the meta-schema introduction.

### Public exposure

The meta-schema **must be publicly accessible** — it's the contract between Zitadel and every customer, SDK, and integration:

| Endpoint | Purpose |
|---|---|
| `GET /v1/schemas/$meta` | Returns the meta-schema JSON — the canonical definition of what a valid identity schema looks like |
| `GET /.well-known/zitadel-identity-schema` | Optional well-known endpoint for discovery |

### What this enables

- **SDK generation**: SDKs can fetch the meta-schema to validate schemas client-side before `POST /v1/schemas`
- **Monaco autocomplete**: The console JSON editor loads the meta-schema for IntelliSense on `x-*` keys
- **Customer documentation**: The meta-schema IS the documentation — no separate prose needed
- **Schema evolution**: The meta-schema `$id` includes a version path (e.g., `/v1/`) so it can evolve without breaking existing schemas

### Versioning

The meta-schema `$id` should include a version: `https://zitadel.com/schemas/v1/identity-meta-schema`. When new annotations are added, the version increments. Old schemas remain valid — new annotations are additive.

## Default Schema & Gradual Rollout

### Problem

Today, `getDefaultSchemaConfig()` picks the oldest schema (`ORDER BY created_at ASC LIMIT 1`). There's no way to:
- Deploy a new schema version alongside the old one
- Roll it out to a subset of identities
- Roll back without deleting the schema

### Decision: `is_default` flag + identity-level `schema_id` override

#### 1. Multiple schema versions coexist as separate rows

The `schemas` table already has `id`, `type`, and `version` columns. Remove the unique constraint on `(type, org_id)` to allow multiple versions per type:

```
human_user_v1  (is_default: true,  version: 1)
human_user_v2  (is_default: false, version: 2)  ← new login flow, rolling out
```

Each row is a self-contained JSON Schema blob with its own annotations.

#### 2. `is_default` flag

```sql
ALTER TABLE schemas ADD COLUMN is_default BOOLEAN DEFAULT false;
```

One schema per `(type, org_id)` is marked `is_default = true`. Setting a new default via `PATCH /v1/schemas/{id}` automatically unsets the previous default.

#### 3. Schema resolution order

The flow engine resolves the schema for a given identity:

```
1. If identity.schema_id is set and non-empty → use that schema
2. Else → use the is_default=true schema for the identity's type + org
```

Before identity is known (pre-login branding/auth settings), always use the default.

#### 4. Assignment via existing bulk endpoint

Customers assign the new schema to specific identities using the existing bulk identity endpoint:

```http
PATCH /v1/identities/bulk
{
  "filter": {"state": "active", "metadata.cohort": "beta"},
  "update": {"schema_id": "human_user_v2"}
}
```

This enables gradual rollout:
1. Create `human_user_v2` (not default)
2. Bulk-assign to beta cohort
3. Monitor login success rates, support tickets
4. If good → `PATCH /v1/schemas/human_user_v2 {"is_default": true}`
5. If bad → bulk-reassign affected identities back to `human_user_v1`

#### Use cases

| Scenario | How |
|---|---|
| **Gradual rollout** | Assign new schema to 10% of identities, monitor, expand |
| **A/B testing** | Different login flows for different cohorts |
| **Migration safety** | Rollback by bulk-reassigning identities to old schema |
| **Per-customer config** | Different orgs can have different defaults |

## What was added

| File | Purpose |
|---|---|
| `internal/login/` pkg | Schema annotation extraction, flow state machine, UI node builder, flow HTTP handlers, tests |

## What was modified

| File | Change |
|---|---|
| `internal/login/` pkg | Branding + auth settings now schema-driven; flow API routes registered |
| `internal/bootstrap/` pkg | Default schemas updated with annotations (`x-auth`, `x-login`) |

## Alternatives considered

| Approach | Why not |
|---|---|
| **Separate login_policies table** | Config drift with schemas; extra CRUD surface |
| **Custom DSL (à la Janssen Agama)** | Steep learning curve; no IDE support; Java dependency |
| **File-based config (à la Ory Kratos)** | Requires restart to apply changes; no dynamic multi-schema |
| **Hard-coded presets only** | Insufficient flexibility for power users |

## Amendment: Login Flow Separation (2026-03-30)

### What changed

`x-login` and `x-branding` have been **removed from user schemas** and moved to the **login flow** entity (`login_flows` table). The user schema no longer defines UX/presentation concerns.

### New responsibility boundaries

| Concern | Where | Annotation/Field |
|---|---|---|
| Field definitions, types, validation | User schema | `properties` |
| Per-field auth behavior (identifier, verify, recover, MFA) | User schema | `x-identifier`, `x-verify`, `x-recover`, `x-mfa` |
| Auth method narrowing per user type | User schema | `x-auth-methods` |
| Claim mapping from SSO | User schema | `x-claim` |
| PII redaction, visibility | User schema | `x-sensitive`, `x-hidden` |
| Login preset, MFA, registration | **Login flow** | `preset`, `x-login` in flow config |
| Branding (heading, colors, layout, CSS) | **Login flow** | `branding` in flow config |
| Captcha, fingerprint, rate limiting | **Login flow** | `captcha`, `fingerprint`, `rate_limit` |
| Registration field selection | **Login flow** | `registration.fields`, `registration.user_schema` |
| Audience targeting | **Login flow** | `audience` (org_ids, schema_ids, user_ids, app_ids) |

### Auth methods cascade

Login flow provides the **base** available auth methods. User schema provides **narrower overrides** — it can restrict but not widen.

```
Login flow auth_methods (broad defaults)
  ↓ narrowed by
User schema x-auth-methods (per-type restrictions)
```

Example: The login flow enables `password + magic_link + sso`. A `service_account` schema can disable `password` and `magic_link`, leaving only API-based auth. But it cannot enable a method the flow hasn't enabled.

### Why

The original ADR assumed a 1:1 mapping between user schema and login experience. In practice, customers need multiple login experiences for the same user schema (e.g., "Quick signup" vs. "Full signup" for `human_user`). The login flow entity decouples UX from data shape, enabling audience-targeted, A/B-testable login experiences.

