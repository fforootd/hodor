# ADR-019: Server-Driven Login UI + Web Components

**Status**: Accepted
**Date**: 2026-03-28
**Builds on**: ADR-002 (Schema-Driven Login), ADR-003 (Auth Methods), ADR-007 (Schema ↔ Engine)

## Context

The login UI is currently a Vue SPA (`web/src/login/LoginApp.vue`) that talks to the flow API (`/v1/login/flows`). While the flow API already provides a server-driven node tree (`UINode[]`), the rendering is tightly coupled to Vue + shadcn-vue + Tailwind CSS. Customers who want to embed the login experience into their own applications can't simply drop in a `<script>` tag.

### Problems

1. **No embeddability**: Customers cannot drop the login into their React/Angular/plain HTML apps
2. **OIDC/SAML endpoint ownership**: When customers use custom domains, identity provider endpoints must be reachable on their domain
3. **Registration is a separate flow**: No unified flow engine path for self-service registration

## Decision

### 1. The server is the single source of truth

Every login screen is a `FlowStep` containing:
- `nodes: UINode[]` — the complete UI tree
- `branding: BrandingConfig` — colors, fonts, logo, custom CSS
- `errors: FlowError[]` — global errors (e.g. "invalid password")
- `messages: FlowMessage[]` — info/warning/success banners
- `css: string` — custom CSS from `x-branding.custom_css`

The **client is a dumb renderer**. It maps `UINode.type` to Vue or DOM elements, collects form values, and POSTs them back. Zero business logic lives in the client.

### 2. `<zitadel-login>` uses Vue `defineCustomElement`

Rather than building a vanilla JS web component (which would duplicate the entire styling system), we use Vue's `defineCustomElement()` to wrap the existing `LoginApp.vue` as a standard Web Component:

- **Same rendering code**: The shadcn-vue components, Tailwind styles, and UINode renderer are reused
- **No drift**: The hosted login at `/login` and the embeddable `<zitadel-login>` share the same component
- **Proven pattern**: Already working in `create-user-wc.ts` + `CreateUserWizard.ce.vue`
- **Trade-off**: Ships ~80KB Vue runtime — acceptable for a login page, and avoids maintaining two styling systems

**Files**:
- `LoginApp.ce.vue` — CE wrapper that injects design tokens into Shadow DOM and forwards events
- `zitadel-login-wc.ts` — Entry point that calls `defineCustomElement()` and registers `<zitadel-login>`
- `LoginApp.vue` — The renderer (accepts `api-base-url`, `redirect-uri` props; emits events)

### 3. Self-registering, event-driven

```html
<script type="module" src="/assets/zitadel-login.js"></script>

<zitadel-login
  api-base-url="https://auth.acme.com"
  redirect-uri="/callback"
></zitadel-login>

<script>
  document.querySelector('zitadel-login')
    .addEventListener('login-complete', (e) => {
      console.log('Session:', e.detail.session_id);
      window.location.href = e.detail.redirect_uri;
    });
</script>
```

**Events** (native `CustomEvent`, `bubbles: true`, `composed: true`):
| Event | Detail |
|---|---|
| `login-complete` | `{ session_id, redirect_uri }` |
| `login-error` | `{ code, message }` |
| `login-redirect` | `{ redirect_url }` |

### 4. Registration is part of the flow engine

The flow engine gains a `StepRegister` step that generates registration nodes from schema field annotations. When `x-login.registration_allowed` is true, the identifier step includes a "Create account" link that transitions to registration. Schema fields are introspected to build the form dynamically.

### 5. I18n is client-controlled

The server returns English text as defaults in `branding.texts`. The `texts` map uses structured keys (e.g. `identifier_label`, `continue_button`, `register_heading`). Customers can override these via `x-branding.texts` in their schema.

### 6. OIDC/SAML on customer domains

When customers point their domain (e.g., `login.acme.com`) at Zitadel:
- The **Domain Resolver** maps the host to an org
- OIDC endpoints are served by the built-in OIDC provider — same binary, just routes
- For **embedded** (cross-origin) deployment, the server returns CORS headers
- The flow stores the OIDC `redirect_uri` and `state` — on completion, redirects to the stored URI

### 7. `rust-embed` for distribution

The built WC bundle lands in `web/dist/assets/` alongside other Vite outputs. The Rust binary serves `web/dist` directly via `rust-embed`. No additional serving infrastructure is needed.

## UINode Type Registry

| Type | Vue Component | Purpose |
|---|---|---|
| `heading` | `<h1>` | Step title |
| `description` | `<p>` | Step subtitle |
| `input` | `<Label> + <Input>` | Form field (supports value, disabled, errors, minlength, maxlength, pattern) |
| `submit` | `<Button type="submit">` | Primary action |
| `button` | `<Button variant="outline">` | Alternative action |
| `sso_button` | `<Button>` with provider icon | SSO redirect |
| `divider` | `<Separator>` with "or" | Visual separator |
| `avatar` | `<Avatar>` | User identity indicator |
| `link` | `<Button variant="link">` | Navigation (back, sign in) |
| `error` | `<Alert variant="destructive">` | Inline error message |
| `info` | `<Alert>` | Informational text |
| `icon` | `<div>` | Decorative icon |
| `spinner` | `<Spinner>` | Loading state |
| `hidden` | `<input type="hidden">` | Flow metadata |
| `group` | `<div>` with children | Grouping container |
| `registration_link` | `<Button variant="link">` | "Create account" CTA |

## Consequences

- **Embeddable**: Any web page can embed `<zitadel-login>` with a single `<script>` tag
- **Single rendering path**: One component (LoginApp.vue), one styling system (shadcn-vue)
- **Schema-driven registration**: Same schema annotations drive admin wizard and self-service registration
- **OIDC-aware flows**: Flow state carries authorization context for proper redirects
- **Branding via schema**: `x-branding` annotations control colors, fonts, logo, custom CSS
- **Headless option**: Customers who want zero dependencies can call the flow API directly and build their own renderer

## Alternatives Considered

| Approach | Why not |
|---|---|
| Vanilla JS web component | Clean and lightweight (~15KB), but duplicates the entire styling system. Maintaining two CSS codebases creates drift. |
| Lit | Additional dependency; marginal benefit over either vanilla or Vue CE |
| React-based WC | Same framework coupling problem, different framework |
| Server-rendered HTML (no JS) | Can't handle dynamic flows (passkey, SSO redirects) without full page reloads |
