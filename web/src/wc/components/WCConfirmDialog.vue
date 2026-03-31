<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
    role="dialog"
    aria-modal="true"
    @click.self="onCancel"
  >
    <div class="w-full max-w-md rounded-xl border border-[var(--color-border)] bg-[var(--color-card)] p-5 shadow-2xl">
      <div class="space-y-2">
        <h3 class="text-base font-semibold tracking-tight text-[var(--color-card-foreground,var(--color-foreground))]">
          {{ title }}
        </h3>
        <p class="text-sm leading-6 text-[var(--color-muted-foreground)]">
          {{ description }}
        </p>
      </div>

      <div class="mt-5 flex justify-end gap-2">
        <button
          type="button"
          class="inline-flex h-9 items-center justify-center rounded-md border border-[var(--color-border)] bg-transparent px-4 text-sm font-medium text-[var(--color-foreground)] transition-colors hover:bg-[var(--color-muted)] disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="loading"
          @click="onCancel"
        >
          {{ cancelLabel }}
        </button>
        <button
          type="button"
          class="inline-flex h-9 items-center justify-center rounded-md px-4 text-sm font-medium text-[var(--color-destructive-foreground)] transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
          :style="{ backgroundColor: 'var(--color-destructive)' }"
          :disabled="loading"
          @click="$emit('confirm')"
        >
          {{ loading ? loadingLabel : confirmLabel }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    open: boolean
    title: string
    description: string
    confirmLabel?: string
    cancelLabel?: string
    loading?: boolean
    loadingLabel?: string
  }>(),
  {
    confirmLabel: 'Delete',
    cancelLabel: 'Cancel',
    loading: false,
    loadingLabel: 'Deleting…',
  },
)

const emit = defineEmits<{
  confirm: []
  'update:open': [value: boolean]
}>()

function onCancel() {
  if (props.loading) return
  emit('update:open', false)
}
</script>
