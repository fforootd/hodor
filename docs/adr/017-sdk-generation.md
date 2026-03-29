# ADR-017: SDK Generation Strategy

**Status**: Accepted  
**Date**: 2026-03-28  
**Authors**: @fforootd

## Context

The Zitadel console's frontend uses hand-written TypeScript types and API wrappers
(`web/src/api/resources.ts`, ~330 lines) that must be manually kept in sync with Go
handler changes. External JS/TS consumers have no supported client library.

We need:
1. A single source of truth for the API contract
2. Typed clients for internal (console) and external use
3. A generation pipeline that catches drift automatically

## Decision

### OpenAPI 3.1 as the Single Source of Truth

Go structs define all request/response types. An `OpenAPIRegistry` reflects these
types into JSON Schema at build time. The full spec is exportable via:

```
zitadel openapi-export > openapi.json
```

No server or database required — the spec is derived purely from Go type metadata.

### Code-First Over Spec-First

We chose code-first (annotating existing Go handlers) over spec-first (maintaining
YAML/JSON specs) because:
- Handlers already exist; rewriting them is unnecessary churn
- Go struct tags (`json:"..."`) are the most reliable source of field names
- The `OpenAPIRegistry` is additive — no handler refactoring needed

### @hey-api/openapi-ts for TypeScript SDK

The generated SDK lives at `packages/client-js/` (`@zitadel/client-js` on npm) and
uses `@hey-api/openapi-ts` + `@hey-api/client-fetch` because:
- Generates full service methods, not just types
- Tree-shakeable output
- Plugin ecosystem (TanStack Query, Zod — future options)
- The console and external users share the exact same SDK

### Monorepo Package Layout

```
packages/
├── openapi.json       # Shared contract (generated)
├── client-js/         # @zitadel/client-js (npm)
└── client-go/         # Go SDK (future)
```

### npm Naming Convention

| Package | Purpose |
|---|---|
| `@zitadel/client-js` | Core API client |
| `@zitadel/client-go` | Go SDK (future) |
| `@zitadel/react` | React auth bindings (exists) |
| `@zitadel/vue` | Vue auth bindings (exists) |
| `@zitadel/cli` | CLI tool (future) |
| `@zitadel/create-{f}` | Framework scaffolders (future) |

### Authentication in the SDK

**v1**: Bearer token injection, configurable base URL, interceptor hooks.  
**v2** (future): Full OIDC PKCE flow, session management, FGA client-side helpers.

## Consequences

- Every new API endpoint must add an `OpenAPIOperation` registration
- `make quality` validates the spec + SDK compile as part of CI
- Breaking API changes surface as TypeScript compile errors in both the console and SDK
- External consumers get first-class TypeScript support

## Alternatives Rejected

| Alternative | Why rejected |
|---|---|
| Swaggo (comment annotations) | Fragile, comment bloat, easy to forget |
| Huma (framework) | Would require rewriting all ~50 handlers |
| openapi-typescript (types-only) | Still requires hand-writing service methods |
| Protobuf/gRPC | Rejected in ADR-001 |
| Separate SDK repo | Premature — API is evolving fast |
