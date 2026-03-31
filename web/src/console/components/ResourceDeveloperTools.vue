<template>
  <Card class="rounded-3xl shadow-sm">
    <CardHeader class="pb-3">
      <CardTitle class="text-lg">{{ title }}</CardTitle>
      <p class="text-sm text-muted-foreground">
        {{ description }}
      </p>
    </CardHeader>
    <CardContent class="space-y-6">
      <JsonEditor
        v-model="jsonContent"
        label="Canonical JSON"
        :schema="schema || undefined"
        height="420px"
        @valid="$emit('json-valid', $event)"
        @error="$emit('json-error', $event)"
      />
      <p v-if="jsonError" class="text-xs text-destructive">{{ jsonError }}</p>
      <CurlSnippetPanel :snippets="curlSnippets" />
    </CardContent>
  </Card>
</template>

<script setup lang="ts">
import type { CurlSnippet } from '@/console/utils/schema-resource'
import CurlSnippetPanel from '@/console/components/CurlSnippetPanel.vue'
import JsonEditor from '@/console/components/JsonEditor.vue'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

withDefaults(defineProps<{
  curlSnippets: CurlSnippet[]
  description?: string
  jsonError?: string
  schema: Record<string, any> | null
  title?: string
}>(), {
  description: 'JSON and cURL stay available for inspection without taking over the main operator flow.',
  jsonError: '',
  title: 'Developer Mode',
})

const jsonContent = defineModel<string>('jsonContent', { default: '{}' })

defineEmits<{
  'json-valid': [parsed: Record<string, any>]
  'json-error': [message: string]
}>()
</script>
