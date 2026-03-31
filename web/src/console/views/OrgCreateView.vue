<template>
  <ResourceCreateCockpit
    v-model:active-tab="activeTab"
    v-model:json-content="jsonContent"
    back-route="/orgs"
    :badges="badges"
    :curl-snippets="curlSnippets"
    description="Start with the organization profile, review the schema-backed values, and keep developer tooling separate."
    details-description="Capture the organization details operators care about first."
    :error="error"
    eyebrow="Organization creation"
    :json-error="jsonError"
    :json-valid="jsonValid"
    :review-rows="reviewRows"
    :review-summary-cards="reviewSummaryCards"
    :schema="schemaContext.schema"
    singular-title="Organization"
    :submitting="submitting"
    :summary-cards="summaryCards"
    @submit="submit"
    @json-valid="onJsonValid"
    @json-error="onJsonError"
  >
    <template #details>
      <SchemaFieldEditor
        v-if="schemaContext.schema"
        v-model="formData"
        :fields="schemaFields"
      />
      <div v-else class="flex items-center gap-2 text-sm text-muted-foreground">
        <Spinner class="size-4" />
        Loading schema…
      </div>
    </template>
  </ResourceCreateCockpit>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { orgApi } from '@/api/resources'
import ResourceCreateCockpit from '@/console/components/ResourceCreateCockpit.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { useResourceCreate } from '@/console/composables/useResourceCreate'
import { extractSchemaFields, type SummaryFact } from '@/console/utils/schema-resource'
import { Spinner } from '@/components/ui/spinner'

const activeTab = ref('details')

const {
  schemaContext,
  formData,
  jsonValid,
  jsonContent,
  jsonError,
  submitting,
  error,
  payload,
  curlSnippets,
  reviewFacts,
  submit,
  onJsonValid,
  onJsonError,
} = useResourceCreate({
  schemaType: 'org',
  apiPath: '/v1/orgs',
  resourceName: 'Organization',
  listRoute: '/orgs',
  createFn: orgApi.create,
})

const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const displayName = computed(() => String(formData.value.display_name || formData.value.name || 'Pending name'))
const metadataStatus = computed(() =>
  Object.keys((payload.value.metadata as Record<string, unknown>) || {}).length
    ? 'Metadata included'
    : 'No metadata yet',
)
const badges = computed(() => ([
  { label: 'Organization', variant: 'outline' as const },
  { label: schemaContext.value.schemaType || 'org', variant: 'secondary' as const },
]))
const summaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Name preview', value: displayName.value },
  { label: 'Metadata', value: metadataStatus.value },
  { label: 'Payload', value: `${Object.keys(payload.value.data || {}).length} schema fields in payload` },
]))
const reviewRows = computed(() => reviewFacts.value)
const reviewSummaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Organization', value: displayName.value },
  { label: 'Schema', value: schemaContext.value.display.singular || 'Organization' },
  { label: 'Developer payload', value: `${Object.keys(payload.value.data || {}).length} schema fields in payload` },
]))
</script>
