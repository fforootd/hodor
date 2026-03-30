<template>
  <!-- Login-02 / Signup-02: Split-screen — form left, cover image right -->
  <div class="login-layout-split" :class="{ 'is-preview': preview }" :style="bgVars">
    <link v-if="branding?.font_url" rel="stylesheet" :href="branding.font_url" />
    <div class="split-form-side">
      <!-- Brand logo top-left -->
      <div class="split-brand">
        <a href="#" class="split-brand-link">
          <img v-if="logo" :src="logo" :alt="branding?.org_name || 'Logo'" class="split-logo" />
          <span v-else class="split-org-name">{{ branding?.org_name || 'Zitadel' }}</span>
        </a>
      </div>
      <!-- Form content -->
      <div class="split-form-content">
        <div class="split-form-inner">
          <slot />
        </div>
      </div>
    </div>
    <!-- Cover image side -->
    <div class="split-cover-side">
      <img
        v-if="branding?.cover_image"
        :src="branding.cover_image"
        alt=""
        class="split-cover-img"
      />
      <div v-else class="split-cover-placeholder">
        <svg viewBox="0 0 24 24" class="split-cover-icon" fill="none" stroke="currentColor" stroke-width="1">
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <circle cx="8.5" cy="8.5" r="1.5" />
          <path d="m21 15-5-5L5 21" />
        </svg>
      </div>
    </div>
    <slot name="footer" />
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
}))
</script>

<style scoped>
.login-layout-split {
  display: grid;
  min-height: 100svh;
}
@media (min-width: 1024px) {
  .login-layout-split { grid-template-columns: 1fr 1fr; }
}
.login-layout-split.is-preview {
  min-height: 100%;
  grid-template-columns: 1fr 1fr;
}
.split-form-side {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1.5rem;
}
.login-layout-split.is-preview .split-form-side {
  padding: 1rem;
}
@media (min-width: 768px) {
  .split-form-side { padding: 2.5rem; }
}
.split-brand {
  display: flex;
  justify-content: center;
  gap: 0.5rem;
}
@media (min-width: 768px) {
  .split-brand { justify-content: flex-start; }
}
.split-brand-link {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 500;
  text-decoration: none;
  color: inherit;
}
.split-logo { height: 1.5rem; }
.split-org-name { font-size: 1rem; font-weight: 600; }
.split-form-content {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
}
.split-form-inner { width: 100%; max-width: 20rem; }
.split-cover-side {
  position: relative;
  display: none;
  background: hsl(var(--muted, 0 0% 96%));
  overflow: hidden;
}
@media (min-width: 1024px) {
  .split-cover-side { display: block; }
}
.login-layout-split.is-preview .split-cover-side {
  display: block;
}
.split-cover-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.split-cover-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  color: hsl(0 0% 70%);
}
.split-cover-icon { width: 4rem; height: 4rem; opacity: 0.5; }
</style>
