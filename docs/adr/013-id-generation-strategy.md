# ADR-013: ID Generation — Replace Sonyflake with UUIDv7

- **Status**: Proposed
- **Date**: 2026-03-28
- **Authors**: Zitadel Architecture Team
- **Upstream Reference**: [zitadel/zitadel#8305](https://github.com/zitadel/zitadel/issues/8305)

## Context

Zitadel currently uses [Sonyflake](https://github.com/sony/sonyflake) (a Snowflake variant from Sony) to generate 64-bit time-ordered IDs. While this works, it introduces **operational complexity and failure modes** that are disproportionate for our architecture:

### Problems with Sonyflake

1. **Machine ID Requirement**: Sonyflake requires each instance to know its unique 16-bit machine ID. The default implementation resolves this from the lower 16 bits of the host's private IP address. This creates:
   - **Cloud failures**: Containers sharing IP ranges can collide.
   - **Test brittleness**: CI runners and local dev machines don't have stable IPs → we already lazy-init with `nil` machineID and hope for the best.
   - **K8s gotcha**: Pod IPs can recycle, violating the uniqueness assumption within the same 10ms window.

2. **64-bit Ceiling**: Sonyflake IDs are `int64`. While sufficient for a single Zitadel instance, they are not a universal standard. Every downstream integration (SCIM, OIDC `sub` claim, FGA tuples, analytics pipelines) must treat IDs as opaque strings anyway — we gain nothing from the compactness.

3. **Singleton Global State**: The `id` package uses `sync.Once` to initialize a global `*sonyflake.Sonyflake`. This makes testing harder (can't create isolated generators) and prevents multi-instance embedding.

4. **Dependency for ~40 Lines of Logic**: `github.com/sony/sonyflake` is imported for what amounts to a timestamp + counter + machine-id packing. We're pulling in a dependency for bit manipulation we could do ourselves (or let UUIDv7 solve natively).

5. **Not Human-Inspectable**: A Sonyflake ID like `612578054438387769` tells you nothing. A UUIDv7 like `019604f0-7c80-7a9e-8f3c-2a6b4d8e1f5a` embeds a visible timestamp and is instantly recognizable as a UUID.

### Why Not Make IDs a DB Problem?

We considered three strategies for who "owns" ID generation:

| Strategy | Pros | Cons |
|---|---|---|
| **DB-generated** (SERIAL, AUTOINCREMENT, DEFAULT gen_random_uuid()) | Zero app logic, always unique | Not available before INSERT, doesn't work with event-sourcing (need ID before command), SQLite AUTOINCREMENT has perf caveats, different syntax per dialect |
| **App-generated, opaque** (Sonyflake) | Available before INSERT, time-ordered | Machine ID coordination, global state, single-vendor format |
| **App-generated, standard** (UUIDv7) | Available before INSERT, time-ordered, RFC 9562 standard, no coordination | 128-bit (16 bytes vs 8), slightly larger indexes |

**Decision**: App-generated UUIDv7. The ID must exist _before_ we INSERT, because:
- Event-sourcing: the aggregate ID is minted at command time and referenced in events
- Bulk import: external systems send pre-assigned IDs
- Token generation: the token ID is embedded in the token hash before the DB row exists

Making IDs a DB problem would require `INSERT ... RETURNING id` everywhere, break event causality ordering, and introduce dialect-specific DDL differences for the most fundamental column in every table.

## Decision

**Replace Sonyflake with `google/uuid.NewV7()` (RFC 9562 UUIDv7).**

### ID Format

All primary key IDs will be:
- **Type**: `TEXT` (SQLite) / `UUID` (Postgres)
- **Format**: UUIDv7 — a 128-bit UUID with a 48-bit Unix millisecond timestamp prefix, ensuring chronological sort order
- **String representation**: Standard UUID format `xxxxxxxx-xxxx-7xxx-yxxx-xxxxxxxxxxxx`

### Package Interface

```go
package id

import "github.com/google/uuid"

// New generates a new UUIDv7.
func New() string {
    return uuid.Must(uuid.NewV7()).String()
}
```

No machine ID. No global state. No `sync.Once`. No coordination.
`uuid.NewV7()` uses `crypto/rand` internally — no configuration needed.

### Token ID Encoding

Tokens already use string-prefixed IDs (`zit_pat_`, `zit_ses_`, `zit_opq_`). UUIDv7 integrates naturally:

```
zit_pat_019604f0-7c80-7a9e-8f3c-2a6b4d8e1f5a
zit_ses_019604f0-7c81-7b2d-9e4a-3c5f6d7e8a9b
```

The prefix makes the token type instantly identifiable (ala Stripe `sk_live_`), and the UUID portion is still parseable, sortable, and globally unique.

### Schema Migration

```sql
-- SQLite: INTEGER PRIMARY KEY → TEXT PRIMARY KEY
-- Postgres: BIGINT PRIMARY KEY → UUID PRIMARY KEY (native type)
```

This is a **breaking migration** for existing data. The migration strategy:
1. Add new `uuid` column to each table
2. Backfill: `UPDATE entities SET uuid = lower(hex(randomblob(16)))` (SQLite approximation) or `gen_random_uuid()` (Postgres)
3. Swap foreign keys to reference new UUID columns
4. Drop old integer ID columns

For Hodor (pre-production), we can do a clean schema swap with no backfill.

## Comparison Matrix

| Property | Sonyflake | UUIDv7 | DB SERIAL |
|---|---|---|---|
| Coordination needed | Yes (machine ID) | **No** | No |
| Time-ordered | Yes (10ms) | **Yes (1ms)** | Sequentially |
| Available before INSERT | Yes | **Yes** | No |
| Standard format | No (Sony proprietary) | **Yes (RFC 9562)** | N/A |
| Size | 8 bytes | 16 bytes | 4-8 bytes |
| Human-readable | No | **Somewhat** | No |
| Dependencies | `sony/sonyflake` | `google/uuid` (≈stdlib) | None |
| Global state | Singleton | **None** | DB sequence |
| Cross-DB compatible | N/A | **Yes** (TEXT or native UUID) | Dialect-specific |
| Test-friendly | Requires mock | **Just call it** | Requires DB |
| SCIM/OIDC compatible | Needs string cast | **Native string** | Needs string cast |
| Collision probability | Machine-ID dependent | ≈0 (128-bit + crypto/rand) | 0 (DB enforced) |

## Consequences

### Positive
- **Zero configuration**: No machine ID resolution, no IP detection, no metadata service calls
- **Test-friendly**: No global state; every `id.New()` is independent, deterministic-safe
- **Standards-based**: RFC 9562, natively supported by Postgres, recognized by every downstream system
- **Removes dependency**: Drop `github.com/sony/sonyflake` from go.mod
- **Consistent with ADR-002**: The ADR already mentions "application-generated ULIDs for all IDs" — UUIDv7 is the RFC-standardized evolution of ULID
- **Index-friendly**: The timestamp prefix ensures B-tree insert locality (new rows go to the right edge), avoiding random write amplification that UUIDv4 causes

### Negative
- **Breaking change**: All foreign key relationships and API responses change from integer to string
- **Larger IDs**: 16 bytes vs 8 bytes per ID column (mitigated by: most columns are already TEXT in SQLite, and Postgres has a native 16-byte UUID type)
- **Existing consumers**: Any code parsing IDs as `int64` must be updated to `string`

### Risks
- **Migration complexity**: For production deployments with existing data, the INTEGER→UUID migration requires careful FK re-pointing. Mitigated by: Hodor is pre-production, so we do a clean schema swap.
- **Sort order**: UUIDv7 string sort ≠ chronological sort (lexicographic vs numeric). Mitigated by: UUIDv7's hex encoding preserves chronological ordering when compared as strings.

## Migration Plan

### Phase 1: Update `internal/id` package
- Replace Sonyflake with `google/uuid.NewV7()`
- Return `string` instead of `int64`
- Update all callers

### Phase 2: Update schema
- Change `INTEGER PRIMARY KEY` to `TEXT PRIMARY KEY` (SQLite)
- Add `UUID PRIMARY KEY` DDL for Postgres schema
- Update all INSERT/SELECT queries

### Phase 3: Update API responses
- JSON responses change from `"id": 612578054438387769` to `"id": "019604f0-7c80-7a9e-8f3c-2a6b4d8e1f5a"`
- Frontend `parseInt` calls → string handling
- Token resolution: already string-based, no change

### Phase 4: Clean up
- Remove `github.com/sony/sonyflake` from go.mod
- Remove testutil `time.Now().UnixNano()` ID hacks
- Update ADR-002 to reference UUIDv7 instead of ULID

## References

- [RFC 9562 — UUID Version 7](https://www.rfc-editor.org/rfc/rfc9562.html#name-uuid-version-7)
- [google/uuid UUIDv7 support](https://github.com/google/uuid)
- [zitadel/zitadel#8305](https://github.com/zitadel/zitadel/issues/8305) — Upstream issue requesting UUID-based IDs
- [Sonyflake machine ID problems](https://github.com/sony/sonyflake/issues/30) — Known issues with cloud deployments
