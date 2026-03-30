# ADR-022: Provider Catalog, Schema Binding, and Session Provenance

**Status**: Accepted  
**Date**: 2026-03-30  
**Builds on**: [ADR-015](015-actions-catalog.md), [ADR-019](019-server-driven-login-wc.md), [ADR-005](005-unified-data-model.md)

## Context

Providers already existed as installable catalog entries and runtime resources, but the implementation had three gaps:

1. The catalog payload shape, provider API shape, and provider runtime shape were not the same contract.
2. Provider-to-user-schema targeting was implicit, falling back to the default human-user schema.
3. Sessions and auth events did not persist enough provider/flow provenance to explain how a session was created.

This made marketplace-style provider installation awkward and made social/enterprise provider behavior harder to reason about.

## Decision

### 1. Canonical provider shape

Providers use one canonical payload shape:

```json
{
  "display_name": "Google",
  "kind": "google",
  "protocol": "oidc",
  "connection": {
    "issuer": "https://accounts.google.com",
    "client_id": "...",
    "client_secret": "...",
    "scopes": ["openid", "profile", "email"]
  },
  "mapping": {
    "claims": {
      "email": "claims.email",
      "display_name": "claims.name"
    }
  },
  "target": {
    "schema_type": "human_user"
  },
  "linking": {
    "mode": "create_or_link",
    "match_by": "verified_email"
  },
  "session": {},
  "ui": {
    "display_order": 10
  },
  "enabled": true,
  "catalog_ref": {
    "template_id": "google-oidc",
    "template_version": "1.0.0",
    "official": true
  }
}
```

### 2. Naming

- `kind` identifies the marketplace/provider family: `google`, `github`, `gitlab`, `entra`, `custom`
- `protocol` identifies the runtime adapter: `oidc`, `oauth2`, `saml`
- `catalog_ref` tracks marketplace origin and version

The old `template` field remains only as a compatibility/storage bridge while the `providers` table still exists.

### 3. Provider-driven schema targeting

Providers explicitly declare their user target via:

- `target.schema_id` for an exact schema version
- `target.schema_type` for “current default schema of this type”

SSO linking and first-login create use the provider target as the source of truth.

### 4. Linking contract

Provider linking is explicit:

- `linking.mode`: `create_or_link` or `link_only`
- `linking.match_by`: `verified_email`, `identifier`, or `none`

Default for OIDC/OAuth-style providers is `create_or_link + verified_email`.

### 5. Flow interaction

Login flows own provider presentation, not provider data. Flow config may now narrow SSO choices with:

```json
{
  "sso": {
    "providers": {
      "mode": "allowlist",
      "ids": ["prov_google", "prov_github"]
    }
  }
}
```

If omitted, all enabled compatible providers are shown.

### 6. Session and event provenance

Session creation persists the following provenance in session metadata and `session.created` / `auth.sso_login` events:

- `auth_method`
- `provider_id`
- `provider_kind`
- `login_flow_id`
- `auth_context`

This makes SSO-created sessions explainable and auditable.

### 7. Catalog behavior

Zitadel ships with official provider templates preloaded in the catalog, but does not auto-install provider instances on bootstrap.

`/v1/catalog?type=provider` is the primary provider marketplace surface.  
`/v1/providers/templates` remains as a compatibility endpoint and is marked deprecated.

## Consequences

- Catalog install, provider API, and runtime now share one provider contract.
- Social and enterprise providers can carry typed mappings plus explicit schema targeting.
- Flow/provider/session boundaries are clearer:
  - provider = connection + mapping + target + linking
  - flow = UX + provider selection
  - session = created auth state with provenance
- The `providers` table remains during the transition, but canonical provider JSON is stored in metadata so the runtime can behave as if providers are first-class schema entities already.
