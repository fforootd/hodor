# ADR-033: Customizable Login Layouts

> *Renumbered from ADR-020 to resolve duplicate numbering.*

**Status**: Accepted  
**Date**: 2026-03-29  
**Supersedes**: N/A  
**Extends**: ADR-002 (Schema-Driven Login), ADR-019 (Server-Driven Login UI + Web Components)

## Context

The current login UI renders a single, centered card layout (`login-01`). Customers need
layout variety—split-screen, muted background, card-with-image, minimal—to match their
branding. The shadcn-vue blocks (`login-01` through `login-05`, `signup-01` through `signup-05`)
demonstrate these patterns.

## Decision

### 1. Layout Selection via `x-branding.layout`

The `x-branding` schema annotation gains a new `layout` enum:

```
"centered" | "split" | "muted" | "card_image" | "minimal"
```

The backend emits the layout value in `FlowStep.branding`. The frontend uses `<component :is>` to
switch between 5 layout shell components. The form/node renderer stays identical across layouts.

### 2. Dark Mode

`x-branding.dark_mode` supports `"light" | "dark" | "auto"`. In `auto` mode the UI reads
`prefers-color-scheme`. Dark-mode tokens are injected into the shadow DOM `:host(.dark)`.

### 3. New UINode Types

| Type | Purpose |
|---|---|
| `social_group` | Container for SSO buttons with configurable position (above/below form) |
| `terms_footer` | Terms/Privacy links from `branding.terms_url` / `privacy_url` |
| `password_hint` | "Forgot password?" link, auto-generated when recovery is configured |
| `field_description` | Helper text rendered below inputs |
| `consent_checkbox` | Required/optional checkbox with markdown-link labels |

### 4. Consent Checkboxes

`x-branding.consent[]` defines an array of `{ id, label, required }` items. Labels support
markdown links (`[Terms](https://...)`). These are rendered as `consent_checkbox` nodes on registration.

### 5. Password Confirmation

Handled entirely client-side. Not a server node. The UI compares against schema constraints
(`minLength`, `pattern`) and shows a confirmation field during registration only.

### 6. Social Provider Position

`x-branding.social_position` (`"top" | "bottom"`) controls whether SSO buttons appear above
or below the identifier form. `"top"` wraps them in a `social_group` container node.

### 7. Branding Cascade

`ResolveBranding(schema, org, app)` merges branding from schema → org override → app override,
following the ADR-009 settings cascade.

### 8. Web Component Props

The `<zitadel-login>` custom element adds `layout`, `dark-mode`, `cover-image`, and
`primary-color` attributes that override server-provided values for embedding flexibility.

## Consequences

- **Positive**: Customers get visual variety without custom CSS. Layout selection is a single
  schema field change. Dark mode works out of the box.
- **Negative**: 5 layout components to maintain. However, they are thin shells (~50 LOC each)
  that delegate all logic to the shared form renderer.
- **Migration**: Existing schemas default to `layout: "centered"`, `dark_mode: "light"`. No
  breaking changes.

## References

- [shadcn-vue login blocks](https://www.shadcn-vue.com/blocks/login)
- [shadcn-vue signup blocks](https://www.shadcn-vue.com/blocks/signup)
- ADR-002: Schema-Driven Login
- ADR-019: Server-Driven Login UI + Web Components
- ADR-009: Settings Engine Pipeline
