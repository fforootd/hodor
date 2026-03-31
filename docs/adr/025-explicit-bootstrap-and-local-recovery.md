# ADR-025: Explicit Bootstrap and Local Break-Glass Recovery

**Status**: Proposed  
**Date**: 2026-03-31  
**Builds on**: ADR-018 (Startup Lifecycle & Schema Migration Strategy), ADR-020 (Authorization Model)  
**Related**: [Developer Experience](../design/developer-experience.md), [Local Development](../guides/local-development.md)

## Context

Zitadel's current first-run story is optimized for local convenience:

- `zitadel start` can auto-migrate and auto-bootstrap
- a TTY prompt creates the first admin on an empty instance
- non-interactive startup falls back to a generated password

That works for zero-config development, but it is not a strong operator story for self-hosted deployments:

- bootstrap is implicit inside `start`
- the trust model is unclear for remote or containerized installs
- recovery of broken admin access has no explicit local break-glass path
- future cloud workflows (`zitadel cloud create`, cloud recovery) need a cleaner separation between local operator authority and remote control-plane authority

## Decision

### 1. Self-hosted bootstrap becomes an explicit CLI workflow

Add an explicit command:

```bash
zitadel bootstrap admin
```

This command is the recommended operator workflow for creating the first local admin on an unclaimed self-hosted instance.

Behavior:

- load config and resolve local storage paths
- run schema migrations
- seed built-in system resources
- fail if any users already exist
- create the first admin
- ensure instance-owner FGA access

### 2. Local break-glass recovery is a separate CLI workflow

Add a second command:

```bash
zitadel recover admin
```

This command is a local-only break-glass path for self-hosted operators who have lost normal access.

Behavior:

- perform schema version check only
- resolve a target admin by user ID or identifier
- reset the password and reactivate the user when found
- optionally create a new break-glass admin when `--create-if-missing` is explicitly set
- re-grant instance-owner FGA access idempotently

### 3. Trust boundaries stay separate

These flows intentionally use different trust models:

- `bootstrap` trusts local operator authority on a self-hosted instance
- `recover` trusts local operator authority for audited break-glass actions
- future `zitadel cloud *` commands will trust the cloud control plane and management-secret based workflows instead

The bootstrap and recovery commands in this ADR do not introduce any remote HTTP bootstrap or recovery endpoint.

### 4. The zero-config startup flow remains for DX compatibility

`zitadel start` keeps its current interactive bootstrap behavior for now. This preserves the local-first developer experience and avoids breaking existing convenience workflows.

However:

- it is treated as a DX convenience path
- it is not the recommended operator workflow for self-hosted installs

### 5. Setup UI and cloud workflows are deferred

This ADR does not add:

- a first-run browser onboarding UI
- remote bootstrap endpoints
- remote recovery endpoints
- `zitadel cloud create`
- cloud-side recovery automation

Those belong to a later control-plane workstream with a distinct security model.

## Consequences

- self-hosted operators get a clear, auditable bootstrap path
- local break-glass recovery exists without weakening the network trust boundary
- the current zero-config dev story remains intact
- future cloud provisioning can evolve independently from local bootstrap semantics
- FGA gains an idempotent "ensure instance owner" helper so recovery can safely grant ownership after initial bootstrap

## Follow-On Work

- define the cloud control-plane command taxonomy (`zitadel cloud *`)
- design a setup UI for claimed-but-unconfigured instances
- add clearer audit surfacing for local operator actions in the console
