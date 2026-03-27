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

- **Pure Go**: The core engine is a pure Go binary. Avoid heavy external dependencies unless specified by ADRs.
- **Frontend**: The embedded UIs (Login, Console) are Vue SPAs built using `shadcn-vue` and Tailwind CSS.
- **REST over ConnectRPC**: Service schemas define the API.
- **Analytics Separation**: OLTP storage is distinct from analytical data paths.
- Follow standard Go idioms, and check `docs/design/developer-experience.md` for our zero-config philosophy.
