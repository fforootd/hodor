# Testing Matrix

This matrix maps the current repo to the question-oriented taxonomy. Families are the primary classification. Tiers describe when each suite currently runs.

## Tier Summary

| Tier | Purpose | Current entry points |
|---|---|---|
| `fast` | Cheap always-on local and CI feedback | `cargo test --workspace`, `cargo fmt --check && cargo clippy --workspace -- -D warnings`, `npm run lint -w web && npm run typecheck -w web`, `npm test -w web` |
| `pr` | Required stable wall on every pull request | All fast + family suites + spanner-cert, `.github/workflows/ci-pr.yml` |
| `release` | Required stable wall on every push to `main` | Same as pr, `.github/workflows/ci-main.yml` |
| `nightly` | Slower or quarantined coverage | pr + quarantine + conformance, `.github/workflows/nightly-families.yml`, `.github/workflows/oidc-conformance-daily.yml`, `.github/workflows/db-perf-daily.yml`, `.github/workflows/fuzz-daily.yml` |
| `manual` | Operator-invoked reruns and certification | `workflow_dispatch` on the nightly/manual workflows and `./conformance/oidc/scripts/run-op.sh` |

## Core Resource Coverage

| Surface | `contracts` | `invariants` | `journeys` | `subsystems` |
|---|---|---|---|---|
| `users` | [crates/zitadel-server/tests/contracts_http_router.rs](../../crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](../../crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/users.spec.ts](../../browser-tests/journeys/admin/users.spec.ts) | covered indirectly by existing user use-case tests in [crates/zitadel-app/tests/use_case_tests.rs](../../crates/zitadel-app/tests/use_case_tests.rs) |
| `groups` | [crates/zitadel-server/tests/contracts_http_router.rs](../../crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](../../crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/groups.spec.ts](../../browser-tests/journeys/admin/groups.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](../../crates/zitadel-app/tests/use_case_tests.rs) |
| `projects` | [crates/zitadel-server/tests/contracts_http_router.rs](../../crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](../../crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/projects.spec.ts](../../browser-tests/journeys/admin/projects.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](../../crates/zitadel-app/tests/use_case_tests.rs) via named-resource behavior tests |
| `apps` | [crates/zitadel-server/tests/contracts_http_router.rs](../../crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](../../crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/applications.spec.ts](../../browser-tests/journeys/admin/applications.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](../../crates/zitadel-app/tests/use_case_tests.rs) via named-resource behavior tests |
| `instances` | [crates/zitadel-server/tests/contracts_management_root_instance.rs](../../crates/zitadel-server/tests/contracts_management_root_instance.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](../../crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/instances.spec.ts](../../browser-tests/journeys/admin/instances.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](../../crates/zitadel-app/tests/use_case_tests.rs) |
| `login` | [crates/zitadel-server/tests/contracts_http_router.rs](../../crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](../../crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/login/password-login.spec.ts](../../browser-tests/journeys/login/password-login.spec.ts) | not a subsystem target in this phase |

Out of scope for this phase: `memberships`, `orgs`, and federation-admin setup. Existing tests for those surfaces can remain, but they are not part of the CRUD/login expansion checklist.

Current UI gap: the default human-user detail cockpit does not yet expose an editable schema form after creation. The user journey therefore verifies create, navigation into the edit boundary, and delete. Full user-update coverage is still enforced by `contracts` and `invariants`.

The stable PR wall is always-on. It does not use path-based skipping for the owned test families.

The `prompt=login` browser case is tagged `@quarantine`. The repo still enforces `prompt=login` semantics at the server layer in [crates/zitadel-server/tests/contracts_http_router.rs](../../crates/zitadel-server/tests/contracts_http_router.rs), but the browser journey currently stalls after the forced re-auth prompt in the real `auth_request_id` UI flow.

