<script setup lang="ts">
import { cn } from '@/lib/utils'

const props = defineProps<{ class?: string }>()
</script>

<template>
  <p :class="cn('text-sm text-muted-foreground max-w-md', props.class)">
    <slot />
  </p>
</template>
