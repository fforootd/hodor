<script setup lang="ts">
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<{
  class?: string
  variant?: 'default' | 'icon'
}>(), {
  variant: 'default',
})
</script>

<template>
  <div
    :class="cn(
      'flex items-center justify-center',
      variant === 'icon' && 'rounded-full bg-muted p-3 [&>svg]:size-6 [&>svg]:text-muted-foreground',
      props.class,
    )"
  >
    <slot />
  </div>
</template>
