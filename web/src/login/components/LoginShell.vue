<template>
  <div class="login-shell" :style="themeVars">
    <component :is="layoutComponent" :branding="resolvedBranding" :preview="preview">
      <link v-if="resolvedBranding?.font_url" rel="stylesheet" :href="resolvedBranding.font_url" />
      <component
        v-if="resolvedBranding?.custom_css"
        :is="'style'"
        v-text="resolvedBranding.custom_css"
      />

      <Card class="w-full" :class="cardClass">
        <CardHeader v-if="showCardBranding" class="text-center">
          <div v-if="effectiveLogo" class="mb-2 flex justify-center">
            <img :src="effectiveLogo" :alt="resolvedBranding?.org_name" class="h-8" />
          </div>
          <div v-else class="mb-2 text-xl font-bold tracking-tight">
            {{ resolvedBranding?.org_name || 'Zitadel' }}
          </div>
        </CardHeader>

        <CardContent :class="{ 'pt-6': !showCardBranding }">
          <slot :branding="resolvedBranding" />
        </CardContent>
      </Card>

      <template #footer>
        <p
          v-if="!resolvedBranding?.hide_zitadel_branding"
          class="mt-6 text-center text-xs text-muted-foreground"
        >
          Powered by Zitadel
        </p>
      </template>
    </component>
  </div>
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
  if ((layout === 'split' || layout === 'card_image') && !resolvedBranding.value?.cover_image) {
    return 'centered'
  }
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
const showCardBranding = computed(() => !['muted', 'split'].includes(effectiveLayout.value))

const cardClass = computed(() => {
  if (effectiveLayout.value === 'split' || effectiveLayout.value === 'card_image') {
    return ''
  }
  return 'max-w-sm'
})

const radiusMap: Record<string, string> = {
  sm: '0.5rem',
  md: '0.75rem',
  lg: '1rem',
  xl: '1.5rem',
  full: '9999px',
}

const themeVars = computed<Record<string, string>>(() => {
  const branding = resolvedBranding.value
  const colors = branding?.colors || {}

  return {
    '--color-background': colors.background || '#f6f6f3',
    '--color-foreground': colors.text || '#16161c',
    '--color-card': colors.surface || '#ffffff',
    '--color-card-foreground': colors.text || '#16161c',
    '--color-popover': colors.surface || '#ffffff',
    '--color-popover-foreground': colors.text || '#16161c',
    '--color-primary': colors.primary || '#f25543',
    '--color-primary-foreground': colors.primary_foreground || '#ffffff',
    '--color-secondary': colors.muted || '#ededeb',
    '--color-secondary-foreground': colors.text || '#16161c',
    '--color-muted': colors.muted || '#ededeb',
    '--color-muted-foreground': colors.text || '#6b6b76',
    '--color-accent': colors.accent || colors.primary || '#f25543',
    '--color-accent-foreground': colors.primary_foreground || '#ffffff',
    '--color-border': colors.border || '#dddde4',
    '--color-input': colors.border || '#dddde4',
    '--color-ring': colors.accent || colors.primary || '#f25543',
    '--color-destructive': colors.error || '#ef4444',
    '--color-destructive-foreground': '#ffffff',
    '--radius': radiusMap[branding?.border_radius || 'md'] || radiusMap.md,
    '--brand-primary': colors.primary || '#f25543',
    '--brand-background': colors.background || '#f6f6f3',
    '--brand-surface': colors.surface || '#ffffff',
    '--brand-text': colors.text || '#16161c',
    '--brand-error': colors.error || '#ef4444',
  }
})
</script>
