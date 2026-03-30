# ADR-022: Dedicated Resource Tables with Schema-Validated Metadata

**Status**: Proposed  
**Date**: 2026-03-29  
**Supersedes**: ADR-005 §2 ("No separate tables"), ADR-006 §1 (`x-storage: "entities"`)  
**Preserves**: ADR-005 §§3-6, ADR-006 §§2-8, ADR-007, ADR-008, ADR-009, ADR-016

## Context

ADR-005 established that "all persistent domain objects are entities" in a single `entities` table, differentiated by `schema_id`. After implementing this and consolidating providers into it (migration 00006), several problems surfaced:

1. **Every query requires JSONB extraction** — `json_extract(data, '$.protocol')` instead of `WHERE protocol = 'oidc'`. Slow, unindexable, unreadable.
2. **No FK integrity** — `linked_accounts.provider_id` can't reference a specific row-type in a generic table. No CASCADE. Orphans accumulate.
3. **The DB optimizer is blind** — no column statistics, no type-specific indexes, no per-table sharding.
4. **The "entity header" is the same everywhere** — `id, org_id, identifier, state, schema_id, metadata, created_at, updated_at`. Every dedicated table would share this. But that's normal table design, not a problem.
5. **Analytics replication requires JSONB parsing** — each row in `entities` must be introspected to determine its type. Dedicated tables ARE the type.

### What the entity model got right

The **schema registry** is excellent. Schemas drive UI generation, validation, engine bindings, uniqueness, and identifier resolution. None of this depends on storage being a single table. Schemas govern the `metadata` JSONB extension column on any table.

### What the marketplace actually does

The marketplace installs *instances of known types*, not new table structures:
- "Add GitLab login" → creates a row in `providers`
- "Add passwordless flow" → creates a row in `login_flows`
- "Add custom user schema" → changes `schema_id` on `users`, redefines what `metadata` fields exist

No customer ever creates a new *table*. They customize the *shape* of existing resource metadata.

## Core Principle

> **Each resource type has its own table with typed columns for the platform contract, plus a `metadata JSONB` column for customer-defined extensions validated by schemas.**

## Decision

### 1. Resource Tables Replace `entities`

Each resource type gets a dedicated table with:
- **Typed columns** for platform-defined fields (the contract)
- **`schema_id`** referencing the schema that validates the metadata
- **`metadata JSONB`** for customer-defined extension fields
- **Standard header**: `id, org_id, created_at, updated_at`

