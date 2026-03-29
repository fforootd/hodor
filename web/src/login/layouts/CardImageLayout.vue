<template>
  <!-- Login-04 / Signup-04: Wide card with embedded image alongside form -->
  <div class="login-layout-card-image" :style="bgVars">
    <link v-if="branding?.font_url" rel="stylesheet" :href="branding.font_url" />
    <div class="card-image-outer">
      <div class="card-image-card">
        <!-- Form side -->
        <div class="card-image-form">
          <slot />
        </div>
        <!-- Image side (inline in the card) -->
        <div class="card-image-media">
          <img
            v-if="branding?.cover_image"
            :src="branding.cover_image"
            alt=""
            class="card-image-img"
          />
          <div v-else class="card-image-placeholder">
            <svg viewBox="0 0 24 24" class="card-image-icon" fill="none" stroke="currentColor" stroke-width="1">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <circle cx="8.5" cy="8.5" r="1.5" />
              <path d="m21 15-5-5L5 21" />
            </svg>
          </div>
        </div>
      </div>
      <slot name="footer" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { FlowBranding } from '@/api/branding'

const props = defineProps<{
  branding: FlowBranding | null
}>()

const bgVars = computed(() => {
  const c = props.branding?.colors || {}
  const bg = c.background || '#f0f2ff'
  return {
    background: `linear-gradient(135deg, ${bg} 0%, #fafbff 50%, #f5f3ff 100%)`,
    fontFamily: props.branding?.font_family || 'Inter, system-ui, sans-serif',
  }
})
</script>

<style scoped>
.login-layout-card-image {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100svh;
  width: 100%;
  padding: 1.5rem;
}
@media (min-width: 768px) {
  .login-layout-card-image { padding: 2.5rem; }
}
.card-image-outer {
  width: 100%;
  max-width: 56rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.5rem;
}
.card-image-card {
  display: grid;
  width: 100%;
  overflow: hidden;
  border-radius: var(--radius, 0.75rem);
  border: 1px solid hsl(var(--border, 0 0% 90%));
  background: hsl(var(--card, 0 0% 100%));
  box-shadow: 0 4px 14px -3px rgba(0, 0, 0, 0.1);
}
@media (min-width: 768px) {
  .card-image-card { grid-template-columns: 1fr 1fr; }
}
.card-image-form {
  padding: 2rem;
  display: flex;
  flex-direction: column;
  justify-content: center;
}
@media (min-width: 768px) {
  .card-image-form { padding: 2.5rem; }
}
.card-image-media {
  display: none;
  position: relative;
  background: hsl(var(--muted, 0 0% 96%));
  min-height: 20rem;
  overflow: hidden;
}
@media (min-width: 768px) {
  .card-image-media { display: block; }
}
.card-image-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.card-image-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  color: hsl(0 0% 70%);
}
.card-image-icon { width: 3rem; height: 3rem; opacity: 0.5; }
</style>
