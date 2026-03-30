# ADR-016: Schema-Driven Uniqueness & Identifier Resolution

**Status**: Proposed  
**Date**: 2026-03-28  
**Builds on**: [ADR-002](002-schema-driven-login.md) (Schema-Driven Login), [ADR-005](005-unified-data-model.md) (Unified Data Model), [ADR-006](006-entity-naming-model.md) (Entity Naming)

## Context

The system has a fundamental mismatch between schema annotations and database constraints:

- `x-identifier: true` drives the **login UI** (which fields appear as login inputs) but has **no uniqueness enforcement**.
- The only uniqueness constraint is a hardcoded index: `UNIQUE(org_id, identifier)` on a single column.
- The login lookup (`WHERE identifier = ?`) **ignores org context** entirely, creating correctness bugs when two orgs have the same username.
- There is no way to express instance-level uniqueness (email must be globally unique) vs org-level uniqueness (username only needs to be unique within an org).

As Zitadel's schema-driven model matures, uniqueness must be declarative — defined in the schema, not hardcoded in SQL.

## Core Principle

> **Uniqueness is data integrity. Identifiers are login behavior. They are orthogonal.**

A field can be unique without being a login identifier (e.g., `employee_id`). A field can be a login identifier without being unique (e.g., shared family phone number). The schema declares both independently.

## Decision

### 1. New Annotation: `x-unique`

Per-field annotation declaring uniqueness scope:

```json
{
  "properties": {
    "email": {
      "type": "string",
      "format": "email",
      "x-identifier": true,
      "x-unique": "instance"
    },
    "username": {
      "type": "string",
      "x-identifier": true,
      "x-unique": "org"
    },
    "phone": {
      "type": "string",
      "x-identifier": true
    }
  }
}
```

| `x-unique` value | Meaning | Constraint |
|---|---|---|
| `"instance"` | Globally unique across all orgs and schema types | `UNIQUE(field_name, normalized_value)` |
| `"org"` | Unique within the entity's org (across all schema types) | `UNIQUE(org_id, field_name, normalized_value)` |
| absent / `false` | No uniqueness enforced | — |

**Default**: When `x-unique` is not specified, no uniqueness is enforced. The `x-identifier` annotation alone does not imply uniqueness.

### 2. Current Scope: User-Backed Uniqueness

The current implementation enforces uniqueness through the `users` table and `unique_fields.user_id`. Instance- and org-scoped values are therefore cross-user-type for identities that live in `users`, but this mechanism does not yet generalize to non-user resources such as apps or providers.

### 3. Normalization

All unique field values are **normalized to lowercase** before storage and lookup. This prevents `Alice@Example.com` and `alice@example.com` from being treated as different identifiers.

```go
normalizedValue := strings.ToLower(strings.TrimSpace(value))
```

### 4. Database: `unique_fields` Table

A normalized table that the schema engine populates on entity create/update:

```sql
CREATE TABLE IF NOT EXISTS unique_fields (
    scope_id         TEXT NOT NULL DEFAULT '',  -- org_id for org-scope, '' for instance-scope
    field_name       TEXT NOT NULL,             -- e.g., 'email', 'username'
    normalized_value TEXT NOT NULL,             -- lowercase, trimmed

    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    UNIQUE(scope_id, field_name, normalized_value)
);

CREATE INDEX idx_unique_fields_user   ON unique_fields(user_id);
CREATE INDEX idx_unique_fields_lookup ON unique_fields(normalized_value, field_name);
```

**Why not scope by schema_type?** Per decision §2, uniqueness crosses schema types. A `human_user` email and a `service_user` email compete for the same namespace.

### 5. Identifier Resolution for Login

The login flow resolves an identifier to an entity using a two-phase lookup:

```
1. Instance-scoped match: WHERE normalized_value = ? AND scope_id = ''
2. Org-scoped match:      WHERE normalized_value = ? AND scope_id = {org_context}
3. If no match → "account not found"
```

#### How Login Gets Org Context

The login page determines org context through multiple mechanisms (in priority order):

1. **Custom domain**: `login.acme.com` → resolves to org via domain registry
2. **Subdomain**: `acme.zitadel.com` → org slug "acme"
3. **URL parameter**: `/login?org=acme` or `/login?org_id=019...`
4. **Login flow context**: Passed as parameter in `POST /v1/login/flows`
5. **No context**: Only instance-scoped identifiers can match. Org-scoped identifiers require explicit context.

This aligns with the existing domain resolution concept.

#### What `x-identifier` Controls

`x-identifier: true` tells the login flow engine which schema fields can be typed into the login input:

```json
"email":    { "x-identifier": true }   → login accepts email values
"phone":    { "x-identifier": true }   → login accepts phone values
"username": { "x-identifier": true }   → login accepts username values
```

The flow engine uses this to:
- Build the identifier input step (label, placeholder, autocomplete hints)
- Know which fields to search in `unique_fields` during resolution
- Display appropriate UI (email keyboard on mobile, etc.)

Without `x-identifier`, a field is never used for login lookup — even if it has `x-unique`.

### 6. Entity Lifecycle

#### Create

1. Parse entity's schema for `x-unique` fields
2. Extract and normalize field values from entity data
3. Insert into `unique_fields` within the same transaction
4. If UNIQUE constraint violation → return `409 Conflict` with field-specific error:
   ```json
   {
     "error": "uniqueness_violation",
     "field": "email",
     "value": "alice@example.com",
     "scope": "instance"
   }
   ```

