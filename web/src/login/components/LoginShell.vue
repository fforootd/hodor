<template>
  <component :is="layoutComponent" :branding="resolvedBranding" :preview="preview">
    <link v-if="resolvedBranding?.font_url" rel="stylesheet" :href="resolvedBranding.font_url" />
    <component
      v-if="resolvedBranding?.custom_css"
      :is="'style'"
      v-text="resolvedBranding.custom_css"
    />

    <Card class="w-full" :class="cardClass">
      <CardHeader class="text-center">
        <div v-if="effectiveLogo" class="flex justify-center mb-2">
          <img :src="effectiveLogo" :alt="resolvedBranding?.org_name" class="h-8" />
        </div>
        <div v-else class="text-xl font-bold tracking-tight mb-2">
          {{ resolvedBranding?.org_name || 'Zitadel' }}
        </div>
      </CardHeader>

      <CardContent>
        <slot :branding="resolvedBranding" />
      </CardContent>
    </Card>

    <template #footer>
      <p
        v-if="!resolvedBranding?.hide_zitadel_branding"
        class="mt-6 text-xs text-muted-foreground text-center"
      >
        Powered by Zitadel
      </p>
    </template>
  </component>
</template>

<script setup lang="ts">
import { computed, type Component } from 'vue'
import type { FlowBranding } from '@/api/branding'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import CenteredLayout from '../layouts/CenteredLayout.vue'
import SplitLayout from '../layouts/SplitLayout.vue'
import MutedLayout from '../layouts/MutedLayout.vue'
import CardImageLayout from '../layouts/CardImageLayout.vue'
import MinimalLayout from '../layouts/MinimalLayout.vue'

const layoutMap: Record<string, Component> = {
  centered: CenteredLayout,
  split: SplitLayout,
  muted: MutedLayout,
  card_image: CardImageLayout,
  minimal: MinimalLayout,
}

const props = withDefaults(
  defineProps<{
    branding: FlowBranding | null
    layoutOverride?: string
    darkModeOverride?: string
    coverImageOverride?: string
    primaryColorOverride?: string
    preview?: boolean
  }>(),
  {
    layoutOverride: '',
    darkModeOverride: '',
    coverImageOverride: '',
    primaryColorOverride: '',
    preview: false,
  },
)

const resolvedBranding = computed<FlowBranding | null>(() => {
  if (!props.branding) return null
  const colors = props.primaryColorOverride
    ? { ...props.branding.colors, primary: props.primaryColorOverride }
    : props.branding.colors

  return {
    ...props.branding,
    colors,
    cover_image: props.coverImageOverride || props.branding.cover_image,
    layout: props.layoutOverride || props.branding.layout,
    dark_mode: props.darkModeOverride || props.branding.dark_mode,
  }
})

const effectiveLayout = computed(() => {
  const layout = resolvedBranding.value?.layout || props.layoutOverride || 'centered'
  return layoutMap[layout] ? layout : 'centered'
})

const effectiveDarkMode = computed(() => {
  const mode = resolvedBranding.value?.dark_mode || props.darkModeOverride || 'light'
  if (mode === 'auto' && typeof window !== 'undefined') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }
  return mode
})

const effectiveLogo = computed(() => {
  if (effectiveDarkMode.value === 'dark' && resolvedBranding.value?.logo_dark) {
    return resolvedBranding.value.logo_dark
  }
  return resolvedBranding.value?.logo_url || ''
})

const layoutComponent = computed(() => layoutMap[effectiveLayout.value] || CenteredLayout)

const cardClass = computed(() => {
  if (effectiveLayout.value === 'split' || effectiveLayout.value === 'card_image') {
    return ''
  }
  return 'max-w-sm'
})
</script>
