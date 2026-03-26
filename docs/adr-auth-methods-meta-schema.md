# ADR: Unified Auth Methods and Meta-Schema Validation

**Date:** 2026-03-26  
**Status:** Accepted  
**Supersedes:** Partial overlap with `adr-schema-driven-login.md` (auth_methods section)

## Problem

The current `x-login.auth_methods` annotation only covers **interactive** (browser-based) authentication. But authentication is a spectrum:

- **Human users** need interactive login (password, passkey, magic link) + may also use PATs for CI scripts
- **Service users** need non-interactive auth (PAT, API key, client certificate) + may occasionally need interactive login for console access
- **AI agents** need delegation tokens, API keys, or OAuth client credentials

Today, `service_user_v1` and `ai_agent_v1` schemas have **zero auth configuration**. If an admin creates a service user, there's no schema-driven way to specify what auth methods are available. The auth methods are effectively hardcoded per identity type, which defeats the purpose of schema-driven identity.

## Decision

### 1. Replace `x-login.auth_methods` with root-level `x-auth-methods`

Auth methods move from inside `x-login` to a top-level annotation. Each method declares whether it's `interactive` (browser-based) or non-interactive (API/token-based):

```json
"x-auth-methods": {
  "password":      { "enabled": true,  "interactive": true,  "position": 1 },
  "magic_link":    { "enabled": true,  "interactive": true,  "position": 2 },
  "passkey":       { "enabled": false, "interactive": true,  "position": 0 },
  "sso":           { "enabled": true,  "interactive": true,  "position": 3 },
  "pat":           { "enabled": false, "interactive": false },
  "api_key":       { "enabled": false, "interactive": false },
  "client_cert":   { "enabled": false, "interactive": false }
}
```

### 2. `x-login` references interactive methods from `x-auth-methods`

`x-login` no longer defines methods — it configures **how** the interactive methods are presented:

```json
"x-login": {
  "preset": "identifier_first",
  "mfa_required": false,
  "registration_allowed": true
}
```

The flow engine reads enabled interactive methods from `x-auth-methods`, not from `x-login.auth_methods`.

### 3. Every schema can declare auth methods

Service users gain auth config:

```json
{
  "type": "object",
  "x-auth-methods": {
    "pat":         { "enabled": true,  "interactive": false, "max_tokens": 10 },
    "api_key":     { "enabled": true,  "interactive": false },
    "password":    { "enabled": true,  "interactive": true,  "position": 1 }
  },
  "x-login": {
    "preset": "identifier_first"
  },
  "properties": { ... }
}
```

### 4. Introduce a meta-schema that validates annotations

A JSON Schema meta-schema validates that `x-*` annotations are well-formed. This enables:
- Real-time validation in the Monaco editor
- API-side validation on `PATCH /v1/schemas/{id}`
- Documentation of all valid annotations in one place

The meta-schema defines the allowed `x-auth-methods` keys, the structure of `x-login`, `x-branding`, and per-field `x-auth`.

## Auth Method Registry

| Method | Interactive | Description |
|---|---|---|
| `password` | ✅ | Password-based login |
| `passkey` | ✅ | WebAuthn/FIDO2 passkey |
| `magic_link` | ✅ | Email sign-in link |
| `sso` | ✅ | SSO/OIDC redirect |
| `pat` | ❌ | Personal access token (Bearer) |
| `api_key` | ❌ | API key (header/secret) |
| `client_cert` | ❌ | mTLS client certificate |

## Schema Defaults by Type

| Schema Type | Default Interactive | Default Non-Interactive |
|---|---|---|
| `human_user` | password ✅, magic_link ✅, passkey ❌, sso ✅ | pat ❌ |
| `service_user` | password ❌ | pat ✅, api_key ✅ |
| `ai_agent` | — | pat ✅, client_cert ❌ |
| `app` | — | — (uses OAuth grants) |

## What Changes

### Backend

| File | Change |
|---|---|
| `internal/login/flow.go` | `SchemaAuthConfig` gains `AuthMethods map[string]*AuthMethodEntry`; `LoginConfig.AuthMethods` removed; extraction reads `x-auth-methods` |
| `internal/login/flow.go` | New `AuthMethodEntry` struct with `Interactive bool`; node builder reads interactive methods from `AuthMethods` |
| `internal/bootstrap/bootstrap.go` | All 4 built-in schemas updated with `x-auth-methods` |

### Frontend

| File | Change |
|---|---|
| `SchemaDetailView.vue` | Sidebar "Login Flow" toggles read/write `x-auth-methods`; new "API Auth" section for non-interactive methods |

### New File

| File | Purpose |
|---|---|
| `internal/schema/meta_schema.go` | Embedded meta-schema JSON for annotation validation |

## Migration Path

`ExtractAuthConfig` supports both the old format (`x-login.auth_methods`) and the new format (`x-auth-methods`) with a fallback: if `x-auth-methods` is absent but `x-login.auth_methods` is present, it auto-converts. This ensures backward compatibility.

## Alternatives Considered

| Approach | Why not |
|---|---|
| Keep methods inside `x-login` | Can't represent non-interactive methods; semantically wrong for service users |
| Separate `auth_config` table | Defeats single-source-of-truth; config drift |
| Per-type hardcoded methods | Inflexible; can't customize per-org or per-schema instance |
