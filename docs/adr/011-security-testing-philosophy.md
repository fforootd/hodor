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
│   Fuzz / property       │  Attack-pattern fuzzing, no 5xx invariant
├─────────────────────────┤
│ Integration (Axum +     │  Full API pipeline: HTTP → middleware → DB
│ SQLite)                 │  Token resolution, AuthZ matrix, IDOR
├─────────────────────────┤
│  Unit (crate-level)     │  Hash functions, cookie sign/verify,
│                         │  token generation, password verification
└─────────────────────────┘
```

**Unit tests** are fast, crate-local functions with no I/O. They validate cryptographic primitives:
- `crates/zitadel-authn/src/session.rs` — token hashing, lookup normalization, session scoping
- `crates/zitadel-authn/src/cookie.rs` — HMAC sign/verify round-trip, tamper detection, key rotation
- `crates/zitadel-authn/src/password.rs` — argon2id hashing, unicode support, credential replacement

**Integration tests** exercise the full request pipeline via an Axum test server or router harness + SQLite:
- `crates/zitadel-server/tests/router_contract.rs` — public-vs-protected routes, auth resolution, user CRUD, login state transitions, SSO callback redirects
- `crates/zitadel-api/src/middleware.rs` — uniform `401` responses and bearer-vs-cookie precedence at the API layer

**Property/fuzz tests** use Rust-native tooling with attack-pattern seed corpora:
- property tests around cookie sign/verify, password verify/rehash, and login flow transitions
- bounded fuzzing for token resolution, cookie auth, header parsing, and malformed JSON bodies

### Naming Conventions

| Layer | Pattern | Example |
|-------|---------|---------|
| Unit | `snake_case_behavior_name` | `token_hash_deterministic` |
| Integration | `snake_case_scenario_name` | `protected_routes_follow_the_current_actor_contract` |
| Property / fuzz | `snake_case_property_name` or `fuzz_*` | `cookie_round_trip_holds_for_generated_tokens` |
| Matrix | table-driven `cases` arrays in a route-contract test | `(actor × endpoint × status)` |

### Test Harness Pattern

The test harness should provide:

1. **One app per test** — each test gets a fresh SQLite DB plus a full Axum router assembled from the real API/login/OIDC stacks
2. **Shared fixtures** — `crates/zitadel-testkit` provides `TestDb`, `TestApp`, `AuthActor`, and setup helpers for users, sessions, PATs, and OIDC clients
3. **Bearer-first auth** — helper methods use `Authorization: Bearer` internally and add signed cookies only when the scenario requires cookie coverage
4. **Direct setup helpers** — fixtures create identities, sessions, PATs, and auth codes without unnecessary HTTP round-trips
5. **SQLite-native timestamps** — tests should use the same `YYYY-MM-DD HH:MM:SS` text format that SQLite emits for `CURRENT_TIMESTAMP`

### Authorization Matrix

Following the OWASP Authorization Testing Automation approach, `TestAuthorizationMatrix` is a **table-driven test** with rows of `(method, path, body, unauth_code, user_code, admin_code)`:

```rust
let cases = vec![
    ("GET", "/v1/users", 401, 200, 200),
    ("GET", "/v1/sessions", 401, 200, 200),
    ("GET", "/v1/auth/whoami", 401, 200, 200),
    ("GET", "/v1/fga/model", 401, 200, 200),
];
```

**Invariant:** Unauthenticated requests always get `401`, and authenticated requests must match the current runtime authorization contract without regressing into accidental `401` or `403` responses.

### Fuzz Strategy

| Parameter | Value |
|-----------|-------|
| **Framework** | Rust-native fuzz/property tooling |
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

In CI workflows:

- **Rust lint/test jobs** run on backend changes
- **Web static/unit and Playwright suites** run on browser-facing changes
- **Fuzz/property jobs** should run with bounded budgets in dedicated or release-candidate workflows

### When to Add Tests

Use this decision tree when adding new code:

```
New endpoint?
  └─ Add or update a row in the router contract matrix
  └─ If self-service: add the relevant cross-user / IDOR check

New token type?
  └─ Add resolution coverage in `crates/zitadel-server/tests/router_contract.rs`
  └─ Add property or fuzz coverage for malformed or adversarial inputs

New credential type?
  └─ Add hash/verify coverage in `crates/zitadel-authn/src/password.rs`
  └─ Add property coverage for extraction, migration, and failure cases

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
- **SQLite timestamp format (`YYYY-MM-DD HH:MM:SS`)** must be used consistently in tests — RFC3339 format silently breaks `datetime('now')` comparisons
- **Cookie unsigned fallback is permanently removed** — old sessions will not authenticate after this change (acceptable for R&D prototype)
