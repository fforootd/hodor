# Testing Matrix

This matrix maps the current repo to the question-oriented taxonomy. Families are the primary classification. Tiers describe when each suite currently runs.

## Tier Summary

| Tier | Purpose | Current entry points |
|---|---|---|
| `fast` | Cheap always-on local and CI feedback | `just test`, `just rust-static`, `just web-static`, `just test-web` |
| `pr` | Required stable wall on every pull request | `just test-pr`, `.github/workflows/ci-pr.yml` |
| `release` | Required stable wall on every push to `main` | `just test-release`, `.github/workflows/ci-main.yml` |
| `nightly` | Slower or quarantined coverage | `just test-nightly`, `.github/workflows/nightly-families.yml`, `.github/workflows/oidc-conformance-daily.yml`, `.github/workflows/db-perf-daily.yml`, `.github/workflows/fuzz-daily.yml` |
| `manual` | Operator-invoked reruns and certification | `workflow_dispatch` on the nightly/manual workflows and `just conformance-oidc` |

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
| `invariants` | `crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs` | server / tenant isolation | `pr`, `release` |
| `invariants` | `crates/zitadel-fga/tests/invariants_authorization_hierarchy.rs` | FGA / authorization inheritance | `pr`, `release` |
| `subsystems` | `crates/zitadel-api/tests/subsystems_backend_boundary.rs` | API / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-login/tests/subsystems_backend_boundary.rs` | login / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-oidc/tests/subsystems_backend_boundary.rs` | OIDC / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-server/tests/subsystems_backend_boundary.rs` | server / repository boundary | `pr`, `release` |
| `subsystems` | `crates/zitadel-storage/tests/subsystems_storage_postgres_roles.rs` | storage / role runtime | `pr`, `release` |
| `resilience` | `crates/zitadel-storage/tests/resilience_storage_spanner_transient.rs` | storage / transient semantics | `nightly`, env-gated `manual` |
| `performance` | `.github/workflows/db-perf-daily.yml` and `just perf-db-*` | storage / latency and throughput trends | `nightly`, `manual` |
| `upgrade` | reserved for future suites | migrations and compatibility | documentation only |

## Command Map

```bash
just journeys
just journeys-admin
just journeys-login
just journeys-oidc
just journeys-quarantine
just contracts
just invariants
just subsystems
just resilience
just conformance-oidc
just test-pr
just test-release
just test-nightly
```

## Notes

- `just test` is the fast local default. It is not the full PR wall.
- `journeys` is the family name. Browser-only versus API-assisted setup is a test-design detail, not a top-level taxonomy bucket.
- `conformance` is reserved for standards validation. Repo-authored Playwright OIDC coverage belongs to `journeys`, not `conformance`.
- `performance` stays outside the required PR/release wall even though the repo already runs a daily DB harness.
- `fuzz` is a specialized nightly/manual workflow, not a blocking family in the core wall.
- `upgrade` stays in the taxonomy immediately so future suites land in the right place without another rename.
