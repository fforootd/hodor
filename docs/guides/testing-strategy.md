# Testing Strategy

Zitadel organizes tests by the question they answer, not by whether they happen to use a browser, an HTTP client, or direct storage calls.

## Two Axes

We keep two separate axes:

- **Test family** describes what the test proves.
- **Execution tier** describes when and how often we run it.

Current execution tiers:

- `fast`
- `pr`
- `nightly`
- `release`
- `manual`

Current families:

- `conformance`
- `journeys`
- `contracts`
- `invariants`
- `subsystems`
- `resilience`
- `performance`
- `upgrade`

The same family can run in multiple tiers. For example, `journeys` can run as `pr` smoke coverage and as a fuller nightly lane.

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

`performance` and `upgrade` are part of the taxonomy immediately, but they stay documentation-only until the first concrete suites land.

## Command Map

```bash
just test
just test-web
just journeys
just journeys-smoke
just journeys-oidc
just contracts
just invariants
just subsystems
just conformance-oidc
just test-pr
just test-nightly
```

Temporary compatibility aliases remain for:

- `just test-ui`
- `just ui-smoke`
- `just test-acceptance-oidc`
- `just acceptance-oidc-op`
- `just acceptance-oidc-rp`
- `just oidc-conformance`
- `just test-e2e`
- `just e2e-smoke`
- `just oidc-conformance-rp`
