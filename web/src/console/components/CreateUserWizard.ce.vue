<template>
  <div class="zitadel-create-user-ce">
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
 */
import CreateUserWizard from './CreateUserWizard.vue'

const props = withDefaults(defineProps<{
  schemaType?: string
  orgId?: string
  apiBaseUrl?: string
}>(), {
  schemaType: 'human_user',
  orgId: '',
  apiBaseUrl: '',
})

const emit = defineEmits<{
  (e: 'user-created', detail: { entityId: string }): void
  (e: 'wizard-closed'): void
  (e: 'wizard-error', detail: { error: string }): void
}>()

function onCreated(entityId: string) {
  // Dispatch native CustomEvent for non-Vue consumers
  const el = document.querySelector('zitadel-create-user')
  el?.dispatchEvent(new CustomEvent('user-created', {
    detail: { entityId },
    bubbles: true,
    composed: true,
  }))
}

function onClose() {
  const el = document.querySelector('zitadel-create-user')
  el?.dispatchEvent(new CustomEvent('wizard-closed', {
    bubbles: true,
    composed: true,
  }))
}

function onError(error: string) {
  const el = document.querySelector('zitadel-create-user')
  el?.dispatchEvent(new CustomEvent('wizard-error', {
    detail: { error },
    bubbles: true,
    composed: true,
  }))
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

.zitadel-create-user-ce {
  color: var(--color-foreground);
  background: var(--color-background);
}
</style>
