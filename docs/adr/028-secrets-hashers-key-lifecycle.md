# ADR-028: Configurable Secrets, Hashers & Key Lifecycle

**Status:** Proposed
**Date:** 2026-04-02
**Builds on:** [ADR-009](009-settings-engine-pipeline.md) (Hierarchical Settings), [ADR-027](027-fips-compliance.md) (FIPS Compliance)

## Context

Multiple cryptographic concerns are either hardcoded or disconnected in the current codebase:

- **Password hasher** params are hardcoded (`Swapper::production()` = Argon2id 64 MB / 3 iter / 4 threads). Operators cannot tune them without code changes.
- **Encryption config** (`EncryptionConfig`) is defined in config but never wired to `SecretBox`.
- **OIDC signing keys** are ephemeral (regenerated each restart), preventing multi-instance deployments and key rotation.
- **Token lifetimes** and **session duration** are hardcoded (3600s / 24h).
- **No unified secret generator** for OTP codes, magic links, verification codes, client secrets, etc.
- **FIPS support** (ADR-027) needs a configurable foundation to swap algorithms at startup.

Go Zitadel's `defaults.yaml` demonstrates the mature model: configurable hashers with legacy verifier support, per-purpose secret generators (length, charset, expiry), per-purpose encryption keys with rotation, and OIDC key lifecycle management. This ADR brings equivalent capability to the Rust prototype.

## Decision

### 1. Two-Tier Configuration Split

**TOML config (system-level, startup)** for infrastructure concerns that must be known before the database is reachable:

| Section | Controls |
|---|---|
| `[encryption]` | AES-256-GCM key ring (already defined, now wired) |
| `[password_hasher]` | Algorithm, Argon2 params, bcrypt cost |
| `[secret_hasher]` | API/machine secret hashing (lower cost) |
| `[oidc]` | Signing algorithm, key size, key lifetimes, token lifetimes |
| `[session]` | Max session age |
| `[generators.*]` | Instance-level default profiles for secret generation |

**Settings table (per-org, cascading per ADR-009)** for policy that legitimately varies per-org or per-app. Secret generator profiles can be overridden at org scope (e.g., longer OTP codes for a high-security org).

### 2. Secret Generator in `zitadel-crypto`

A `GeneratorProfile` struct defines length, charset, optional expiry, and optional dash formatting:

```rust
pub struct GeneratorProfile {
    pub length: usize,
    pub charset: CharsetKind,       // Digits, UpperDigits, Alphanumeric, Custom
    pub custom_chars: String,       // for device auth codes etc.
    pub expiry_secs: Option<u64>,
    pub dash_interval: Option<usize>,
}
```

Each well-known purpose (ClientSecret, EmailVerificationCode, OtpSms, etc.) has compiled-in defaults matching Go Zitadel's `DefaultInstance.SecretGenerators`. A `generate(profile) -> String` function produces cryptographically random strings from the resolved charset.

### 3. Configurable Password Hasher

`Swapper::from_config(cfg: &PasswordHasherConfig)` replaces the hardcoded `production()` / `dev()` factory methods. The config specifies algorithm and parameters:

```toml
[password_hasher]
algorithm = "argon2id"
memory_cost_kb = 65536
time_cost = 3
parallelism = 4
```

Under FIPS (ADR-027), `algorithm = "pbkdf2"` is required and startup validation rejects Argon2.

### 4. OIDC Token Lifetimes from Config

All hardcoded `3600` values in `op.rs` are replaced with `config.oidc.access_token_lifetime` and `config.oidc.id_token_lifetime`. Defaults align with Go Zitadel (12h).

### 5. Session Duration from Config

The hardcoded `MAX_AGE = 86400` (24h) in cookie.rs and `'+24 hours'` SQL literal in session.rs become `config.session.max_age_secs`.

### 6. Wire Encryption Config

The existing `EncryptionConfig` is connected: server startup constructs `SecretBox` from `config.encryption` and threads it through `ApiState` / `LoginState` for encrypting sensitive fields at rest.

### 7. OIDC Key Lifecycle (Future)

Persistent signing keys stored encrypted in a `signing_keys` table. Background rotation based on `private_key_lifetime`. Multiple keys coexist (old keys remain for verification until `public_key_lifetime` expires). This is deferred to a follow-up PR.

## Consequences

- **Positive:** Operators can tune all crypto parameters without code changes, via TOML or env vars.
- **Positive:** Secret generation is unified, consistent, and configurable per-org via the settings cascade.
- **Positive:** FIPS (ADR-027) can validate algorithm selection at startup against a well-typed config.
- **Positive:** Zero breaking change: all new config fields have defaults matching current hardcoded behavior.
- **Negative:** More config surface to document and validate.
- **Negative:** OIDC key persistence is deferred, so ephemeral keys remain for now.
