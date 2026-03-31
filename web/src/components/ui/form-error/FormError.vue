<template>
  <Alert v-if="error" variant="destructive">
    <AlertTriangleIcon class="size-4" />
    <AlertDescription>{{ error }}</AlertDescription>
  </Alert>
</template>

<script setup lang="ts">
import { Alert, AlertDescription } from '@/components/ui/alert'
import { AlertTriangle as AlertTriangleIcon } from 'lucide-vue-next'

defineProps<{
  error: string
}>()
</script>
