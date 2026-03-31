<template>
  <ResourceCreateCockpit
    v-model:active-tab="activeTab"
    v-model:json-content="jsonContent"
    back-route="/projects"
    :badges="badges"
    :curl-snippets="curlSnippets"
    description="Shape the project around its schema-backed fields, keep org context visible, and leave developer tooling in its own lane."
    details-description="Capture the project details that matter for operators and collaboration."
    :error="error"
    eyebrow="Project creation"
    :json-error="jsonError"
    :json-valid="jsonValid"
    :review-rows="reviewRows"
    :review-summary-cards="reviewSummaryCards"
    :schema="schemaContext.schema"
    singular-title="Project"
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
import { projectApi } from '@/api/resources'
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
  schemaType: 'project',
  apiPath: '/v1/projects',
  resourceName: 'Project',
  listRoute: '/projects',
  createFn: projectApi.create,
  includeOrgHeader: true,
})

const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const projectName = computed(() => String(formData.value.name || 'Pending project name'))
const projectDescription = computed(() => String(formData.value.description || 'No description yet'))
const badges = computed(() => ([
  { label: 'Project', variant: 'outline' as const },
  { label: schemaContext.value.schemaType || 'project', variant: 'secondary' as const },
  ...(currentOrgId.value ? [{ label: currentOrgId.value, variant: 'secondary' as const }] : []),
]))
const summaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Project', value: projectName.value },
  { label: 'Organization context', value: currentOrgId.value || 'No org selected' },
  { label: 'Description', value: projectDescription.value },
]))
const reviewRows = computed(() => reviewFacts.value)
const reviewSummaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Project', value: projectName.value },
  { label: 'Org context', value: currentOrgId.value || 'No org selected' },
  { label: 'Developer payload', value: `${Object.keys(payload.value.data || {}).length} schema fields in payload` },
]))
</script>
