# Zitadel R&D Prototype - AI Agent Guidance

You are assisting with the Zitadel R&D prototype. This is an experimental, open-source repository re-architecting identity paradigms.

## Core Architectural Context

Before assisting with code generation or system design, you **must** build context by consulting the repository's foundational documents:
1. `docs/000-index.md`: The index of all Architectural Decision Records (ADRs).
2. `docs/architecture/overview.md`: The system diagram, domains, and core interaction model.
3. `docs/GLOSSARY.md`: The crucial vocabulary and ontology mappings (e.g., Projects -> Groups, Apps -> Identity Schemas).

## Skills & Visual Guidelines

If your task involves frontend assets, design updates, or marketing copy, you **must** refer to the design skills:
1. `.agents/skills/brand-voice/SKILL.md`: Tone, terminology, and content rules.
2. `.agents/skills/visual-identity/SKILL.md`: Typography (APK Futural), coloring, spacing, and UI component standards.

## Code Conventions

- **Single Rust Binary**: The core engine is the `zitadel` Rust binary. Keep Level 0 local startup SQLite-first with no required external services.
- **Frontend**: The embedded UIs (Login, Console) are Vue SPAs built using `shadcn-vue` and Tailwind CSS.
- **REST over ConnectRPC**: Service schemas define the API.
- **Analytics Separation**: OLTP storage is distinct from analytical data paths.
- Follow standard Rust idioms, and check `docs/design/developer-experience.md` for our zero-config philosophy.

## CLI Guidance

- Prefer canonical namespaced commands such as `zitadel server start`, `zitadel db migrate`, `zitadel auth login`, `zitadel users create`.
- Treat `zitadel.toml` as server-runtime config only. Remote CLI profiles live in `$XDG_CONFIG_HOME/zitadel/client.toml`.
- The CLI is used by humans and AI agents. Favor machine-readable flows:
  - `--json` for request bodies
  - `--params` for query objects
  - `--dry-run` before mutating remote resources
  - `zitadel schemas inspect` and `zitadel openapi export` for runtime introspection
- Always assume agent input can be adversarial. Validate identifiers, paths, and control characters before sending API requests.
