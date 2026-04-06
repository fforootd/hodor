# Hodor Todos

> Last cleaned: 2026-04-05. Completed items removed (preserved in git history).

## Protocol & Identity

- SAML SP, IDP (will be annoying in rust as well)
- OP Claim mappings and validation with schema and expr
- Client generation for SDKs (starting with TS/JS which we need for Vue anyways)
- Token settings per instance - per org - per project - per app

## Schema & Data Model

- Move all objects into the metaschema
- Describe interaction Schema FGA and EXPR
- Generate UI stubs from schemas?
- User uniqueness per org not per instance
- Make sure uniqueness is given for ids as well as username or other fields from schemas
- Secrets generator defaults — should store on user schemas
- toml really the best idea? Should we consider creating a schema for the config instead?

## Developer Experience

- IdentityListView should be UserListView
- Should we allow customers to store/sync schemas, settings and such things from git?
- Use Goose for DB migrations
- Test first time onboarding wizard — a) if it even works and b) security wise
- CLI to transform data from auth0, keycloak into zitadel (migration cases). The CLI should check the import files against the schemas and badge the uploads with error and progress tracking
- VSCode extension
- Remote MCP

## Observability

- Observability ingestion from SDK
- Usage telemetry

## UI / Frontend

- Timestamps should be local time not UTC in the UI
- Frontend packages and web components
- Login better error handling
- Single user list

## Cloud & Operations

- We need to document how managed service for the storages work

## Actions & Integrations

- Should we also store the flows? In the events... also we want to instrument them meaning the client provides trace context and so on
- Actions logic with webhooks
- Groups as marketplace addon for FGA?

## Cache & Performance

- Load test
- PG unlogged tables evaluation (deferred to post-perf-test — see ADR-017)
- Implement L4 HTTP caching: ETag for OIDC Discovery, JWKS, schema definitions
- Implement SQLite kv_cache + query_cache tables (ADR-017 L2 expansion)
- Generic Cache[K,V] interface implementation (SQLite + in-memory + Redis backends)
