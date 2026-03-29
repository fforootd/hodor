<template>
  <div class="zitadel-login-ce" :class="{ dark: isDark }">
    <LoginApp
      :api-base-url="apiBaseUrl"
      :redirect-uri="redirectUri"
      :state="oidcState"
      :layout-override="layout"
      :dark-mode-override="darkMode"
      :cover-image-override="coverImage"
      :primary-color-override="primaryColor"
      @login-complete="onComplete"
      @login-error="onError"
      @login-redirect="onRedirect"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Custom Element wrapper for LoginApp.
 *
 * This .ce.vue file is consumed by defineCustomElement().
 * Styles are inlined via <style> (Vue CE mode auto-injects them into shadow DOM).
 *
 * Usage:
 *   <zitadel-login
 *     api-base-url="https://auth.acme.com"
 *     redirect-uri="/callback"
 *     layout="split"
 *     dark-mode="auto"
 *     cover-image="https://acme.com/hero.jpg"
 *     primary-color="#ff6600"
 *   ></zitadel-login>
 *
 * Events:
 *   - 'login-complete'  — { detail: { session_id, redirect_uri } }
 *   - 'login-error'     — { detail: { code, message } }
 *   - 'login-redirect'  — { detail: { redirect_url } }
 *
 * ADR-019: Server-Driven Login UI + Web Components
 * ADR-020: Customizable Login Layouts
 */
import LoginApp from './LoginApp.vue'
import { computed, getCurrentInstance } from 'vue'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  redirectUri?: string
  oidcState?: string
  // Layout & branding overrides — these take precedence over server-provided values
  layout?: string       // "centered" | "split" | "muted" | "card_image" | "minimal"
  darkMode?: string     // "light" | "dark" | "auto"
  coverImage?: string   // URL for cover/hero image
  primaryColor?: string // Hex color for primary brand color
  customCss?: string    // Raw CSS injected into the shadow DOM for quick overrides
}>(), {
  apiBaseUrl: '',
  redirectUri: '',
  oidcState: '',
  layout: '',
  darkMode: '',
  coverImage: '',
  primaryColor: '',
  customCss: '',
})

const isDark = computed(() => props.darkMode === 'dark')

// Inject custom CSS into the shadow DOM when the attribute is set.
import { watch, onMounted } from 'vue'
let customStyleEl: HTMLStyleElement | null = null

function injectCustomCss(css: string) {
  const instance = getCurrentInstance()
  const root = (instance?.proxy?.$el as HTMLElement)?.getRootNode() as ShadowRoot | null
  if (!root || !('adoptedStyleSheets' in root)) return

  if (!customStyleEl) {
    customStyleEl = document.createElement('style')
    customStyleEl.setAttribute('data-custom-css', '')
    root.appendChild(customStyleEl)
  }
  customStyleEl.textContent = css
}

onMounted(() => {
  if (props.customCss) injectCustomCss(props.customCss)
})
watch(() => props.customCss, (css) => {
  if (css) injectCustomCss(css)
})

function getHostElement(): HTMLElement | null {
  const instance = getCurrentInstance()
  return (instance?.proxy?.$el as HTMLElement)?.closest('zitadel-login') || null
}

function onComplete(detail: { session_id: string; redirect_uri: string }) {
  const el = getHostElement()
  el?.dispatchEvent(new CustomEvent('login-complete', {
    detail,
    bubbles: true,
    composed: true,
  }))
}

function onError(detail: { code: string; message: string }) {
  const el = getHostElement()
  el?.dispatchEvent(new CustomEvent('login-error', {
    detail,
    bubbles: true,
    composed: true,
  }))
}

function onRedirect(detail: { redirect_url: string }) {
  const el = getHostElement()
  el?.dispatchEvent(new CustomEvent('login-redirect', {
    detail,
    bubbles: true,
    composed: true,
  }))
}
</script>

<style>
/* Base reset for shadow DOM — mirrors the design system tokens */
:host {
  display: block;
  font-family: 'Inter', ui-sans-serif, system-ui, -apple-system, sans-serif;
  --color-background: hsl(0 0% 100%);
  --color-foreground: hsl(240 10% 3.9%);
  --color-card: hsl(0 0% 100%);
  --color-card-foreground: hsl(240 10% 3.9%);
  --color-popover: hsl(0 0% 100%);
  --color-popover-foreground: hsl(240 10% 3.9%);
  --color-primary: hsl(240 5.9% 10%);
  --color-primary-foreground: hsl(0 0% 98%);
  --color-secondary: hsl(240 4.8% 95.9%);
  --color-secondary-foreground: hsl(240 5.9% 10%);
  --color-muted: hsl(240 4.8% 95.9%);
  --color-muted-foreground: hsl(240 3.8% 46.1%);
  --color-accent: hsl(240 4.8% 95.9%);
  --color-accent-foreground: hsl(240 5.9% 10%);
  --color-destructive: hsl(0 84.2% 60.2%);
  --color-destructive-foreground: hsl(0 0% 98%);
  --color-border: hsl(240 5.9% 90%);
  --color-input: hsl(240 5.9% 90%);
  --color-ring: hsl(240 5.9% 10%);
  --radius: 0.5rem;
}

/* Dark mode tokens for shadow DOM */
:host(.dark) {
  --color-background: hsl(240 10% 3.9%);
  --color-foreground: hsl(0 0% 98%);
  --color-card: hsl(240 10% 3.9%);
  --color-card-foreground: hsl(0 0% 98%);
  --color-primary: hsl(0 0% 98%);
  --color-primary-foreground: hsl(240 5.9% 10%);
  --color-secondary: hsl(240 3.7% 15.9%);
  --color-secondary-foreground: hsl(0 0% 98%);
  --color-muted: hsl(240 3.7% 15.9%);
  --color-muted-foreground: hsl(240 5% 64.9%);
  --color-accent: hsl(240 3.7% 15.9%);
  --color-accent-foreground: hsl(0 0% 98%);
  --color-destructive: hsl(0 62.8% 30.6%);
  --color-destructive-foreground: hsl(0 0% 98%);
  --color-border: hsl(240 3.7% 15.9%);
  --color-input: hsl(240 3.7% 15.9%);
  --color-ring: hsl(240 4.9% 83.9%);
}

.zitadel-login-ce {
  color: var(--color-foreground);
  background: var(--color-background);
}

.zitadel-login-ce.dark {
  color-scheme: dark;
}
</style>
