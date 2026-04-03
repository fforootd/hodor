# Design: SAML SP/IDP, SCIM 2.0 Server, LDAP Client & Server

## Context

The prototype already implements OIDC provider, a generic provider model (with `protocol` field supporting "oidc", "oauth2", "saml"), user/group/membership tables, and a login flow with SSO. This plan adds four major identity protocol capabilities to make the platform a comprehensive identity hub.

---

## 1. SAML (SP + IDP)

### New Crate: `crates/zitadel-saml/`

```
src/
  lib.rs              -- SamlState, routes()
  xml/
    mod.rs            -- XML utilities
    canonicalize.rs   -- Exclusive C14N
    signature.rs      -- XML-DSig sign/verify (using rsa + sha2)
    schema.rs         -- SAML type definitions (quick-xml + serde)
  idp/
    mod.rs            -- IDP router
    metadata.rs       -- GET /saml/metadata (EntityDescriptor XML)
    sso.rs            -- GET+POST /saml/sso (receive AuthnRequest, issue Response)
    slo.rs            -- GET+POST /saml/slo
    assertion.rs      -- SAML assertion builder
    response.rs       -- SAML response builder with signing
  sp/
    mod.rs            -- SP service
    metadata.rs       -- SP metadata generation
    authn_request.rs  -- Build AuthnRequest (deflate + base64)
    response_parser.rs-- Parse + verify SAML Response from external IDP
  cert.rs             -- X.509 self-signed cert generation (x509-cert + rsa)
  bindings.rs         -- HTTP-POST / HTTP-Redirect binding implementations
```

### Dependencies
- `quick-xml` 0.36 (XML parse/write), `x509-cert` 0.2 + `der` 0.7 (cert handling), `flate2` 1 (deflate for HTTP-Redirect), `base64` 0.22
- Evaluate `samael` crate — if it has dropped `openssl-sys` dep it could replace the hand-rolled XML-DSig layer. Otherwise build focused XML-DSig with existing `rsa` + `sha2`.

### IDP Endpoints (Zitadel issues SAML assertions)

| Endpoint | Purpose |
|----------|---------|
| `GET /saml/metadata` | IDP metadata XML |
| `GET/POST /saml/sso` | SSO (receive AuthnRequest → login → issue Response) |
| `GET/POST /saml/slo` | Single Logout |

**Flow:** SP sends AuthnRequest → Zitadel validates SP against `apps` table (lookup by `saml_entity_id`) → creates `saml_auth_requests` record → redirects to existing login UI (same pattern as OIDC `auth_request_id` in `steps.rs`) → after auth, builds signed SAML Response → POST to SP's ACS URL.

### SP Flow (Zitadel consumes external IDP assertions)

Extend `crates/zitadel-login/src/sso.rs`:
- `sso_start()`: add branch for `protocol == "saml"` → build AuthnRequest, redirect to external IDP
- New `POST /v1/auth/saml/callback` → parse SAMLResponse, verify signature, extract attributes, call existing `complete_federated_login()`
- SAML connection fields stored in provider `connection.extra` HashMap (no schema change needed): `entity_id`, `sso_url`, `slo_url`, `certificate`, `name_id_format`, `binding`, `sign_requests`

### DB Migration (`00007_saml.sql`)
- **`saml_certificates`** table: id, instance_id, usage (signing/encryption), role (idp/sp), certificate_pem, private_key_enc (SecretBox encrypted), key_id, nonce, not_before, not_after, active
- **`apps` table ALTER**: add `saml_metadata_url`, `saml_metadata_xml`, `saml_acs_url`, `saml_entity_id`, `saml_name_id_format`, `saml_sign_assertions`, `saml_sign_responses`
- **`saml_auth_requests`** table: id, instance_id, request_id, issuer, acs_url, relay_state, name_id_policy, user_id, done, created_at

### Config Addition (`crates/zitadel-config/src/saml.rs`)
```rust
pub struct SamlConfig {
    pub entity_id: String,               // defaults to server public_origin
    pub key_size: u32,                   // 2048
    pub certificate_validity_days: u32,  // 365
    pub assertion_lifetime_secs: u64,    // 300
    pub sign_responses: bool,            // true
    pub sign_assertions: bool,           // true
    pub default_name_id_format: String,  // persistent
}
```

### Certificate Management
- Generate self-signed X.509 certs using `x509-cert` + `rsa` (both already in workspace)
- Private keys encrypted via existing `SecretBox`, stored in `saml_certificates` table
- Auto-generate on first IDP metadata request (lazy init, same as OIDC `RuntimeKeyStore`)
- Rotation: new cert when approaching `not_after`, old certs remain in metadata for verification

---

## 2. SCIM 2.0 Server

### New Crate: `crates/zitadel-scim/`

