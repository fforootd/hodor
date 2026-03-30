<template>
  <div class="flex min-h-screen items-center justify-center bg-background px-6 py-10">
    <div
      class="flex w-full max-w-md flex-col items-center gap-5 rounded-2xl border bg-card px-8 py-10 text-center shadow-sm"
    >
      <div
        class="flex size-12 items-center justify-center rounded-full"
        :class="isFatal ? 'bg-destructive/10 text-destructive' : 'bg-primary/10 text-primary'"
      >
        <AlertCircle v-if="isFatal" class="size-6" />
        <Spinner v-else class="size-6" />
      </div>

      <div class="space-y-2">
        <p class="text-lg font-semibold">{{ title }}</p>
        <p class="text-sm text-muted-foreground">{{ description }}</p>
        <p
          v-if="state === 'waiting_for_server' && retryDelayMs > 0"
          class="text-xs text-muted-foreground/80"
        >
          Retrying soon...
        </p>
        <p
          v-if="state === 'fatal' && error?.kind === 'configuration' && configurationHint"
          class="text-xs text-muted-foreground/80"
        >
          {{ configurationHint }}
        </p>
      </div>

      <Button v-if="isFatal" type="button" class="w-full" @click="$emit('retry')">
        Retry
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed } from 'vue'
  import { AlertCircle } from 'lucide-vue-next'
  import { Button } from '@/components/ui/button'
  import { Spinner } from '@/components/ui/spinner'
  import type {
    AppBootstrapErrorDetail,
    AppBootstrapState,
  } from '@/bootstrap/app-bootstrap'

  const props = withDefaults(
    defineProps<{
      state: AppBootstrapState
      appName: string
      error?: AppBootstrapErrorDetail | null
      retryDelayMs?: number
      configurationHint?: string
    }>(),
    {
      error: null,
      retryDelayMs: 0,
      configurationHint: '',
    },
  )

  defineEmits<{
    retry: []
  }>()

  const normalizedAppName = computed(() => props.appName.trim() || 'app')
  const capitalizedAppName = computed(() => {
    const [first = '', ...rest] = normalizedAppName.value
    return `${first.toUpperCase()}${rest.join('')}`
  })
  const isFatal = computed(() => props.state === 'fatal')

  const title = computed(() => {
    if (props.state === 'initializing') return `Loading ${normalizedAppName.value}`
    if (props.state === 'waiting_for_server') return 'Starting Zitadel'
    if (props.state === 'fatal') return `${capitalizedAppName.value} is unavailable`
    return `Loading ${normalizedAppName.value}`
  })

  const description = computed(() => {
    if (props.state === 'initializing') {
      return `Preparing ${normalizedAppName.value} before we render the app.`
    }
    if (props.state === 'waiting_for_server') {
      return props.error?.message || 'Zitadel is still starting. Try again in a moment.'
    }
    if (props.state === 'fatal') {
      return props.error?.message || `${capitalizedAppName.value} is temporarily unavailable.`
    }
    return ''
  })
</script>
