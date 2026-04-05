<template>
  <!-- Login-01 / Signup-01: Centered card on gradient background -->
  <div class="login-layout-centered" :class="{ 'is-preview': preview }" :style="bgVars">
    <link v-if="branding?.font_url" rel="stylesheet" :href="branding.font_url" />
    <div class="centered-inner">
      <slot />
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

const bgVars = computed(() => {
  const c = props.branding?.colors || {}
  const bg = c.background || '#f0f2ff'
  return {
    '--brand-bg': bg,
    background: `linear-gradient(135deg, ${bg} 0%, #fafbff 50%, #f5f3ff 100%)`,
    fontFamily: props.branding?.font_family || 'Inter, system-ui, sans-serif',
  }
})
</script>

<style scoped>
.login-layout-centered {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100svh;
  width: 100%;
  padding: 1.5rem;
}
.login-layout-centered.is-preview {
  min-height: 100%;
  padding: 1rem;
}
@media (min-width: 768px) {
  .login-layout-centered { padding: 2.5rem; }
}
.centered-inner {
  width: 100%;
  max-width: 24rem;
}
</style>