```
src/
  lib.rs              -- ScimState, routes()
  types.rs            -- SCIM resource types (User, Group, Schema, etc.)
  filter.rs           -- RFC 7644 filter parser → SQL WHERE
  users.rs            -- /scim/v2/Users CRUD handlers
  groups.rs           -- /scim/v2/Groups CRUD handlers
  schemas.rs          -- /scim/v2/Schemas (static)
  resource_types.rs   -- /scim/v2/ResourceTypes (static)
  service_provider.rs -- /scim/v2/ServiceProviderConfig (static)
  bulk.rs             -- /scim/v2/Bulk batch handler
  mapping.rs          -- SCIM ↔ internal model mapping
  error.rs            -- SCIM error response format (RFC 7644 §3.12)
```

**No new external crates needed** — SCIM is REST/JSON, fits perfectly on Axum + serde.

### Endpoints (all under `/scim/v2/`)

| Method | Path | Description |
|--------|------|-------------|
| GET | /ServiceProviderConfig | Server capabilities |
| GET | /Schemas, /Schemas/{id} | SCIM schema definitions |
| GET | /ResourceTypes | User + Group |
| GET/POST | /Users | List (with filter/pagination) / Create |
| GET/PUT/PATCH/DELETE | /Users/{id} | CRUD |
| GET/POST | /Groups | List / Create |
| GET/PUT/PATCH/DELETE | /Groups/{id} | CRUD |
| POST | /Bulk | Batch operations |

### Schema Mapping (key fields)

| SCIM | Internal | Notes |
|------|----------|-------|
| `id` | `users.id` | Read-only UUID |
| `externalId` | `users.external_id` | **New column** |
| `userName` | `users.identifier` | Unique per org |
| `name.*` | `users.metadata` JSON | given_name, family_name |
| `emails[].value` | `users.metadata.emails` | JSON array |
| `active` | `users.state` | true→active, false→inactive |
| `groups` | memberships query | Read-only, derived |

### Auth: Dedicated SCIM Tokens
- New `scim_tokens` table (id, instance_id, org_id, name, token_hash, scopes, expires_at)
- Separate from PATs — org-scoped, no user identity, SCIM-specific
- Admin API endpoints: `POST/GET/DELETE /v1/scim-tokens`
- SCIM middleware: extract Bearer → hash → lookup in scim_tokens → inject org scope

### DB Migration (part of `00008_scim_ldap.sql`)
- `ALTER TABLE users ADD COLUMN external_id TEXT DEFAULT ''` + index
- `CREATE TABLE scim_tokens (...)` with token_hash index

---

## 3. LDAP Client (Federation)

### Location: `crates/zitadel-ldap/src/client/`

```
client/
  mod.rs            -- LdapClient provider implementation
  connection.rs     -- Connection pool, bind, search helpers
  mapping.rs        -- External LDAP attrs → internal user model
```

### Dependency: `ldap3` crate (pure Rust async LDAP client)

### Provider Integration
Uses existing provider model with `protocol: "ldap"`. Connection JSON:
```json
{
  "host": "ldap.corp.example.com",
  "port": 636,
  "tls_mode": "ldaps",
  "bind_dn": "cn=service,dc=corp,dc=example,dc=com",
  "bind_password": "encrypted-via-secretbox",
  "base_dn": "ou=users,dc=corp,dc=example,dc=com",
  "search_filter": "(&(objectClass=person)(sAMAccountName={identifier}))",
  "search_scope": "subtree"
}
```

