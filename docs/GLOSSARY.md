# Glossary & Ontology

This document defines the core vocabulary and terminology mappings for the Zitadel R&D prototype. Because this system redefines standard IAM concepts to fit a unified schema-driven model, AI agents and human contributors **must** use these definitions instead of hallucinating standard OAuth/IAM terminology.

| Term | Meaning | Legacy / Standard Equivalent |
|---|---|---|
| **Entity** | An instance of a schema. The universal noun for all domain objects (human users, service accounts, apps, organizations). | Identity, Client, Project |
| **Schema** | A versioned JSON Schema that defines the shape, behavior, and display of an entity type. | Fixed database columns |
| **Type** | The machine identifier for a schema (e.g., `human_user`, `app`). Immutable. | Hardcoded class/model |
| **Alias** | The human-readable name for a type (e.g., "Users"). Defined via `x-display`. | - |
| **Group** | A navigation section that categorizes schema types (e.g., "identities", "applications"). Replaces the concept of "Projects". A Group containing apps, users, and grants IS a project. | Project |
| **Path** | An API route alias (e.g., "users" → `/v1/users`). Defined via `x-display`. | - |
| **App** | An OIDC/SAML Client. It is simply an entity with a specific schema (`app` or `app_saml`). | OIDC Client, OAuth App |
| **Org** | Organization. The top-level scope/context for filtering entities. | Tenant |

## Key Paradigms

1. **Apps are Identities**: Non-human identities (Service Accounts) and Applications (OIDC Clients) use exactly the same underpinnings as human users.
2. **If it doesn't have a schema, it doesn't exist**: All persistent domain objects must be entities defined by a JSON Schema.
3. **Relationships via FGA**: Relationships between entities (e.g., membership in a group, ownership of an org) are represented as graph edges in OpenFGA, not relational tables.
