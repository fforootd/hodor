# ADR-027: FIPS Compliance — Opt-in Compile Target

**Status:** Proposed
**Date:** 2026-04-02

## Context
The goal is to provide a FIPS 140-2/3 compliant deployment option for enterprise and government environments, without compromising the core Developer Experience (DX) principles defined in `developer-experience.md` ("Zero-Config First Run", "Pure Single Binary"). 

A fully FIPS-compliant setup requires a NIST-validated cryptographic module. In Rust, this typically means using a library like `aws-lc-rs` (with its `fips` feature) or dynamically linking OpenSSL. These modules introduce heavy build toolchain requirements (C compilers, CMake, Go) and runtime characteristics (Power-On Self-Tests) that conflict with our zero-config, ultra-fast cold start goals. Furthermore, modern secure defaults like Argon2 are not FIPS-approved, necessitating a fallback to PBKDF2.

## Decision
FIPS compliance will be implemented as an **opt-in compile target** rather than the default runtime behavior.

1. **Compile-Time Boundary (`fips` feature flag):** The `zitadel-crypto` crate will introduce a `fips` feature flag. 
   - **Default Build:** Remains pure-Rust, requires no CGO, and uses modern defaults (e.g., Argon2 for password hashing).
   - **FIPS Build (`cargo build --features fips`):** Swaps the backend to the FIPS-validated `aws-lc-rs` module.
2. **FIPS Configuration Profile:** A dedicated configuration profile (e.g., `fixtures/zitadel.fips.toml`) will be provided to explicitly configure FIPS-approved algorithms (like PBKDF2) and strict TLS cipher suites.
3. **Strict Startup Validation:** To adhere to the "No Operational Surprises" principle, a FIPS-compiled binary will perform strict configuration validation on startup. If it detects non-FIPS compliant settings (e.g., `password_hash_algorithm = "argon2"`), it will fail fast and exit rather than silently degrading compliance.

## Consequences

- **Positive:** We maintain an exceptional DX for standard users—fast compiles, trivial cross-compilation, tiny binaries, and optimal security defaults (Argon2).
- **Positive:** Regulated organizations can achieve verifiable FIPS 140-3 compliance.
- **Negative:** The `zitadel-crypto` crate must maintain dual backends (pure Rust vs `aws-lc-rs`), increasing internal complexity.
- **Negative:** FIPS users are forced to use PBKDF2 instead of the superior Argon2 algorithm due to NIST approval constraints.