```sql
-- All resource tables share this header pattern:
CREATE TABLE <resource> (
    id          TEXT PRIMARY KEY,
    org_id      TEXT NOT NULL REFERENCES orgs(id),
    -- type-specific typed columns here --
    state       TEXT NOT NULL DEFAULT 'active',
    schema_id   TEXT DEFAULT '',
    metadata    JSONB DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 2. Resource Table Catalog

| Table | Schema Types | Key Typed Columns |
|---|---|---|
| `users` | `human_user`, `service_user`, `ai_agent` | `identifier`, `display_name`, `user_type`, `state` |
| `providers` | `provider` | `name`, `protocol`, `template`, `config`, `claim_overrides`, `enabled`, `display_order` |
| `apps` | `app`, `app_saml` | `name`, `app_type`, `client_id`, `redirect_uris`, `grant_types` |
| `actions` | `action` | `name`, `hook`, `action_type`, `trigger`, `config`, `priority`, `enabled` |
| `login_flows` | `login_flow` | `name`, `strategy`, `steps`, `config` |

### 3. Schema Role Shifts

| Aspect | Before (ADR-005) | After |
|---|---|---|
| Defines core fields | ✅ Schema defines everything | ❌ Typed columns define core |
| Defines extension fields | N/A (one pool) | ✅ Schema defines `metadata` shape |
| `x-unique` / `x-identifier` | Applied to `data` JSONB | Applied to `metadata` JSONB |
| `x-display` / `x-engine-*` | ✅ | ✅ Unchanged |
| `x-storage` | `"entities"` or `"dedicated"` | **Removed** — use new `x-table` |
| `x-table` | N/A | `"users"`, `"providers"`, etc. |
| `x-table-filter` | N/A | `{"user_type": "human"}` |

### 4. Users Table — Minimal + Schema-Driven

```sql
CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    org_id        TEXT NOT NULL REFERENCES orgs(id),
    identifier    TEXT NOT NULL,
    display_name  TEXT DEFAULT '',
    user_type     TEXT NOT NULL DEFAULT 'human',
    state         TEXT NOT NULL DEFAULT 'active',
    schema_id     TEXT DEFAULT '',
    metadata      JSONB DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, identifier)
);
```

- `identifier` — the primary login name. That's the only identity field as a typed column.
- Email, phone, employee_id — all in `metadata`, enforced via `unique_fields` when schema declares `x-unique`. In the current implementation this uniqueness index is user-backed (`unique_fields.user_id`), not yet generalized across every dedicated table.
- `user_type` — discriminator for `human`, `service`, `ai_agent`. Same table, different schemas.

### 5. FK Integrity

```sql
-- Real CASCADE — delete user, everything follows
user_credentials.user_id     REFERENCES users(id)       ON DELETE CASCADE
linked_accounts.user_id      REFERENCES users(id)       ON DELETE CASCADE
linked_accounts.provider_id  REFERENCES providers(id)   ON DELETE CASCADE
sessions.user_id             REFERENCES users(id)       ON DELETE CASCADE
tokens.user_id               REFERENCES users(id)       ON DELETE CASCADE
```

### 6. `unique_fields` — Cross-Table Uniqueness

The `unique_fields` table enforces `x-unique` constraints across all resource types. Since it references multiple tables, no single FK is possible:

```sql
CREATE TABLE unique_fields (
    scope_id         TEXT NOT NULL DEFAULT '',
    field_name       TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    resource_type    TEXT NOT NULL,    -- 'user', 'provider', 'app'
    resource_id      TEXT NOT NULL,    -- ID in the resource table
    UNIQUE(scope_id, field_name, normalized_value)
);
```

Cleanup: each resource's delete handler removes its `unique_fields` rows. This is 2 lines of code per delete path.

### 7. Groups — FGA Only

Groups are purely FGA tuples (`group:engineering#member@user:alice`). No table. No schema. Just authorization model relationships.

### 8. What Stays Unchanged

| Component | Status |
|---|---|
| Schema registry (`schemas` table) | ✅ Unchanged |
| Settings cascade (`settings` table) | ✅ Unchanged |
| Catalog system (templates, install) | ✅ Updated to target dedicated tables |
| FGA authorization | ✅ Unchanged |
| Event system (`events` table) | ✅ Unchanged |
| Sessions, tokens, OIDC tables | ✅ Unchanged (already dedicated) |
| `x-display`, `x-engine-*`, `x-auth-methods` | ✅ Unchanged |
| `unique_fields` for uniqueness | ✅ Minor: add `resource_type` column |
| `entity_indexes` for search | ✅ Rename to `resource_indexes` |
| Dynamic nav from schemas | ✅ Unchanged |
| Auto-generated API routes | ✅ Routes target per-table CRUD |

## Consequences

### Positive
- **Queryable**: typed columns, proper indexes, DB optimizer works
- **FK integrity**: CASCADE deletes, no orphans
- **Shardable**: per-table partitioning is natural
- **Readable**: `SELECT name, protocol FROM providers` vs `json_extract(data, '$.protocol')`
- **Analytics replication**: each table = one resource type, no JSONB introspection
- **Still extensible**: `metadata JSONB` + schema validation = customer-defined fields

### Negative
- **More tables**: 5 resource tables instead of 1 entity table
- **Header duplication**: 8 common columns repeated per table (standard, not a real cost)
- **Generic CRUD needs per-table awareness**: the router must know which table stores each schema type

### Migration
- POC: clean slate — drop `entities`, create resource tables, re-seed
- No backward compatibility required
