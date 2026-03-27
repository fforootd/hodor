# Zitadel Architecture & Design Thinking

> This directory contains architecture decisions, design thinking, and vision documents for the Zitadel identity platform. These are living documents that evolve with the project — not customer documentation.

## Start Here

| Document | What it covers |
|---|---|
| [Architecture Overview](architecture/overview.md) | System diagram, three domains, request pipeline, deployment topologies |
| [Event Pipeline](architecture/event-pipeline.md) | Events table as queue, async consumers, backpressure, OTEL export |

## ADRs (Architecture Decision Records)

Numbered decisions with rationale. See [ADR Index](000-index.md) for the full list.

## Design

| Document | What it covers |
|---|---|
| [Developer Experience](design/developer-experience.md) | Zero-config, pure Go single binary, config cascade, testing philosophy |
| [Design Decisions](design/design-decisions.md) | Resolved decisions: OpenFGA, passkeys, SCIM scope, whitelabeling |
| [Design Patterns](design/design-patterns.md) | Unified identity, i18n rules, notification channels, schema registry |

## Vision

| Document | What it covers |
|---|---|
| [Market Positioning](vision/positioning.md) | Three pillars, landscape analysis, target audiences |
| [Pricing Philosophy](vision/pricing-philosophy.md) | Open core model, consumption-based thinking |

## For AI Agents

These docs are designed to be readable by both humans and AI coding assistants. Key navigation:

- **Understanding the codebase:** Start with [Architecture Overview](architecture/overview.md)
- **Understanding a design choice:** Check [ADR Index](000-index.md)
- **Understanding patterns:** Read [Design Patterns](design/design-patterns.md)
- **Understanding constraints:** Read [Developer Experience](design/developer-experience.md)