#### Update

1. Delete existing `unique_fields` rows for this entity
2. Re-insert with new values
3. Same constraint checking applies

#### Delete

Handled by `ON DELETE CASCADE` — removing an entity automatically frees its unique values.

### 7. Schema Evolution

When a schema is created or updated, the system validates uniqueness feasibility against existing data.

#### Safe Changes (no validation needed)

| Change | Why Safe |
|---|---|
| Remove `x-unique` from a field | Relaxing constraints never breaks |
| Change `"instance"` → `"org"` | Relaxing scope never breaks |
| Add `x-identifier` without `x-unique` | No data integrity impact |
| Remove `x-identifier` | Login behavior change, but no data issue |

#### Dangerous Changes (require validation)

| Change | Risk | System Behavior |
|---|---|---|
| Add `x-unique` to existing field | Existing duplicates | **Block** unless zero duplicates; show preview |
| Change `"org"` → `"instance"` | Cross-org duplicates | **Block** unless zero cross-org duplicates; show preview |

#### Schema Save Validation

Before saving a schema with new or tightened `x-unique` constraints:

```sql
-- Check for instance-level duplicates on field 'email' across all entities of any type:
SELECT normalized_value, COUNT(*) as cnt
FROM unique_fields
WHERE field_name = 'email'
GROUP BY normalized_value
HAVING cnt > 1;
```

If duplicates exist, the schema save is **rejected** with a detailed report:

```json
{
  "error": "schema_uniqueness_violation",
  "violations": [
    {
      "field": "email",
      "scope": "instance",
      "duplicates": [
        { "value": "alice@example.com", "user_ids": ["019a...", "019b..."], "orgs": ["org-1", "org-2"] }
      ]
    }
  ]
}
```

#### Schema Upgrade Preview (UI)

The Schema Management UI's upgrade preview should display uniqueness impact analysis:

```
⚠️ Uniqueness Impact

Field "email" — Adding x-unique: "instance"
  ⚠ 3 duplicate values found:
    - alice@example.com  (2 entities across 2 orgs)
    - bob@test.com       (2 entities in same org)

  → Resolve duplicates before upgrading.
```

### 8. Scope for Later

The following are explicitly deferred:

- **Nested property uniqueness** (e.g., `profile.employee_id`) — only top-level properties support `x-unique` for now
- **Composite uniqueness** (e.g., unique on `(first_name, last_name)` together) — not needed yet
- **Conditional uniqueness** (e.g., unique only when `state = 'active'`) — complexity not justified

## Updated `human_user` Schema

```json
{
  "properties": {
    "email": {
      "type": "string",
      "format": "email",
      "x-identifier": true,
      "x-unique": "instance",
      "x-verify": "email",
      "x-recover": "email"
    },
    "phone": {
      "type": "string",
      "x-identifier": true,
      "x-sensitive": true,
      "x-mfa": "sms"
    },
    "username": {
      "type": "string",
      "x-identifier": true,
      "x-unique": "org"
    }
  }
}
```

In this example:
- **email**: Login identifier, globally unique, verified, used for recovery
- **phone**: Login identifier, not unique (shared family phones), used for MFA
- **username**: Login identifier, unique within org (allows `alice` in both org-1 and org-2)

## Meta-Schema Update

Add `x-unique` to the meta-schema's allowed per-field annotations:

```json
{
  "x-unique": {
    "oneOf": [
      { "type": "string", "enum": ["instance", "org"] },
      { "type": "boolean", "const": false }
    ],
    "description": "Uniqueness scope. 'instance' = globally unique, 'org' = unique per org. Orthogonal to x-identifier."
  }
}
```

## Consequences

- **Schema-declarative**: Uniqueness is part of the schema, not hardcoded DDL
- **Flexible scoping**: Instance-wide email + org-scoped username in the same schema
- **Cross-type safety**: No two entities of any type can share a globally-unique value
- **Evolution-safe**: Schema upgrades are validated against existing data before applying
- **Normalized**: Case-insensitive by default, preventing subtle duplicate bugs
- **Login-decoupled**: `x-identifier` (login behavior) and `x-unique` (data integrity) are independent annotations

## What Was Added

| File | Purpose |
|---|---|
| `docs/adr/016-uniqueness-constraints.md` | This ADR |

## What Will Be Modified

| File | Change |
|---|---|
| `internal/schema/meta_schema.json` | Add `x-unique` to allowed per-field annotations |
| `internal/schema/schemas/human_user.json` | Add `x-unique` annotations to email, add username field |
| `internal/database/migrations/*/00004_unique_fields.sql` | New `unique_fields` table |
| `internal/api/api.go` | Entity create/update: enforce `x-unique` via `unique_fields` |
| `internal/login/login.go` | Identifier resolution via `unique_fields` lookup |
| `internal/login/flow.go` | Extract `x-unique` alongside `x-identifier` |

## Alternatives Considered

| Approach | Why Not |
|---|---|
| **Keep hardcoded `UNIQUE(org_id, identifier)`** | Can't express instance-level uniqueness; single identifier only |
| **Add more columns (`email`, `username`, `phone`)** | Rigid; every new identifier type requires DDL changes |
| **JSON path uniqueness via CHECK constraints** | SQLite/Postgres have different JSON path syntax; fragile |
| **Application-level uniqueness (no DB constraint)** | Race conditions; uniqueness must be enforced at the database level |
