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
  "x-auth-methods": {
    "password":   {"enabled": true,  "interactive": true,  "position": 1},
    "passkey":    {"enabled": false, "interactive": true,  "position": 0},
    "magic_link": {"enabled": true,  "interactive": true,  "position": 2},
    "sso":        {"enabled": true,  "interactive": true,  "position": 3}
  },
  "x-login": {
    "preset": "identifier_first",
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

## Meta-Schema: The Canonical ZITADEL Identity Schema

All `x-*` annotations are defined in a single **meta-schema** (`internal/schema/meta_schema.json`). This is not just an internal validation tool — it is **THE** schema definition for ZITADEL identity schemas. It defines the allowed structure and vocabulary of `x-auth-methods`, `x-login`, `x-branding`, per-field `x-auth`, `x-claim-mapping`, `x-sensitive`, `x-hidden`, `x-user-editable`, and `x-source`.

See [ADR: Unified Auth Methods and Meta-Schema Validation](./adr-auth-methods-meta-schema.md) for the full decision on `x-auth-methods` and the meta-schema introduction.

### Public exposure

The meta-schema **must be publicly accessible** — it's the contract between ZITADEL and every customer, SDK, and integration:

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
