<template>
  <Tabs v-model="activeTab" class="space-y-4">
    <TabsList class="grid w-full max-w-md grid-cols-3">
      <TabsTrigger value="form">Form</TabsTrigger>
      <TabsTrigger value="json">JSON</TabsTrigger>
      <TabsTrigger value="curl">cURL</TabsTrigger>
    </TabsList>

    <TabsContent value="form">
      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-sm">{{ formTitle }}</CardTitle>
        </CardHeader>
        <CardContent>
          <SchemaFieldEditor
            v-if="fields.length"
            :fields="fields"
            :model-value="safeModelValue"
            @update:model-value="updateForm"
          />
          <p v-else class="text-sm text-muted-foreground">No schema fields available.</p>
        </CardContent>
      </Card>
    </TabsContent>

    <TabsContent value="json">
      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-sm">Canonical JSON</CardTitle>
        </CardHeader>
        <CardContent class="space-y-2">
          <JsonEditor
            v-model="jsonContent"
            label="Schema Data"
            :schema="schema || undefined"
            height="420px"
            @valid="onJsonValid"
            @error="onJsonError"
          />
          <p v-if="jsonError" class="text-xs text-destructive">{{ jsonError }}</p>
        </CardContent>
      </Card>
    </TabsContent>

    <TabsContent value="curl">
      <CurlSnippetPanel :snippets="curlSnippets" />
    </TabsContent>
  </Tabs>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { CurlSnippet } from '@/console/utils/schema-resource'
import {
  extractSchemaFields,
  normalizeResourceData,
  stringifyResourceData,
} from '@/console/utils/schema-resource'
import CurlSnippetPanel from '@/console/components/CurlSnippetPanel.vue'
import JsonEditor from '@/console/components/JsonEditor.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

const props = withDefaults(defineProps<{
  curlSnippets: CurlSnippet[]
  formTitle?: string
  modelValue: Record<string, any>
  schema: Record<string, any> | null
}>(), {
  formTitle: 'Fields',
})

const emit = defineEmits<{
  'update:modelValue': [value: Record<string, any>]
  'update:jsonValid': [value: boolean]
}>()

const activeTab = ref('form')
const jsonContent = ref('{}')
const jsonError = ref('')

const safeModelValue = computed(() => normalizeResourceData(props.modelValue))
const fields = computed(() => extractSchemaFields(props.schema))

watch(() => props.modelValue, (value) => {
  const next = stringifyResourceData(normalizeResourceData(value))
  if (next !== jsonContent.value) {
    jsonContent.value = next
  }
}, { deep: true, immediate: true })

function updateForm(value: Record<string, any>) {
  emit('update:modelValue', normalizeResourceData(value))
}

function onJsonValid(parsed: Record<string, any>) {
  jsonError.value = ''
  emit('update:jsonValid', true)
  emit('update:modelValue', normalizeResourceData(parsed))
}

function onJsonError(message: string) {
  jsonError.value = message
  emit('update:jsonValid', false)
}
</script>