| Family | Current suites | Owner area | Current tier |
|---|---|---|---|
| `journeys` | `browser-tests/journeys/login/password-login.spec.ts` | browser / password login | `pr`, `release` |
| `journeys` | `browser-tests/journeys/admin/*.spec.ts` | browser / admin flows | `pr`, `release` |
| `journeys` | stable cases in `browser-tests/journeys/login/oidc-code-pkce.spec.ts` | browser / OIDC login | `pr`, `release` |
| `journeys` | `browser-tests/journeys/login/oidc-rp.spec.ts` | browser / federation login | `pr`, `release` |
| `journeys` | `prompt=login forces fresh credentials even when a session exists @quarantine` in `browser-tests/journeys/login/oidc-code-pkce.spec.ts` | browser / OIDC re-auth | `nightly`, `manual` |
| `conformance` | `conformance/oidc` | OIDC protocol compliance | `nightly`, `manual` |
| `contracts` | `crates/zitadel-server/tests/contracts_http_router.rs` | server / router and discovery | `pr`, `release` |
| `contracts` | `crates/zitadel-server/tests/contracts_management_root_instance.rs` | server / root instance management | `pr`, `release` |
| `contracts` | `crates/zitadel-server/tests/contracts_spanner_http_router.rs` | server / auth, observability, and analytics routes on native Spanner | `pr`, `release` |
| `contracts` | `crates/zitadel-db/tests/contracts_spanner_schema_layout.rs` | storage / Spanner DDL guardrails for PKs, indexes, and non-interleaving | `pr`, `release` |
| `invariants` | `crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs` | server / tenant isolation | `pr`, `release` |
| `invariants` | `crates/zitadel-fga/tests/invariants_authorization_hierarchy.rs` | FGA / authorization inheritance, tuple lifecycle, and store scope isolation | `pr`, `release` |
| `subsystems` | `crates/zitadel-api/tests/subsystems_backend_boundary.rs` | API / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-login/tests/subsystems_backend_boundary.rs` | login / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-oidc/tests/subsystems_backend_boundary.rs` | OIDC / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-server/tests/subsystems_backend_boundary.rs` | server / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-storage/tests/subsystems_storage_postgres_roles.rs` | storage / role runtime | `pr`, `release` |
| `subsystems` | `crates/zitadel-storage/tests/subsystems_storage_spanner_emulator.rs` | storage / native Spanner runtime certification | `pr`, `release` |
| `resilience` | `crates/zitadel-storage/tests/resilience_storage_spanner_transient.rs` | storage / transient semantics and observability analytics on native Spanner | `pr`, `release` |
| `resilience` | `crates/zitadel-db/tests/resilience_spanner_effects_jobs.rs` | storage / durable effects and job lease semantics on native Spanner | `pr`, `release` |
| `performance` | `.github/workflows/db-perf-daily.yml` | storage / latency and throughput trends | `nightly`, `manual` |
| `upgrade` | reserved for future suites | migrations and compatibility | documentation only |

## Command Map

```bash
# Journey (browser) tests
npm test -w browser-tests                                       # all journeys
npm test -w browser-tests -- --project=journeys-admin           # admin journeys
npm test -w browser-tests -- --project=journeys-login           # login journeys
npm test -w browser-tests -- --project=journeys-login-oidc      # OIDC journeys
npm test -w browser-tests -- --grep @quarantine                 # quarantined journeys

# Family suites
cargo test -p zitadel-server --test contracts_http_router \
  --test contracts_management_root_instance \
  --test contracts_spanner_http_router                          # contracts
cargo test -p zitadel-server --test invariants_tenant_instance_isolation \
  && cargo test -p zitadel-fga --test invariants_authorization_hierarchy  # invariants
# subsystems: multiple cargo test -p ... commands per crate
cargo test -p zitadel-storage \
  --test resilience_storage_spanner_transient                   # resilience

# Spanner emulator certification: run contracts + invariants + subsystems + resilience in sequence

# OIDC conformance
./conformance/oidc/scripts/run-op.sh

# CI tier reproduction (no single command — run the individual suites above)
# test-pr / test-release: fast + all family suites + spanner-cert
# test-nightly: test-pr + quarantine journeys + conformance
```

## Notes

- `cargo test --workspace` is the fast local default. It is not the full PR wall.
- The spanner-cert lane (contracts + invariants + subsystems + resilience in sequence) is the canonical emulator-backed native Spanner lane. If the Spanner env vars are absent locally, the Spanner-backed tests skip cleanly.
- `journeys` is the family name. Browser-only versus API-assisted setup is a test-design detail, not a top-level taxonomy bucket.
- `conformance` is reserved for standards validation. Repo-authored Playwright OIDC coverage belongs to `journeys`, not `conformance`.
- `performance` stays outside the required PR/release wall even though the repo already runs a daily DB harness.
- `fuzz` is a specialized nightly/manual workflow, not a blocking family in the core wall.
- `upgrade` stays in the taxonomy immediately so future suites land in the right place without another rename.
