/**
 * Shared design tokens for all Zitadel web components.
 *
 * These CSS custom properties are injected into each CE's Shadow DOM
 * via Vue's `<style>` block (CE mode inlines them automatically).
 *
 * Keeping them here avoids the 60+ lines of duplicate CSS that was
 * previously copy-pasted across LoginApp.ce.vue and CreateUserWizard.ce.vue.
 */

/** Light-mode token values. */
export const LIGHT_TOKENS = `
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
`

/** Dark-mode token overrides. */
export const DARK_TOKENS = `
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
`

/** Font stack used across all WCs. */
export const FONT_STACK = "'Inter', ui-sans-serif, system-ui, -apple-system, sans-serif"

/**
 * Complete host styles string for use in `.ce.vue` <style> blocks.
 * Import and paste into the style section, or use programmatically.
 */
export const HOST_STYLES = `
:host {
  display: block;
  font-family: ${FONT_STACK};
  ${LIGHT_TOKENS}
}

:host(.dark) {
  ${DARK_TOKENS}
}
`

/**
 * Build a CSSStyleSheet from the shared tokens.
 * Used with adoptedStyleSheets for style sharing across multiple WCs.
 */
export function createSharedStyleSheet(): CSSStyleSheet {
  const sheet = new CSSStyleSheet()
  sheet.replaceSync(HOST_STYLES)
  return sheet
}

/** Cached global sheet — create once, reuse across all WC instances. */
let _sharedSheet: CSSStyleSheet | null = null

/**
 * Returns a singleton CSSStyleSheet with the design tokens.
 * Multiple WCs on the same page share this same sheet object,
 * avoiding duplicate style parsing.
 */
export function getSharedStyleSheet(): CSSStyleSheet {
  if (!_sharedSheet) {
    _sharedSheet = createSharedStyleSheet()
  }
  return _sharedSheet
}
