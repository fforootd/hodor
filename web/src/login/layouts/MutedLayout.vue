<template>
  <!-- Login-03 / Signup-03: Muted background, brand logo centered above card -->
  <div class="login-layout-muted" :class="{ 'is-preview': preview }" :style="bgVars">
    <link v-if="branding?.font_url" rel="stylesheet" :href="branding.font_url" />
    <div class="muted-wrapper">
      <!-- Centered brand logo -->
      <div class="muted-brand">
        <a href="#" class="muted-brand-link">
          <img v-if="logo" :src="logo" :alt="branding?.org_name || 'Logo'" class="muted-logo" />
          <span class="muted-org-name">{{ branding?.org_name || 'Zitadel' }}</span>
        </a>
      </div>
      <!-- Card content -->
      <div class="muted-card-wrap">
        <slot />
      </div>
      <!-- Footer -->
      <slot name="footer" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { FlowBranding } from '@/api/branding'

const props = defineProps<{
  branding: FlowBranding | null
  preview?: boolean
}>()

const isDark = computed(() => props.branding?.dark_mode === 'dark')
const logo = computed(() => {
  if (isDark.value && props.branding?.logo_dark) return props.branding.logo_dark
  return props.branding?.logo_url || ''
})

const bgVars = computed(() => ({
  fontFamily: props.branding?.font_family || 'Inter, system-ui, sans-serif',
  background: 'var(--color-muted, #f4f4f5)',
}))
</script>

<style scoped>
.login-layout-muted {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100svh;
  width: 100%;
  padding: 1.5rem;
}
.login-layout-muted.is-preview {
  min-height: 100%;
  padding: 1rem;
}
@media (min-width: 768px) {
  .login-layout-muted { padding: 2.5rem; }
}
.muted-wrapper {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.5rem;
  width: 100%;
  max-width: 28rem;
}
.muted-brand {
  display: flex;
  justify-content: center;
}
.muted-brand-link {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  text-decoration: none;
  color: inherit;
}
.muted-logo { height: 2rem; }
.muted-org-name { font-size: 1.125rem; font-weight: 600; }
.muted-card-wrap {
  width: 100%;
}
</style>
