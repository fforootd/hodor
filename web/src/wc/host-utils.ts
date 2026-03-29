/**
 * Shared utilities for Zitadel web components.
 *
 * These helpers replace the ad-hoc implementations scattered across
 * LoginApp.ce.vue and CreateUserWizard.ce.vue with battle-tested,
 * shadow-DOM-aware versions.
 */

import { getCurrentInstance } from 'vue'

// ─── Host Element Discovery ─────────────────────────────────

/**
 * Find the host custom element from inside a CE's shadow DOM.
 *
 * This works correctly even when:
 * - Multiple instances of the same WC exist on the page
 * - The WC is nested inside another Shadow Root
 *
 * Uses getCurrentInstance() to find the actual Vue proxy element,
 * then traverses up to the custom element boundary.
 *
 * @param tagName - The custom element tag (e.g., 'zitadel-login')
 */
export function getHostElement(tagName: string): HTMLElement | null {
  const instance = getCurrentInstance()
  const el = instance?.proxy?.$el as HTMLElement | undefined
  if (!el) return null

  // .closest() crosses shadow boundaries in modern browsers
  return el.closest(tagName) || null
}

// ─── Event Dispatching ──────────────────────────────────────

/**
 * Dispatch a native CustomEvent from the host custom element.
 *
 * Events are created with `bubbles: true` and `composed: true`
 * so they cross shadow DOM boundaries and can be caught by
 * addEventListener on the host element or any ancestor.
 *
 * @param tagName - The custom element tag (e.g., 'zitadel-login')
 * @param eventName - Event name (e.g., 'login-complete')
 * @param detail - Event detail payload
 */
export function dispatchWCEvent<T = unknown>(
  tagName: string,
  eventName: string,
  detail?: T,
): boolean {
  const el = getHostElement(tagName)
  if (!el) {
    console.warn(`[${tagName}] Could not find host element to dispatch "${eventName}"`)
    return false
  }

  return el.dispatchEvent(new CustomEvent(eventName, {
    detail,
    bubbles: true,
    composed: true,
  }))
}

// ─── Custom CSS Injection ───────────────────────────────────

const cssStyleElements = new WeakMap<ShadowRoot, HTMLStyleElement>()

/**
 * Inject or update custom CSS in the component's shadow DOM.
 *
 * Creates a <style data-custom-css> element on first call,
 * then updates its textContent on subsequent calls.
 * Uses WeakMap so each shadow root gets its own style element.
 */
export function injectCustomCSS(css: string): void {
  const instance = getCurrentInstance()
  const el = instance?.proxy?.$el as HTMLElement | undefined
  if (!el) return

  const root = el.getRootNode() as ShadowRoot
  if (!root || !('host' in root)) return

  let styleEl = cssStyleElements.get(root)
  if (!styleEl) {
    styleEl = document.createElement('style')
    styleEl.setAttribute('data-custom-css', '')
    root.appendChild(styleEl)
    cssStyleElements.set(root, styleEl)
  }

  styleEl.textContent = css
}

// ─── API Base Resolution ────────────────────────────────────

/**
 * Resolve the API base URL from:
 * 1. An explicit prop value (highest priority)
 * 2. window.__ZITADEL_BASE_PATH__ (injected by Go server)
 * 3. Empty string (same-origin, default)
 */
export function resolveApiBase(propValue?: string): string {
  if (propValue) return propValue
  return (window as any).__ZITADEL_BASE_PATH__ || ''
}

// ─── Dark Mode ──────────────────────────────────────────────

/**
 * Compute whether dark mode should be active based on
 * the dark-mode attribute value.
 *
 * @param mode - 'light' | 'dark' | 'auto'
 * @returns true if dark mode should be active
 */
export function isDarkMode(mode: string): boolean {
  if (mode === 'dark') return true
  if (mode === 'auto') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
  }
  return false
}

// ─── Credentials Mode ───────────────────────────────────────

/**
 * Determine the correct fetch credentials mode for cross-origin
 * web component embedding.
 *
 * When a WC is embedded on a different origin than the API,
 * cookies need `credentials: 'include'`. Otherwise, use 'same-origin'.
 */
export function credentialsMode(apiBase: string): RequestCredentials {
  if (!apiBase) return 'same-origin'
  try {
    const apiOrigin = new URL(apiBase, window.location.origin).origin
    return apiOrigin !== window.location.origin ? 'include' : 'same-origin'
  } catch {
    return 'same-origin'
  }
}
