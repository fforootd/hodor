<template>
  <div class="zitadel-create-user-ce" :class="{ dark: isDark }">
    <CreateUserWizard
      :open="true"
      :standalone="true"
      :schema-type="schemaType"
      :org-id="orgId"
      :api-base-url="apiBaseUrl"
      @created="onCreated"
      @close="onClose"
      @error="onError"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Custom Element wrapper for CreateUserWizard.
 *
 * This .ce.vue file is consumed by defineCustomElement().
 * Styles are inlined via <style> (Vue CE mode auto-injects them into shadow DOM).
 *
 * FIXED: Host element discovery now uses getCurrentInstance() + closest()
 * instead of document.querySelector() which broke with multiple instances
 * and nested Shadow DOMs.
 */
import CreateUserWizard from './CreateUserWizard.vue'
import { computed, watch, onMounted } from 'vue'
import { dispatchWCEvent, injectCustomCSS, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-create-user'

const props = withDefaults(defineProps<{
  schemaType?: string
  orgId?: string
  apiBaseUrl?: string
  darkMode?: string
  primaryColor?: string
  customCss?: string
}>(), {
  schemaType: 'human_user',
  orgId: '',
  apiBaseUrl: '',
  darkMode: '',
  primaryColor: '',
  customCss: '',
})

const isDark = computed(() => isDarkMode(props.darkMode))

// Inject custom CSS into shadow DOM
onMounted(() => {
  if (props.customCss) injectCustomCSS(props.customCss)
})
watch(() => props.customCss, (css) => {
  if (css) injectCustomCSS(css)
})

function onCreated(entityId: string) {
  dispatchWCEvent(TAG_NAME, 'user-created', { entityId })
}

function onClose() {
  dispatchWCEvent(TAG_NAME, 'wizard-closed')
}

function onError(error: string) {
  dispatchWCEvent(TAG_NAME, 'wizard-error', { error })
}
</script>

<style>
/* Base reset for shadow DOM */
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

.zitadel-create-user-ce {
  color: var(--color-foreground);
  background: var(--color-background);
}

.zitadel-create-user-ce.dark {
  color-scheme: dark;
}
</style>
