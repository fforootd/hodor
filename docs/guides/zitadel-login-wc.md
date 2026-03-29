# `<zitadel-login>` Web Component

A self-contained, embeddable login UI powered by Zitadel's server-driven flow API.

## Quick Start

```html
<!-- 1. Load the component -->
<script type="module" src="https://auth.acme.com/assets/zitadel-login.js"></script>

<!-- 2. Drop it in -->
<zitadel-login
  api-base-url="https://auth.acme.com"
  redirect-uri="/dashboard"
></zitadel-login>
```

The component is a [Custom Element](https://developer.mozilla.org/en-US/docs/Web/API/Web_components/Using_custom_elements) with Shadow DOM encapsulation. It works in any framework (React, Angular, Svelte, plain HTML).

---

## Attributes

All attributes are optional. When omitted, the component uses server-provided defaults from `x-branding`.

| Attribute | Type | Default | Description |
|---|---|---|---|
| `api-base-url` | `string` | `""` (same origin) | Base URL of the Zitadel API. **Required for cross-origin embedding.** |
| `redirect-uri` | `string` | `""` | Where to redirect after successful login (OIDC `redirect_uri`). |
| `oidc-state` | `string` | `""` | OIDC `state` parameter for CSRF protection. |
| `layout` | `string` | Server default | Layout preset. Overrides `x-branding.layout`. |
| `dark-mode` | `string` | Server default | Color scheme. Overrides `x-branding.dark_mode`. |
| `cover-image` | `string` | Server default | Cover image URL for `split` / `card_image` layouts. |
| `primary-color` | `string` | Server default | Primary brand color (hex). Overrides `x-branding.colors.primary`. |

### Layout Presets

| Value | Pattern | Description |
|---|---|---|
| `centered` | login-01 | Card centered on gradient background |
| `split` | login-02 | Two-column: form left, cover image right |
| `muted` | login-03 | Muted background with brand logo above card |
| `card_image` | login-04 | Wide card with embedded image alongside form |
| `minimal` | login-05 | Clean, minimal background |

### Dark Mode

| Value | Behavior |
|---|---|
| `light` | Light theme (default) |
| `dark` | Dark theme — dark tokens injected into Shadow DOM |
| `auto` | Follows `prefers-color-scheme` media query |

---

## Events

The component emits native [CustomEvents](https://developer.mozilla.org/en-US/docs/Web/API/CustomEvent) that bubble through Shadow DOM (`composed: true`).

| Event | `event.detail` | When |
|---|---|---|
| `login-complete` | `{ session_id: string, redirect_uri: string }` | Login succeeded, session created |
| `login-error` | `{ code: string, message: string }` | Flow error (init failure, validation error, etc.) |
| `login-redirect` | `{ redirect_url: string }` | SSO redirect initiated |

```javascript
document.querySelector('zitadel-login')
  .addEventListener('login-complete', (e) => {
    console.log('Session:', e.detail.session_id)
    // Custom redirect logic instead of default behavior
  })
```

---

## CSS Customization

### Option 1: CSS Custom Properties (Recommended)

The component exposes CSS custom properties on `:host` that you can override from outside the Shadow DOM:

```css
zitadel-login {
  /* Color tokens */
  --color-primary: hsl(262 83% 58%);
  --color-primary-foreground: hsl(0 0% 100%);
  --color-background: hsl(0 0% 100%);
  --color-foreground: hsl(240 10% 3.9%);
  --color-card: hsl(0 0% 100%);
  --color-card-foreground: hsl(240 10% 3.9%);
  --color-muted: hsl(240 4.8% 95.9%);
  --color-muted-foreground: hsl(240 3.8% 46.1%);
  --color-accent: hsl(240 4.8% 95.9%);
  --color-accent-foreground: hsl(240 5.9% 10%);
  --color-destructive: hsl(0 84.2% 60.2%);
  --color-border: hsl(240 5.9% 90%);
  --color-input: hsl(240 5.9% 90%);
  --color-ring: hsl(240 5.9% 10%);

  /* Shape */
  --radius: 0.75rem;

  /* Typography */
  font-family: 'Outfit', sans-serif;
}
```

CSS custom properties pierce Shadow DOM, so this works without any special API.

### Option 2: `::part()` Selectors

Key elements expose CSS `part` attributes for external styling:

```css
zitadel-login::part(card) {
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.15);
  border: 2px solid hsl(262 83% 58%);
}

zitadel-login::part(submit-button) {
  background: linear-gradient(135deg, #6366f1, #8b5cf6);
  font-weight: 700;
}

zitadel-login::part(input) {
  border-radius: 999px;
}
```

Available parts: `card`, `card-header`, `card-content`, `submit-button`, `input`, `social-button`, `logo`, `footer`.

### Option 3: `custom-css` Attribute (Inline Injection)

For quick overrides without a stylesheet, inject CSS directly into the Shadow DOM:

```html
<zitadel-login
  api-base-url="https://auth.acme.com"
  custom-css=".login-shell { backdrop-filter: blur(20px); } button[type=submit] { letter-spacing: 0.05em; }"
></zitadel-login>
```

> [!WARNING]
> `custom-css` injects raw CSS into the Shadow DOM. This is powerful but bypasses encapsulation.
> For production, prefer CSS custom properties or `::part()`.

### Option 4: `x-branding.custom_css` (Server-Side)

The schema's `x-branding.custom_css` field injects CSS from the server. This is the recommended approach for multi-tenant customization where each org/app defines its own styles:

```json
{
  "x-branding": {
    "custom_css": ".login-shell { box-shadow: 0 25px 50px -12px rgba(0,0,0,0.15); }"
  }
}
```

### Cascading Priority

CSS sources are applied in this order (later wins):

1. **Default design tokens** (shadow DOM `:host` defaults)
2. **`x-branding.custom_css`** (server-provided, from schema)
3. **CSS custom properties** (host page stylesheet, pierces shadow DOM)
4. **`::part()` selectors** (host page stylesheet)
5. **`custom-css` attribute** (inline, highest specificity inside shadow)

---

## Cross-Origin Embedding (CORS)

When embedding `<zitadel-login>` on a different origin than the Zitadel API, you need to configure CORS.

### Server Configuration

Add your embedding origin to the Zitadel CORS allowlist. This can be done per-org or globally:

```yaml
# zitadel.yaml
cors:
  allowed_origins:
    - "https://app.acme.com"
    - "https://staging.acme.com"
  allowed_methods: ["GET", "POST", "OPTIONS"]
  allowed_headers: ["Content-Type", "Authorization", "X-Flow-ID"]
  allow_credentials: true
  max_age: 3600
```

### How It Works

1. The `<zitadel-login>` component on `https://app.acme.com` sets `api-base-url="https://auth.acme.com"`
2. Flow API requests go to `https://auth.acme.com/v1/login/flows`
3. The browser sends a CORS preflight (`OPTIONS`) request
4. The Zitadel CORS middleware responds with:
   ```
   Access-Control-Allow-Origin: https://app.acme.com
   Access-Control-Allow-Credentials: true
   Access-Control-Allow-Methods: POST, GET, OPTIONS
   ```
5. The browser allows the cross-origin request

### Cookie / Session Considerations

| Concern | Solution |
|---|---|
| **Session cookies** | `SameSite=None; Secure` when CORS is enabled (required for cross-origin cookies) |
| **Credentials** | The fetch client uses `credentials: 'include'` when `api-base-url` is set (cross-origin mode) |
| **HTTPS** | Cross-origin cookies require HTTPS on both origins |
| **Subdomains** | If both origins share a parent domain (e.g., `auth.acme.com` / `app.acme.com`), use `domain=.acme.com` on the cookie |

### Credential Mode

The API client automatically switches between `same-origin` and `include` based on whether `api-base-url` points to a different origin:

```typescript
// Same-origin (default)
credentials: 'same-origin'

// Cross-origin (when api-base-url is set and differs from window.location.origin)
credentials: 'include'
```

---

## Schema-Driven Configuration

Everything the component renders is driven by the identity schema's `x-branding` annotation. WC attributes are **overrides** — if not set, the server value wins.

```json
{
  "x-branding": {
    "layout": "split",
    "dark_mode": "auto",
    "cover_image": "https://acme.com/hero.jpg",
    "logo_url": "https://acme.com/logo.svg",
    "logo_dark": "https://acme.com/logo-white.svg",
    "colors": {
      "primary": "#6366f1",
      "background": "#0f172a"
    },
    "font_family": "Outfit, sans-serif",
    "font_url": "https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700",
    "social_position": "top",
    "border_radius": "lg",
    "terms_url": "https://acme.com/terms",
    "privacy_url": "https://acme.com/privacy",
    "consent": [
      {
        "id": "terms",
        "label": "I agree to the [Terms of Service](https://acme.com/terms) and [Privacy Policy](https://acme.com/privacy)",
        "required": true
      },
      {
        "id": "marketing",
        "label": "Send me product updates and tips",
        "required": false
      }
    ],
    "texts": {
      "identifier_label": "Work Email",
      "continue_button": "Continue to Acme",
      "register_heading": "Join Acme"
    },
    "hide_zitadel_branding": true,
    "custom_css": ".card { box-shadow: 0 25px 50px -12px rgba(0,0,0,0.15); }"
  }
}
```

### Branding Cascade

Branding is resolved in order (later overrides earlier):

```
Instance defaults → Schema x-branding → Org override → App override → WC attribute
```

---

## Framework Examples

### React

```jsx
function LoginPage() {
  return (
    <zitadel-login
      api-base-url="https://auth.acme.com"
      layout="split"
      dark-mode="auto"
      onLoginComplete={(e) => router.push(e.detail.redirect_uri)}
    />
  )
}
```

> **Note**: React ≥19 supports custom element events natively. For React 18, use a ref and `addEventListener`.

### Vue

```vue
<template>
  <zitadel-login
    api-base-url="https://auth.acme.com"
    layout="muted"
    @login-complete="onLogin"
  />
</template>
```

### Angular

```html
<zitadel-login
  api-base-url="https://auth.acme.com"
  layout="card_image"
  cover-image="/assets/hero.jpg"
  (login-complete)="onLogin($event)"
></zitadel-login>
```

### Svelte

```svelte
<zitadel-login
  api-base-url="https://auth.acme.com"
  layout="minimal"
  on:login-complete={handleLogin}
/>
```

### Plain HTML

```html
<script type="module" src="https://auth.acme.com/assets/zitadel-login.js"></script>

<zitadel-login
  api-base-url="https://auth.acme.com"
  layout="centered"
  dark-mode="auto"
  primary-color="#ff6600"
></zitadel-login>

<script>
  document.querySelector('zitadel-login')
    .addEventListener('login-complete', (e) => {
      window.location.href = e.detail.redirect_uri
    })
</script>
```
