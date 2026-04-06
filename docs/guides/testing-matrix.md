# Testing Matrix

This matrix maps the current repo to the question-oriented taxonomy. Families are the primary classification. Tiers describe when each suite currently runs.

## Core Resource Coverage

| Surface | `contracts` | `invariants` | `journeys` | `subsystems` |
|---|---|---|---|---|
| `users` | [crates/zitadel-server/tests/contracts_http_router.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/users.spec.ts](/Users/ffo/git/fforootd/hodor/browser-tests/journeys/admin/users.spec.ts) | covered indirectly by existing user use-case tests in [crates/zitadel-app/tests/use_case_tests.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-app/tests/use_case_tests.rs) |
| `groups` | [crates/zitadel-server/tests/contracts_http_router.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/groups.spec.ts](/Users/ffo/git/fforootd/hodor/browser-tests/journeys/admin/groups.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-app/tests/use_case_tests.rs) |
| `projects` | [crates/zitadel-server/tests/contracts_http_router.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/projects.spec.ts](/Users/ffo/git/fforootd/hodor/browser-tests/journeys/admin/projects.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-app/tests/use_case_tests.rs) via named-resource behavior tests |
| `apps` | [crates/zitadel-server/tests/contracts_http_router.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/applications.spec.ts](/Users/ffo/git/fforootd/hodor/browser-tests/journeys/admin/applications.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-app/tests/use_case_tests.rs) via named-resource behavior tests |
| `instances` | [crates/zitadel-server/tests/contracts_management_root_instance.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/contracts_management_root_instance.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/admin/instances.spec.ts](/Users/ffo/git/fforootd/hodor/browser-tests/journeys/admin/instances.spec.ts) | [crates/zitadel-app/tests/use_case_tests.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-app/tests/use_case_tests.rs) |
| `login` | [crates/zitadel-server/tests/contracts_http_router.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/contracts_http_router.rs) | [crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs) | [browser-tests/journeys/login/password-login.spec.ts](/Users/ffo/git/fforootd/hodor/browser-tests/journeys/login/password-login.spec.ts) | not a subsystem target in this phase |

Out of scope for this phase: `memberships`, `orgs`, and federation-admin setup. Existing tests for those surfaces can remain, but they are not part of the CRUD/login expansion checklist.

Current UI gap: the default human-user detail cockpit does not yet expose an editable schema form after creation. The user journey therefore verifies create, navigation into the edit boundary, and delete. Full user-update coverage is still enforced by `contracts` and `invariants`.

Current OIDC browser gap: [browser-tests/journeys/login/oidc-code-pkce.spec.ts](/Users/ffo/git/fforootd/hodor/browser-tests/journeys/login/oidc-code-pkce.spec.ts) still has one failing `prompt=login` path in the real `auth_request_id` UI flow. The repo still enforces `prompt=login` semantics at the server layer in [crates/zitadel-server/tests/contracts_http_router.rs](/Users/ffo/git/fforootd/hodor/crates/zitadel-server/tests/contracts_http_router.rs), but the browser journey is currently blocked by the login page not advancing from the password step after the forced re-auth prompt.

| Family | Current suites | Owner area | Current tier |
|---|---|---|---|
| `journeys` | `browser-tests/journeys/login/password-login.spec.ts` | browser / password login | `pr` |
| `journeys` | `browser-tests/journeys/admin/users.spec.ts` | browser / admin users | `pr` |
| `journeys` | `browser-tests/journeys/admin/groups.spec.ts` | browser / admin groups | `pr` |
| `journeys` | `browser-tests/journeys/admin/projects.spec.ts` | browser / admin projects | `pr` |
| `journeys` | `browser-tests/journeys/admin/applications.spec.ts` | browser / admin applications | `pr` |
| `journeys` | `browser-tests/journeys/admin/instances.spec.ts` | browser / admin instances | `pr` |
| `journeys` | `browser-tests/journeys/login/oidc-code-pkce.spec.ts` | browser / OIDC login | `pr` |
| `journeys` | `browser-tests/journeys/login/oidc-rp.spec.ts` | browser / federation login | `pr` |
| `conformance` | `conformance/oidc` | OIDC protocol compliance | `nightly`, `manual`, protocol-triggered `pr` |
| `contracts` | `crates/zitadel-server/tests/contracts_http_router.rs` | server / router and discovery | `pr` |
| `contracts` | `crates/zitadel-server/tests/contracts_management_root_instance.rs` | server / root instance management | `pr` |
| `invariants` | `crates/zitadel-server/tests/invariants_tenant_instance_isolation.rs` | server / tenant isolation | `pr` |
| `invariants` | `crates/zitadel-fga/tests/invariants_authorization_hierarchy.rs` | FGA / authorization inheritance | `pr` |
| `subsystems` | `crates/zitadel-api/tests/subsystems_backend_boundary.rs` | API / repository boundary | `pr` |
| `subsystems` | `crates/zitadel-login/tests/subsystems_backend_boundary.rs` | login / repository boundary | `pr` |
| `subsystems` | `crates/zitadel-oidc/tests/subsystems_backend_boundary.rs` | OIDC / repository boundary | `pr` |
| `subsystems` | `crates/zitadel-server/tests/subsystems_backend_boundary.rs` | server / repository boundary | `pr` |
| `subsystems` | `crates/zitadel-storage/tests/subsystems_storage_postgres_roles.rs` | storage / role runtime | `pr` |
| `resilience` | `crates/zitadel-storage/tests/resilience_storage_spanner_transient.rs` | storage / transient semantics | `nightly`, env-gated `manual` |
| `performance` | reserved for future suites | performance | documentation only |
| `upgrade` | reserved for future suites | migrations and compatibility | documentation only |

## Command Map

```bash
just journeys
just journeys-smoke
just journeys-oidc
just contracts
just invariants
just subsystems
just resilience
just conformance-oidc
just test-pr
just test-nightly
```

## Notes

- `journeys` is the family name. Browser-only versus API-assisted setup is a test-design detail, not a top-level taxonomy bucket.
- `conformance` is reserved for standards validation. Repo-authored Playwright OIDC coverage belongs to `journeys`, not `conformance`.
- `performance` and `upgrade` stay in the taxonomy immediately so future suites land in the right place without another rename.
