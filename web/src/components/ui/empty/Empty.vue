<script setup lang="ts">
import { cn } from '@/lib/utils'

const props = defineProps<{ class?: string }>()
</script>

<template>
  <div :class="cn('flex min-h-[200px] flex-col items-center justify-center gap-6 rounded-lg p-8 text-center', props.class)">
    <slot />
  </div>
</template>
