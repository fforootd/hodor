# Architecture & Design Thinking

> **Note:** This is an R&D prototype. APIs, schemas, and architectures are experimental and subject to breaking changes.
>
> These are living documents — architecture decisions, design thinking, and vision. Not customer documentation.
> Start with [Product Architecture](architecture/product-architecture.md) for the deployment model and scaling story. See [System Overview](architecture/overview.md) for the runtime diagram. Check [GLOSSARY.md](GLOSSARY.md) for terminology.

## Architecture Decision Records

ADRs are grouped by theme. For the full chronological list, see the [collapsed table](#chronological-adr-table) at the bottom.

### Core Data Model

| # | Title | Status |
|---|---|---|
| [002](adr/002-schema-driven-login.md) | Schema-Driven Identity, Auth, and Login Flows | Accepted |
| [003](adr/003-auth-methods-meta-schema.md) | Unified Auth Methods and Meta-Schema Validation | Accepted |
| [005](adr/005-unified-data-model.md) | Unified Data Model — Schemas, Orgs, Config Cascade | Accepted |
| [006](adr/006-entity-naming-model.md) | Entity Naming Model — Schema-as-Ontology | Proposed |
| [007](adr/007-schema-engine-interaction.md) | Schema ↔ Engine Interaction Model | Proposed |
| [008](adr/008-meta-schema-catalog.md) | Meta Schema as Entity Catalog | Proposed |
| [016](adr/016-uniqueness-constraints.md) | Schema-Driven Uniqueness & Identifier Resolution | Proposed |
| [022](adr/022-dedicated-resource-tables.md) | Dedicated Resource Tables + Metadata Extensions | Proposed |

### Identity & Auth Protocols

| # | Title | Status |
|---|---|---|
| [001](adr/001-rest-over-connectrpc.md) | REST+JSON over ConnectRPC | Accepted |
| [004](adr/004-apps-as-identities-oidc.md) | Apps as Identities — OIDC Provider | Proposed |
| [019](adr/019-server-driven-login-wc.md) | Server-Driven Login UI + Web Components | Accepted |
| [021](adr/021-login-flow-schema.md) | Login Flow Schema — Composable Bot Detection & Behavioral Telemetry | Accepted |
| [035](adr/035-provider-catalog-schema-binding.md) | Provider Catalog, Schema Binding, and Session Provenance | Accepted |

### Infrastructure & Operations

| # | Title | Status |
|---|---|---|
| [010](adr/010-three-tier-data.md) | Three-Tier Data Architecture | Accepted |
| [012](adr/012-path-based-deployment.md) | Path-Based Deployment Architecture | Accepted |
| [013](adr/013-id-generation-strategy.md) | ID Generation — Replace Sonyflake with UUIDv7 | Proposed |
| [014](adr/014-sdk-generation.md) | SDK Generation Strategy | Accepted |
| [017](adr/017-caching-tiers.md) | Process Cache Semantics | Proposed |
| [018](adr/018-startup-lifecycle.md) | Startup Lifecycle & Schema Migration | Proposed |

### Security & Compliance

| # | Title | Status |
|---|---|---|
| [011](adr/011-security-testing-philosophy.md) | Security Testing Philosophy — OWASP-Grounded | Accepted |
| [024](adr/024-risk-evaluation-policy-consumers.md) | Risk Evaluation and Policy Consumers | Accepted |
| [027](adr/027-fips-compliance.md) | FIPS Compliance — Opt-in Compile Target | Proposed |
| [028](adr/028-secrets-hashers-key-lifecycle.md) | Configurable Secrets, Hashers & Key Lifecycle | Proposed |

### Authorization & Multi-Tenancy

| # | Title | Status |
|---|---|---|
| [020](adr/020-authorization-model.md) | Authorization Model — Immutable Core + Custom Fragment | Accepted |
| [034](adr/034-multi-tenancy.md) | Multi-Tenancy via Instance Boundaries | Accepted |

### Cloud Architecture

| # | Title | Status |
|---|---|---|
| [026](adr/026-cloud-container-per-tenant.md) | Cloud Deployment — Control-Plane Routing and Regional Backends | Proposed |
| [029](adr/029-control-plane-auth-data-plane.md) | Control Plane, Auth Data Plane, and Bounded Eventual Consistency | Accepted |
| [030](adr/030-customer-portal-regional-projections-integrations.md) | Customer Portal, Regional Projections, and Control-Plane Integrations | Proposed |
| [031](adr/031-instance-hierarchy-spanner-geo-placement.md) | Instance Hierarchy with Geo-Partitioned Placement | Proposed |
| [036](adr/036-staff-access-support-grants.md) | Staff Access and Support Grants | Proposed |

### Product Features

| # | Title | Status |
|---|---|---|
| [009](adr/009-settings-engine-pipeline.md) | Hierarchical Settings & Engine Pipeline | Proposed |
| [015](adr/015-actions-catalog.md) | Actions, Templates & Catalog | Proposed |
| [023](adr/023-wide-events.md) | Wide Events as Internal Observability Primitive | Accepted |
| [025](adr/025-explicit-bootstrap-and-local-recovery.md) | Explicit Bootstrap and Local Break-Glass Recovery | Proposed |
| [032](adr/032-backend-layering-use-cases-hooks.md) | Backend Layering, Use Cases, and Hook Pipeline | Proposed |
| [033](adr/033-customizable-login-layouts.md) | Customizable Login Layouts | Accepted |

## Architecture

| Document | Summary |
|---|---|
| [Product Architecture](architecture/product-architecture.md) | One binary, deployment tiers (SQLite → Spanner), instance hierarchy, cloud-exclusive features |
| [System Overview](architecture/overview.md) | System diagram, three domains, request pipeline, deployment topologies |
| [Event Pipeline](architecture/event-pipeline.md) | Events table as queue, async consumers, backpressure, OTEL export |

## Design

| Document | Summary |
|---|---|
| [Developer Experience](design/developer-experience.md) | Zero-config, Rust-first single binary, config cascade, testing philosophy |
| [Design Decisions](design/design-decisions.md) | Compact resolved decisions with rationale (complements the full ADRs) |
| [Design Patterns](design/design-patterns.md) | Unified identity, i18n, notifications, schemas, UI architecture |
| [Storage Architecture](design/storage-architecture.md) | Canonical storage role model with derived defaults and split-topology overrides |
| [Storage Implementation Status](design/storage-implementation-status.md) | Reality check: what storage tiers are actually implemented vs target architecture |

## Guides

| Document | Summary |
|---|---|
| [Local Development](guides/local-development.md) | Canonical `just dev` flow, role-specific commands, seed packs, local SQLite lifecycle |
| [Testing Strategy](guides/testing-strategy.md) | Question-oriented test families, execution tiers, and the command surface that selects them |
| [Testing Matrix](guides/testing-matrix.md) | Current suite inventory mapped to families, owner areas, and execution tiers |
| [Bootstrap and Recovery](guides/bootstrap-recovery.md) | Current Rust bootstrap flow and recovery limitations |
| [OIDC Protocol Compliance](guides/oidc-conformance.md) | Official OIDC provider compliance coverage and its boundary from browser journeys |
| [Secrets & Crypto](guides/secrets-and-crypto.md) | Password hashing, encryption keys, token config, secret generators, FIPS |
| [Zitadel Login Web Component](guides/zitadel-login-wc.md) | Embed and customize the server-driven login web component |

## Vision

| Document | Summary |
|---|---|
| [Market Positioning](vision/positioning.md) | Three pillars, landscape analysis, target audiences |
| [Pricing Philosophy](vision/pricing-philosophy.md) | Open core model, consumption-based design thinking |

## Dependency Graph

```mermaid
graph TD
    A001[001 REST+JSON] --> A002[002 Schema-Driven Login]
    A002 --> A003[003 Auth Methods]
    A001 --> A004[004 Apps as Identities]
    A004 --> A005[005 Unified Data Model]
    A005 --> A006[006 Entity Naming]
    A005 --> A007[007 Schema ↔ Engine]
    A006 --> A007
    A007 --> A008[008 Meta Schema Catalog]
    A008 --> A009[009 Settings Pipeline]
    A005 --> A010[010 Three-Tier Data]
    A006 --> A010
    A002 --> A011[011 Security Testing]
    A003 --> A011
    A005 --> A012[012 Path-Based Deployment]
    A013[013 UUIDv7 IDs]
    A014[014 SDK Generation]
    A009 --> A015[015 Actions & Catalog]
    A004 --> A015
    A002 --> A016[016 Uniqueness]
    A005 --> A016
    A002 --> A019[019 Server-Driven Login WC]
    A012 --> A019
    A005 --> A020[020 AuthZ Model]
    A006 --> A020
    A020 --> A034[034 Multi-Tenancy]
    A005 --> A034
    A034 --> A026[026 Cloud Deployment]
    A034 --> A029[029 Control/Auth Planes]
    A026 --> A030[030 Customer Portal]
    A029 --> A030
    A030 --> A031[031 Instance Hierarchy]
    A034 --> A031
    A026 --> A031
    A029 --> A031
    A020 --> A036[036 Staff Access]
    A031 --> A036
    A034 --> A036
```

## Conventions

- **ADR location**: All ADRs live in `docs/adr/`
- **ADR filename**: `NNN-slug.md` (zero-padded 3-digit number)
- **ADR header**: `# ADR-NNN: Title` + Status / Date / Builds on / Supersedes
- **ADR status**: `Proposed` → `Accepted` → `Superseded`
- **Next ADR number**: 037

---

<details>
<summary>Chronological ADR Table</summary>

| # | Title | Status | Date |
|---|---|---|---|
| [001](adr/001-rest-over-connectrpc.md) | REST+JSON over ConnectRPC | Accepted | 2026-03-26 |
| [002](adr/002-schema-driven-login.md) | Schema-Driven Identity, Auth, and Login Flows | Accepted | 2026-03-26 |
| [003](adr/003-auth-methods-meta-schema.md) | Unified Auth Methods and Meta-Schema Validation | Accepted | 2026-03-26 |
| [004](adr/004-apps-as-identities-oidc.md) | Apps as Identities — OIDC Provider | Proposed | 2026-03-27 |
| [005](adr/005-unified-data-model.md) | Unified Data Model — Schemas, Orgs, Config Cascade | Accepted | 2026-03-27 |
| [006](adr/006-entity-naming-model.md) | Entity Naming Model — Schema-as-Ontology | Proposed | 2026-03-27 |
| [007](adr/007-schema-engine-interaction.md) | Schema ↔ Engine Interaction Model | Proposed | 2026-03-27 |
| [008](adr/008-meta-schema-catalog.md) | Meta Schema as Entity Catalog | Proposed | 2026-03-27 |
| [009](adr/009-settings-engine-pipeline.md) | Hierarchical Settings & Engine Pipeline | Proposed | 2026-03-27 |
| [010](adr/010-three-tier-data.md) | Three-Tier Data Architecture | Accepted | 2026-03-28 |
| [011](adr/011-security-testing-philosophy.md) | Security Testing Philosophy — OWASP-Grounded | Accepted | 2026-03-27 |
| [012](adr/012-path-based-deployment.md) | Path-Based Deployment Architecture | Accepted | 2026-03-28 |
| [013](adr/013-id-generation-strategy.md) | ID Generation — Replace Sonyflake with UUIDv7 | Proposed | 2026-03-28 |
| [014](adr/014-sdk-generation.md) | SDK Generation Strategy | Accepted | 2026-03-28 |
| [015](adr/015-actions-catalog.md) | Actions, Templates & Catalog | Proposed | 2026-03-28 |
| [016](adr/016-uniqueness-constraints.md) | Schema-Driven Uniqueness & Identifier Resolution | Proposed | 2026-03-28 |
| [017](adr/017-caching-tiers.md) | Process Cache Semantics | Proposed | 2026-03-28 |
| [018](adr/018-startup-lifecycle.md) | Startup Lifecycle & Schema Migration | Proposed | 2026-03-28 |
| [019](adr/019-server-driven-login-wc.md) | Server-Driven Login UI + Web Components | Accepted | 2026-03-28 |
| [020](adr/020-authorization-model.md) | Authorization Model — Immutable Core + Custom Fragment | Accepted | 2026-03-29 |
| [021](adr/021-login-flow-schema.md) | Login Flow Schema — Composable Bot Detection & Behavioral Telemetry | Accepted | 2026-03-29 |
| [022](adr/022-dedicated-resource-tables.md) | Dedicated Resource Tables + Metadata Extensions | Proposed | 2026-03-30 |
| [023](adr/023-wide-events.md) | Wide Events as Internal Observability Primitive | Accepted | 2026-03-29 |
| [024](adr/024-risk-evaluation-policy-consumers.md) | Risk Evaluation and Policy Consumers | Accepted | 2026-03-30 |
| [025](adr/025-explicit-bootstrap-and-local-recovery.md) | Explicit Bootstrap and Local Break-Glass Recovery | Proposed | 2026-03-31 |
| [026](adr/026-cloud-container-per-tenant.md) | Cloud Deployment — Control-Plane Routing and Regional Backends | Proposed | 2026-04-04 |
| [027](adr/027-fips-compliance.md) | FIPS Compliance — Opt-in Compile Target | Proposed | 2026-04-02 |
| [028](adr/028-secrets-hashers-key-lifecycle.md) | Configurable Secrets, Hashers & Key Lifecycle | Proposed | 2026-04-02 |
| [029](adr/029-control-plane-auth-data-plane.md) | Control Plane, Auth Data Plane, and Bounded Eventual Consistency | Accepted | 2026-04-04 |
| [030](adr/030-customer-portal-regional-projections-integrations.md) | Customer Portal, Regional Projections, and Control-Plane Integrations | Proposed | 2026-04-04 |
| [031](adr/031-instance-hierarchy-spanner-geo-placement.md) | Instance Hierarchy with Geo-Partitioned Placement | Proposed | 2026-04-04 |
| [032](adr/032-backend-layering-use-cases-hooks.md) | Backend Layering, Use Cases, and Hook Pipeline | Proposed | 2026-04-05 |
| [033](adr/033-customizable-login-layouts.md) | Customizable Login Layouts | Accepted | 2026-03-29 |
| [034](adr/034-multi-tenancy.md) | Multi-Tenancy via Instance Boundaries | Accepted | 2026-04-04 |
| [035](adr/035-provider-catalog-schema-binding.md) | Provider Catalog, Schema Binding, and Session Provenance | Accepted | 2026-03-30 |
| [036](adr/036-staff-access-support-grants.md) | Staff Access and Support Grants | Proposed | 2026-04-05 |

</details>
