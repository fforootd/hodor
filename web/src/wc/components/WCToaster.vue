<script setup lang="ts">
import { computed } from 'vue'
import { Toaster } from '@/components/ui/sonner'

const props = defineProps<{
  dark?: boolean
}>()

const theme = computed(() => (props.dark ? 'dark' : 'light'))
</script>

<template>
  <Toaster position="top-right" :expand="true" rich-colors :theme="theme" />
</template>
