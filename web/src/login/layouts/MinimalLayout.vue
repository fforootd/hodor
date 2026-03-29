<template>
  <!-- Login-05 / Signup-05: Minimal — clean bg, social-first, email below -->
  <div class="login-layout-minimal" :style="bgVars">
    <link v-if="branding?.font_url" rel="stylesheet" :href="branding.font_url" />
    <div class="minimal-inner">
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
}>()

const bgVars = computed(() => ({
  fontFamily: props.branding?.font_family || 'Inter, system-ui, sans-serif',
}))
</script>

<style scoped>
.login-layout-minimal {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100svh;
  width: 100%;
  padding: 1.5rem;
  background: hsl(var(--background, 0 0% 100%));
}
@media (min-width: 768px) {
  .login-layout-minimal { padding: 2.5rem; }
}
.minimal-inner {
  width: 100%;
  max-width: 22rem;
}
</style>
