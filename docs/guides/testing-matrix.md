# Testing Matrix

This matrix maps the current repo to the question-oriented taxonomy. Families are the primary classification. Tiers describe when each suite currently runs.

| Family | Current suites | Owner area | Current tier |
|---|---|---|---|
| `journeys` | `browser-tests/journeys/admin/console-login.spec.ts` | browser / admin console | `pr` |
| `journeys` | `browser-tests/journeys/admin/identity-management.spec.ts` | browser / admin identities | `pr` |
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
