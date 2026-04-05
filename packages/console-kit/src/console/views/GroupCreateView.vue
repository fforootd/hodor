<template>
  <ResourceCreateCockpit
    v-model:active-tab="activeTab"
    v-model:json-content="jsonContent"
    back-route="/groups"
    :badges="badges"
    :curl-snippets="curlSnippets"
    description="Define the group around schema-backed fields, keep org context visible, and separate developer tooling from the main operator flow."
    details-description="Capture the group profile and collaboration framing first."
    :error="error"
    eyebrow="Group creation"
    :json-error="jsonError"
    :json-valid="jsonValid"
    :review-rows="reviewRows"
    :review-summary-cards="reviewSummaryCards"
    :schema="schemaContext.schema"
    singular-title="Group"
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
        Loading schema…
      </div>
    </template>
  </ResourceCreateCockpit>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { groupApi } from '@/api/resources'
import ResourceCreateCockpit from '@/console/components/ResourceCreateCockpit.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { useResourceCreate } from '@/console/composables/useResourceCreate'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { extractSchemaFields, type SummaryFact } from '@/console/utils/schema-resource'

const activeTab = ref('details')
const { currentOrgId } = useOrgContext()

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
  schemaType: 'group',
  apiPath: '/v1/groups',
  resourceName: 'Group',
  listRoute: '/groups',
  createFn: groupApi.create,
  includeOrgHeader: true,
})

const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const groupName = computed(() => String(formData.value.name || 'Pending group name'))
const groupDescription = computed(() => String(formData.value.description || 'No description yet'))
const badges = computed(() => ([
  { label: 'Group', variant: 'outline' as const },
  { label: schemaContext.value.schemaType || 'group', variant: 'secondary' as const },
  ...(currentOrgId.value ? [{ label: currentOrgId.value, variant: 'secondary' as const }] : []),
]))
const summaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Group', value: groupName.value },
  { label: 'Organization context', value: currentOrgId.value || 'No org selected' },
  { label: 'Description', value: groupDescription.value },
]))
const reviewRows = computed(() => reviewFacts.value)
const reviewSummaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Group', value: groupName.value },
  { label: 'Org context', value: currentOrgId.value || 'No org selected' },
  { label: 'Developer payload', value: `${Object.keys(payload.value.data || {}).length} schema fields in payload` },
]))
</script>
