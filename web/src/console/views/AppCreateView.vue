<template>
  <ResourceCreateCockpit
    v-model:active-tab="activeTab"
    v-model:json-content="jsonContent"
    back-route="/applications"
    :badges="badges"
    :curl-snippets="curlSnippets"
    description="Start with the application profile, validate protocol posture before creation, and keep developer tooling separated."
    details-description="Capture the application details operators care about first."
    :error="error"
    eyebrow="Application creation"
    :extra-tabs="[{ label: 'Protocol', value: 'protocol' }]"
    :json-error="jsonError"
    :json-valid="jsonValid"
    :review-rows="reviewRows"
    :review-summary-cards="reviewSummaryCards"
    :schema="schemaContext.schema"
    singular-title="Application"
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

    <template #tab-protocol>
      <div class="grid gap-6 xl:grid-cols-2">
        <Card class="rounded-3xl shadow-sm">
          <CardHeader class="pb-3">
            <CardTitle class="text-lg">Protocol posture</CardTitle>
            <p class="text-sm text-muted-foreground">
              Review the application protocol defaults before creating the resource.
            </p>
          </CardHeader>
          <CardContent class="space-y-3">
            <div
              v-for="(row, rowIndex) in protocolRows"
              :key="rowIndex"
              class="rounded-2xl border bg-muted/20 px-4 py-3"
            >
              <p class="text-[11px] uppercase tracking-wider text-muted-foreground">{{ row.label }}</p>
              <p class="mt-1 text-sm font-medium">{{ row.value }}</p>
            </div>
          </CardContent>
        </Card>

        <Card class="rounded-3xl shadow-sm">
          <CardHeader class="pb-3">
            <CardTitle class="text-lg">Readiness</CardTitle>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="rounded-2xl border bg-muted/20 px-4 py-3">
              <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Redirect URIs</p>
              <p class="mt-1 text-sm font-medium">{{ redirectUriSummary }}</p>
            </div>
            <div class="rounded-2xl border bg-muted/20 px-4 py-3">
              <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Grant types</p>
              <p class="mt-1 text-sm font-medium">{{ grantTypeSummary }}</p>
            </div>
            <div class="rounded-2xl border bg-muted/20 px-4 py-3">
              <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Response types</p>
              <p class="mt-1 text-sm font-medium">{{ responseTypeSummary }}</p>
            </div>
          </CardContent>
        </Card>
      </div>
    </template>
  </ResourceCreateCockpit>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { appApi } from '@/api/resources'
import ResourceCreateCockpit from '@/console/components/ResourceCreateCockpit.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { useResourceCreate } from '@/console/composables/useResourceCreate'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { extractSchemaFields, type SummaryFact } from '@/console/utils/schema-resource'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

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
  schemaType: 'app',
  apiPath: '/v1/apps',
  resourceName: 'Application',
  listRoute: '/applications',
  createFn: appApi.create,
  includeOrgHeader: true,
  defaultFormData: {
    app_type: 'web',
    redirect_uris: [],
    grant_types: ['authorization_code'],
    response_types: ['code'],
  },
})

const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const appName = computed(() => String(formData.value.client_name || formData.value.name || 'Pending application name'))
const appType = computed(() => String(formData.value.app_type || 'web'))
const redirectUris = computed(() => Array.isArray(formData.value.redirect_uris) ? formData.value.redirect_uris : [])
const grantTypes = computed(() => Array.isArray(formData.value.grant_types) ? formData.value.grant_types : [])
const responseTypes = computed(() => Array.isArray(formData.value.response_types) ? formData.value.response_types : [])
const redirectUriSummary = computed(() =>
  redirectUris.value.length ? `${redirectUris.value.length} configured` : 'No redirect URIs yet',
)
const grantTypeSummary = computed(() => grantTypes.value.length ? grantTypes.value.join(', ') : 'No grant types configured')
const responseTypeSummary = computed(() => responseTypes.value.length ? responseTypes.value.join(', ') : 'No response types configured')
const badges = computed(() => ([
  { label: 'Application', variant: 'outline' as const },
  { label: schemaContext.value.schemaType || 'app', variant: 'secondary' as const },
  ...(currentOrgId.value ? [{ label: currentOrgId.value, variant: 'secondary' as const }] : []),
]))
const summaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Application', value: appName.value },
  { label: 'App type', value: appType.value },
  { label: 'Organization context', value: currentOrgId.value || 'No org selected' },
]))
const protocolRows = computed<SummaryFact[]>(() => ([
  { label: 'Application type', value: appType.value },
  { label: 'Grant types', value: grantTypeSummary.value },
  { label: 'Response types', value: responseTypeSummary.value },
  { label: 'Redirect URIs', value: redirectUriSummary.value },
]))
const reviewRows = computed(() => reviewFacts.value)
const reviewSummaryCards = computed<SummaryFact[]>(() => ([
  { label: 'Application', value: appName.value },
  { label: 'Protocol', value: `${appType.value} application` },
  { label: 'Developer payload', value: `${Object.keys(payload.value.data || {}).length} schema fields in payload` },
]))
</script>
