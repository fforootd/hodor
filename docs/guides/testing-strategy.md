# Testing Strategy

Zitadel organizes tests by the question they answer, not by whether they happen to use a browser, an HTTP client, or direct storage calls.

## Two Axes

We keep two separate axes:

- **Test family** describes what the test proves.
- **Execution tier** describes when and how often we run it.

Current execution tiers:

- `fast`
- `pr`
- `release`
- `nightly`
- `manual`

Tier policy:

- `fast` is the zero-config local default and the cheap always-on CI tier.
- `pr` is the full stable wall on every pull request.
- `release` mirrors the same stable wall on every push to `main`.
- `nightly` runs slower, quarantined, or environment-gated coverage outside the required PR wall.
- `manual` is for operator-invoked certification, debugging, and environment-gated reruns.

Current families:

- `conformance`
- `journeys`
- `contracts`
- `invariants`
- `subsystems`
- `resilience`
- `performance`
- `upgrade`

The same family can run in multiple tiers. Stable journeys run in `pr` and `release`, while quarantined or slower journey coverage stays in `nightly` and `manual`.

See [Testing Matrix](testing-matrix.md) for the current suite inventory.

## Families

### Conformance

Conformance proves standards correctness against an external suite or interoperability target.

- Official OIDC Provider conformance
- Harness-driven validation with exported reports and artifacts

Current home:

- `conformance/oidc`

### Journeys

Journeys prove that a real customer flow works end to end.

- Admin opens the console and signs in
- Admin navigates identity management
- An app completes OIDC code + PKCE
- Zitadel acts as an RP against an upstream OIDC provider

Rules:

- Drive the asserted steps through the browser like a user would.
- API seeding is allowed only for preconditions that are not the behavior under test.
- Journeys are product behavior, not protocol conformance.
- Stable journey lanes exclude cases tagged `@quarantine`.
- Quarantined journeys need an explicit reason to remain outside the PR wall.

Current home:

- `browser-tests/journeys`

### Contracts

Contracts prove stable external behavior at the API or routing boundary.

- OpenAPI and discovery surfaces
- Authentication and PAT-only boundary behavior
- Root instance management contracts

Current home:

- crate-local `tests/contracts_*`

### Invariants

Invariants prove the safety properties that must never regress.

- Tenant isolation
- Authorization inheritance and boundaries
- Session and redirect semantics

Current home:

- crate-local `tests/invariants_*`

### Subsystems

Subsystems prove internal domain behavior in focused slices.

- Repository boundary enforcement
- Storage role behavior
- Provider or session subsystem semantics

Current home:

- crate-local `tests/subsystems_*`

### Resilience

Resilience proves recovery, idempotency, and degraded-mode behavior.

Current home:

- crate-local `tests/resilience_*`

### Performance And Upgrade

`performance` is tracked as a specialized daily/manual workflow outside the stable wall. `upgrade` stays documentation-only until the first concrete suite lands.

## Command Map

```bash
just test
just test-fast
just rust-static
just web-static
just test-web
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

## CI Shape

- `PR / Stable Wall` is fixed and always-on for every pull request.
- `Release / Stable Wall` mirrors the same lane set on pushes to `main`.
- Nightly/manual coverage is split across `nightly-families.yml`, `oidc-conformance-daily.yml`, `db-perf-daily.yml`, and `fuzz-daily.yml`.
- Deprecated compatibility aliases were removed. Docs should reference only canonical commands from the map above.