### Login Flow
LDAP is not redirect-based. New endpoint:
```
POST /v1/auth/ldap/verify
Body: { "provider_id": "...", "username": "...", "password": "..." }
```
Flow: load provider → search-then-bind (find user DN, bind with user's password) → extract attributes → map via `mapping.claims` → call existing `find_or_create_identity()` → create session.

Login UI detects LDAP providers and shows username/password form instead of SSO redirect button.

---

## 4. LDAP Server

### Location: `crates/zitadel-ldap/src/server/`

```
server/
  mod.rs            -- TCP listener, TLS, connection accept loop
  handler.rs        -- Bind, Search, Compare dispatch
  dit.rs            -- Virtual DIT mapping (org/user/group → LDAP tree)
  filter.rs         -- LDAP filter → SQL WHERE translation
  schema.rs         -- LDAP objectClass/attributeType definitions
```

### Dependency: `ldap3_server` (pure Rust LDAP server codec), `tokio-rustls` (TLS)

### Architecture
- Separate TCP listener on its own port (default 3389, TLS on 6360)
- Spawned as background tokio task in `run_with_db()` in `crates/zitadel-server/src/lib.rs`
- Shares same `Db` pool and password hasher
- Virtual DIT — every query translates to SQL, no separate LDAP datastore

### DIT (Directory Information Tree)
```
dc=zitadel,dc=io                        (configurable base DN)
├── ou=orgs
│   ├── o={org_name},ou=orgs,...
│   │   ├── ou=users,...
│   │   │   └── uid={identifier},...    (inetOrgPerson)
│   │   └── ou=groups,...
│   │       └── cn={group_name},...     (groupOfNames)
```

### Operations
- **Bind**: Simple bind — parse DN to extract user, verify password via credentials table
- **Search**: Translate LDAP filter to SQL, map rows to LDAP entries, paginate via `max_page_size`
- **Compare**: For `userPassword` verify against credentials; other attrs compare directly

### Config Addition (`crates/zitadel-config/src/ldap.rs`)
```rust
pub struct LdapServerConfig {
    pub enabled: bool,          // false
    pub port: u16,              // 3389
    pub tls_port: u16,          // 6360
    pub base_dn: String,        // "dc=zitadel,dc=io"
    pub anonymous_read: bool,   // false
    pub max_page_size: u32,     // 1000
}
```

---

## 5. Frontend Changes

| Area | Change |
|------|--------|
| **SAML App Config** | New tab in app management for SAML SP apps (entity ID, ACS URL, metadata upload, NameID format, signing toggles) |
| **SAML Provider** | Extend ProviderCreateView/DetailView with SAML connection fields + metadata import button |
| **SCIM Overview** | New `/scim` route — server status, endpoint URLs, SCIM token CRUD |
| **LDAP Provider** | LDAP option in provider create — host, port, bind DN, base DN, search filter, TLS mode |
| **LDAP Overview** | New `/ldap` route — server status, port, base DN, DIT preview |
| **Navigation** | Add "Provisioning" section to sidebar (SCIM, LDAP) |

---

## 6. Migration Summary

**`migrations/{sqlite,postgres}/00007_saml.sql`**
- `saml_certificates` table
- `apps` table: add SAML columns
- `saml_auth_requests` table

**`migrations/{sqlite,postgres}/00008_scim_ldap.sql`**
- `users` table: add `external_id` column + index
- `scim_tokens` table

---

## 7. Implementation Phases

| Phase | Scope | Key Files Modified/Created |
|-------|-------|---------------------------|
| **1: SAML Foundation** | XML types, C14N, XML-DSig, cert generation, migrations, SamlConfig | New crate `zitadel-saml`, config/saml.rs, migrations |
| **2: SAML IDP** | Metadata, SSO, SLO endpoints, assertion builder, login flow integration | zitadel-saml/idp/*, zitadel-login/src/steps.rs, zitadel-server/src/lib.rs |
| **3: SAML SP** | AuthnRequest builder, response parser, sso.rs extension | zitadel-saml/sp/*, zitadel-login/src/sso.rs |
| **4: SCIM Server** | Types, CRUD handlers, filter parser, SCIM auth, token management | New crate `zitadel-scim`, zitadel-api (token endpoints), zitadel-server/src/lib.rs |
| **5: LDAP Client** | Provider integration, search-then-bind, login endpoint | New `zitadel-ldap/src/client/`, zitadel-login/src/sso.rs |
| **6: LDAP Server** | TCP listener, Bind/Search/Compare, virtual DIT, filter translation | New `zitadel-ldap/src/server/`, zitadel-server/src/lib.rs |
| **7: Frontend** | SAML app config, SCIM overview, LDAP views, provider form extensions | web/src/console/views/*, router.ts |

---

## 8. Verification

- **SAML IDP**: Use a test SP (e.g., `samltest.id` or a local SimpleSAMLphp) to perform SP-initiated SSO. Verify metadata endpoint returns valid XML. Verify signed assertion is accepted.
- **SAML SP**: Configure an external IDP (e.g., Keycloak) as a SAML provider and complete federated login. Verify user linking works.
- **SCIM**: Use `curl` or a SCIM compliance test suite to test User/Group CRUD, filtering, pagination. Test with Entra ID SCIM connector if available.
- **LDAP Client**: Configure an OpenLDAP or AD test server as an LDAP provider. Complete login via LDAP. Verify attribute mapping.
- **LDAP Server**: Use `ldapsearch` CLI to bind and search. Test with Apache Directory Studio. Verify TLS/STARTTLS works.
- **Integration**: `just quality` must pass (clippy, fmt, tests, typecheck).

---

## 9. Key Risks

| Risk | Mitigation |
|------|------------|
| XML canonicalization correctness | SAML needs only Exclusive C14N — test against W3C reference vectors |
| `samael` has `openssl-sys` dep | Fall back to hand-rolled XML-DSig (~300-500 LOC with quick-xml + rsa + sha2) |
| LDAP server performance (virtual DIT) | max_page_size limit, connection timeout, index-backed SQL queries |
| SCIM filter parser complexity | Start with eq/ne/co/sw + basic and/or; expand incrementally |
| LDAP client connection pooling | Lazy pool per provider, service bind reused, user bind always fresh |
