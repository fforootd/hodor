# Secrets & Cryptographic Configuration Guide

> How Zitadel handles passwords, tokens, encryption keys, and secret generation.

## Overview

Zitadel uses several layers of cryptographic protection:

| Layer | What | Where configured |
|---|---|---|
| **Password hashing** | User passwords (Argon2id/bcrypt) | `[password_hasher]` in TOML |
| **Secret hashing** | API/machine secrets (lower cost) | `[secret_hasher]` in TOML |
| **Envelope encryption** | Sensitive DB fields (AES-256-GCM) | `[encryption]` in TOML |
| **Cookie signing** | Session cookies (HMAC-SHA256) | `[server] cookie_secrets` |
| **OIDC signing** | JWTs (RS256/ES256) | `[oidc]` in TOML |
| **Secret generation** | OTP codes, verification codes, client secrets | `[generators.*]` in TOML + settings cascade |

## Quick Reference

### Zero-Config (Dev)

Out of the box with no config, Zitadel:
- Uses fast Argon2id (4 MB, 1 iter) for password hashing
- Generates a random HMAC key for cookies on each startup
- Operates in plaintext mode (no envelope encryption)
- Generates ephemeral RSA-2048 signing keys (lost on restart)
- Sets 24h session/cookie max age

This is safe for local development but **not for production**.

### Production Checklist

```toml
# 1. Set stable cookie secrets (survives restarts, supports rotation)
[server]
cookie_secrets = ["your-production-secret-here"]

# 2. Enable envelope encryption for sensitive DB fields
[encryption]
active_key_id = "prod-2024"
[[encryption.keys]]
id = "prod-2024"
secret = "<64 hex characters>"   # Generate: openssl rand -hex 32

# 3. Password hashing (defaults are good for most deployments)
[password_hasher]
algorithm = "argon2id"
memory_cost_kb = 65536    # 64 MB — tune based on server RAM
time_cost = 3
parallelism = 4

# 4. Session lifetime
[session]
max_age_secs = 86400      # 24h — adjust as needed
```

## Password Hashing

### Algorithm Selection

| Algorithm | Default | Use Case | FIPS |
|---|---|---|---|
| `argon2id` | Yes | User passwords (memory-hard, GPU-resistant) | No |
| `bcrypt` | No | Legacy compat, API secrets | No |
| `pbkdf2` | No | FIPS-mandated environments (future) | Yes |

### Transparent Re-Hashing (Passwap)

Zitadel uses a "swapper" pattern: it verifies passwords against **any** supported algorithm, then transparently re-hashes to the configured preferred algorithm on successful login. This means:

- Migrating from bcrypt to argon2id requires zero downtime
- Changing argon2id parameters (e.g., increasing memory) triggers automatic re-hashing
- Seed data can use `$plain$password` format for convenience

### Tuning Argon2id

The OWASP recommendation (as of 2024) is:
- **memory_cost_kb**: 19456 (19 MB) minimum, 65536 (64 MB) recommended
- **time_cost**: 2 minimum, 3 recommended
- **parallelism**: 1 minimum, match CPU cores for best throughput

Our defaults (64 MB / 3 iter / 4 threads) follow OWASP recommendations. Reduce for resource-constrained environments:

```toml
[password_hasher]
memory_cost_kb = 19456    # 19 MB — OWASP minimum
time_cost = 2
parallelism = 1
```

## Envelope Encryption

AES-256-GCM encryption for sensitive fields stored in the database (IDP secrets, SMTP credentials, etc.).

### Key Rotation

Multiple keys can be configured. The `active_key_id` is used for new encryptions; old keys remain for decryption:

```toml
[encryption]
active_key_id = "key-2025"

[[encryption.keys]]
id = "key-2025"
secret = "<new 64 hex chars>"

[[encryption.keys]]
id = "key-2024"
secret = "<old 64 hex chars>"    # kept for decrypting old data
```

Generate a key: `openssl rand -hex 32`

### Plaintext Mode

When no encryption keys are configured, `SecretBox` operates in plaintext passthrough mode. This is the default for development. In production, always configure encryption keys.

## OIDC Token Configuration

```toml
[oidc]
signing_algorithm = "RS256"              # RS256 | ES256 | ES384
key_size = 2048                          # RSA bits (ignored for EC)
access_token_lifetime_secs = 43200       # 12h
id_token_lifetime_secs = 43200           # 12h
refresh_token_idle_secs = 2592000        # 720h (30 days)
refresh_token_max_secs = 7776000         # 2160h (90 days)
private_key_lifetime_secs = 21600        # 6h (key rotation interval)
public_key_lifetime_secs = 108000        # 30h (verification window)
```

## Secret Generators

Configurable code/secret generation for different purposes. Each generator has a default profile that can be overridden in TOML or per-org via the settings cascade.

### Default Profiles

| Purpose | Length | Charset | Expiry |
|---|---|---|---|
| `client_secret` | 64 | alphanumeric | none |
| `email_verification_code` | 6 | upper + digits | 1h |
| `phone_verification_code` | 6 | upper + digits | 1h |
| `otp_sms` | 8 | digits only | 5m |
| `otp_email` | 8 | digits only | 5m |
| `invite_code` | 6 | upper + digits | 72h |
| `passwordless_init_code` | 12 | alphanumeric | 1h |
| `domain_verification` | 32 | alphanumeric | none |
| `device_auth_user_code` | 8 | consonants (no ambiguous) | 5m |

### Overriding in TOML

```toml
[generators.otp_sms]
length = 6           # shorter OTP
expiry_secs = 180    # 3 minutes instead of 5

[generators.client_secret]
length = 128         # extra-long client secrets
```

### Charset Options

- `digits` — 0-9
- `upper_digits` — A-Z + 0-9
- `alphanumeric` — a-z + A-Z + 0-9
- `alphanumeric_symbols` — a-z + A-Z + 0-9 + !@#$%^&*
- `custom` — provide your own character set via `custom_chars`

## Environment Variables

All settings support env var overrides:

```bash
# Nested (recommended)
ZITADEL_PASSWORD_HASHER__ALGORITHM=bcrypt
ZITADEL_OIDC__ACCESS_TOKEN_LIFETIME_SECS=3600
ZITADEL_SESSION__MAX_AGE_SECS=43200

# Flat (Go-compat)
ZITADEL_PASSWORD_HASHER_ALGORITHM=bcrypt
ZITADEL_OIDC_ACCESS_TOKEN_LIFETIME_SECS=3600
ZITADEL_SESSION_MAX_AGE_SECS=43200
```

## FIPS Compliance

See [ADR-027](../adr/027-fips-compliance.md). FIPS mode is an opt-in compile target (`--features fips`) that:

1. Swaps Argon2id for PBKDF2 (FIPS-approved)
2. Uses `aws-lc-rs` as the cryptographic backend
3. Validates configuration at startup (rejects non-FIPS algorithms)

A dedicated `fixtures/zitadel.fips.toml` profile will be provided.

## See Also

- [ADR-028: Configurable Secrets, Hashers & Key Lifecycle](../adr/028-secrets-hashers-key-lifecycle.md)
- [ADR-027: FIPS Compliance](../adr/027-fips-compliance.md)
- [ADR-011: Security Testing Philosophy](../adr/011-security-testing-philosophy.md)
- [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
