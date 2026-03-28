# ADR-011: Security Testing Philosophy — OWASP-Grounded

**Status:** Accepted  
**Date:** 2026-03-27  
**Builds on:** [ADR-002](002-schema-driven-login.md) (Schema-Driven Login), [ADR-003](003-auth-methods-meta-schema.md) (Auth Methods)

---

## Context

Zitadel is an identity platform where authentication and authorization correctness is a security-critical invariant. Bugs in this domain (token validation bypass, IDOR, privilege escalation, information leakage) can have severe consequences.

We ground our testing strategy in five OWASP Cheat Sheets:

| OWASP Sheet | Test Domain |
|-------------|-------------|
| [Authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html) | Password hashing, credential storage, error uniformity |
| [Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html) | Session lifecycle, token expiry/revocation, session listing |
| [Cookie Theft Mitigation](https://cheatsheetseries.owasp.org/cheatsheets/Cookie_Theft_Mitigation_Cheat_Sheet.html) | HMAC signing, HttpOnly, SameSite, `__Host-` prefix |
| [Authorization Testing](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Testing_Automation_Cheat_Sheet.html) | Role × endpoint matrix, IDOR prevention, privilege escalation |
| [Secrets Management](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html#212-passwordless-authentication-and-token-security) | Token entropy, key rotation, PAT lifecycle |

## Decision

### Test Pyramid

Tests are organized in three tiers:

```
┌─────────────────────────┐
│     Fuzz (go native)    │  Attack-pattern fuzzing, no 5xx invariant
├─────────────────────────┤
│   Integration (httptest │  Full API pipeline: HTTP → middleware → DB
│   + SQLite)             │  Token resolution, AuthZ matrix, IDOR
├─────────────────────────┤
│   Unit (pure Go)        │  Hash functions, cookie sign/verify,
│                         │  token generation, password verification
└─────────────────────────┘
```

**Unit tests** are fast, pure functions with no I/O. They validate cryptographic primitives:
- `token_test.go` — token format, prefix dispatch, hash determinism, uniqueness
- `cookie_test.go` — HMAC sign/verify round-trip, tamper detection, key rotation
- `password_test.go` — argon2id hashing, unicode support, credential replacement

**Integration tests** exercise the full request pipeline via `httptest.Server` + SQLite:
- `authn_integration_test.go` — session lifecycle, Bearer tokens, error uniformity, header injection
- `authz_integration_test.go` — authorization matrix, privilege escalation, IDOR prevention

**Fuzz tests** use Go's native fuzzing with attack-pattern seed corpora:
- `fuzz_test.go` (api) — JSON body fuzzing, token resolution, cookie auth, header injection
- `fuzz_test.go` (session) — cookie verify/sign with arbitrary inputs
- `fuzz_test.go` (auth) — extractHash with crafted JSON payloads

### Naming Conventions

| Layer | Pattern | Example |
|-------|---------|---------|
| Unit | `Test{Function}_{Behavior}` | `TestHashToken_Deterministic` |
| Integration | `Test{Subject}_{Scenario}` | `TestNonAdmin_CannotCreateEntity` |
| Fuzz | `Fuzz{Target}` | `FuzzCookieVerify` |
| Matrix | `TestAuthorizationMatrix` | Table-driven with `(role × endpoint × status)` |

### testutil Pattern

The `internal/testutil.TestServer` harness provides:

1. **One server per test** — each test gets a fresh SQLite DB + httptest.Server
2. **WAL checkpoint on cleanup** — `PRAGMA wal_checkpoint(TRUNCATE)` prevents `t.TempDir()` failures from dangling WAL/SHM files
3. **Bearer-first auth** — all `*WithCookie` helpers use `Authorization: Bearer` internally (not HTTP cookies) since direct-insert tokens can't be HMAC-signed
4. **Direct DB helpers** — `CreateIdentity`, `CreateSession`, `CreatePAT`, `LoginAdmin` for test setup without HTTP round-trips
5. **Timestamp format** — always use `2006-01-02 15:04:05` (not RFC3339) for SQLite `datetime('now')` compatibility

### Authorization Matrix

Following the OWASP Authorization Testing Automation approach, `TestAuthorizationMatrix` is a **table-driven test** with rows of `(method, path, body, unauth_code, user_code, admin_code)`:

```go
cases := []testCase{
    {"POST", "/v1/schemas", body, 401, 403, 201},
    {"POST", "/v1/entities", body, 401, 403, 201},
    {"GET", "/v1/sessions", nil, 401, 403, 200},
    {"GET", "/v1/account/profile", nil, 401, 200, 200},
    // ...
}
```

**Invariant:** Unauthenticated requests always get `401`. Non-admin users always get `403` for admin endpoints. Admin requests must not be blocked by auth (`≠ 401/403`).

### Fuzz Strategy

| Parameter | Value |
|-----------|-------|
| **Framework** | Go native (`testing.F`) |
| **CI budget** | 10s per target (configurable) |
| **Invariant** | No 5xx status codes on any input |
| **Seed corpus** | SQLi payloads, path traversal, null bytes, oversized inputs, unicode |
| **Error handling** | Transport-level errors (invalid header bytes) are silently skipped |

### Error Uniformity

All `401 Unauthorized` responses MUST have an identical JSON shape:

```json
{"error": "...", "code": 401}
```

They must NOT leak:
- Whether an identifier exists (`handleLoginStart` returns 200 for both valid and invalid identifiers)
- Stack traces, SQL errors, file paths, or internal state
- Different error messages for expired vs. revoked vs. invalid tokens

### IDOR Prevention

Every self-service endpoint (`/v1/account/*`) is tested with cross-user access attempts:
- User A cannot see User B's profile
- User A cannot see User B's sessions
- `X-Identity-Id` header injection without valid auth returns `401`

### Cookie Security

| Property | Value |
|----------|-------|
| **Signing** | HMAC-SHA256, constant-time comparison |
| **Name (prod)** | `__Host-zitadel_session` (forces Secure, Path=/, no Domain) |
| **Name (dev)** | `__zitadel_session` |
| **Flags** | `HttpOnly`, `SameSite=Lax`, `Secure` (when not localhost) |
| **Unsigned** | **Rejected** — no backward compatibility for raw cookies |
| **Key rotation** | Verify against all keys in config; sign with first |

### CI Integration

In `.github/workflows/ci.yml`:

- **Test timeout:** 240s (increased from 120s to accommodate expanded test suite)
- **Race detection:** enabled on all tests (`-race`)
- **Fuzz budget:** 10s × 11 targets = ~110s of fuzzing per CI run
- **Fuzz targets:** login (2), API (5), session (3), auth (1)

### When to Add Tests

Use this decision tree when adding new code:

```
New endpoint?
  └─ Add row to TestAuthorizationMatrix
  └─ If self-service: add IDOR test

New token type?
  └─ Add resolve test in token_test.go
  └─ Add fuzz target in fuzz_test.go

New credential type?
  └─ Add hash/verify test in password_test.go
  └─ Add fuzz target for extraction

Changed cookie format?
  └─ Test sign/verify round-trip
  └─ Test unsigned rejection
  └─ Verify __Host- prefix behavior

Changed login flow?
  └─ Verify error uniformity (no information leakage)
  └─ Test both valid and invalid identifiers return same status
```

## Consequences

- **Every PR touching auth** must update the authorization matrix if endpoints change
- **Fuzz tests catch edge cases** that manual testing misses (e.g., null bytes in headers)
- **Error uniformity is enforced by tests** — information leakage regressions are caught immediately
- **SQLite timestamp format (`2006-01-02 15:04:05`)** must be used consistently in tests — RFC3339 format silently breaks `datetime('now')` comparisons (this was discovered during this implementation)
- **Cookie unsigned fallback is permanently removed** — old sessions will not authenticate after this change (acceptable for R&D prototype)
