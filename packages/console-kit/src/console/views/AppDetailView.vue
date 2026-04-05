<template>
  <ResourceDetailCockpit
    v-model:active-tab="activeTab"
    v-model:json-content="jsonContent"
    back-route="/applications"
    :badges="badges"
    :curl-snippets="curlSnippets"
    :deleting="deleting"
    :display-title="appTitle"
    eyebrow="Application cockpit"
    :extra-tabs="[{ label: 'Protocol', value: 'protocol' }]"
    :json-error="jsonError"
    :json-valid="jsonValid"
    :load-error="loadError"
    :loading="loading"
    :overview-facts="overviewFacts"
    :resource="item"
    :saving="saving"
    :schema="schemaContext.schema"
    singular-title="Application"
    :state-rows="stateRows"
    :subtitle="item?.client_id || ''"
    @save="save"
    @delete="deleteResource"
    @json-valid="onJsonValid"
    @json-error="onJsonError"
  >
    <template #tab-protocol>
      <div class="grid gap-6 xl:grid-cols-2">
        <Card class="rounded-3xl shadow-sm">
          <CardHeader class="pb-3">
            <CardTitle class="text-lg">Protocol</CardTitle>
            <p class="text-sm text-muted-foreground">
              Review the runtime-facing protocol details for this application.
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
            <CardTitle class="text-lg">Routing</CardTitle>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="rounded-2xl border bg-muted/20 px-4 py-3">
              <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Redirect URIs</p>
              <p class="mt-1 text-sm font-medium">{{ redirectUriSummary }}</p>
            </div>
            <div class="rounded-2xl border bg-muted/20 px-4 py-3">
              <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Post logout URIs</p>
              <p class="mt-1 text-sm font-medium">{{ postLogoutSummary }}</p>
            </div>
          </CardContent>
        </Card>
      </div>
    </template>

    <template #edit-form>
      <SchemaFieldEditor
        v-if="schemaContext.schema"
        v-model="formData"
        :fields="schemaFields"
      />
      <div v-else class="text-sm text-muted-foreground">Loading schema…</div>
    </template>
  </ResourceDetailCockpit>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { appApi } from '@/api/resources'
import ResourceDetailCockpit from '@/console/components/ResourceDetailCockpit.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { useResourceDetail } from '@/console/composables/useResourceDetail'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { formatDateTime } from '@/console/utils/format'
import { extractSchemaFields, type SummaryFact } from '@/console/utils/schema-resource'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

const activeTab = ref('overview')
const { currentOrgId } = useOrgContext()

const {
  item,
  formData,
  schemaContext,
  loading,
  saving,
  deleting,
  jsonValid,
  jsonContent,
  jsonError,
  loadError,
  curlSnippets,
  overviewFacts,
  save,
  deleteResource,
  onJsonValid,
  onJsonError,
} = useResourceDetail({
  resourceType: 'app',
  resourceName: 'Application',
  listRoute: '/applications',
  apiPath: '/v1/apps',
  includeOrgHeader: true,
  fetchFn: appApi.get,
  updateFn: appApi.update,
  deleteFn: appApi.delete,
})

const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const appTitle = computed(() => String(formData.value.client_name || item.value?.name || 'Application'))
const redirectUris = computed(() => Array.isArray(item.value?.redirect_uris) ? item.value?.redirect_uris : [])
const postLogoutUris = computed(() => Array.isArray(item.value?.post_logout_redirect_uris) ? item.value?.post_logout_redirect_uris : [])
const grantTypes = computed(() => Array.isArray(item.value?.grant_types) ? item.value?.grant_types : [])
const responseTypes = computed(() => Array.isArray(item.value?.response_types) ? item.value?.response_types : [])
const redirectUriSummary = computed(() =>
  redirectUris.value.length ? `${redirectUris.value.length} configured` : 'No redirect URIs configured',
)
const postLogoutSummary = computed(() =>
  postLogoutUris.value.length ? `${postLogoutUris.value.length} configured` : 'No post logout URIs configured',
)
const badges = computed(() => ([
  { label: item.value?.state || 'active', variant: 'outline' as const },
  { label: item.value?.app_type || 'app', variant: 'secondary' as const },
  ...(currentOrgId.value ? [{ label: currentOrgId.value, variant: 'secondary' as const }] : []),
]))
const stateRows = computed<SummaryFact[]>(() => ([
  { label: 'Created', value: formatDateTime(item.value?.created_at || '') },
  { label: 'Updated', value: formatDateTime(item.value?.updated_at || '') },
  { label: 'Redirect URIs', value: `${redirectUris.value.length}` },
]))
const protocolRows = computed<SummaryFact[]>(() => ([
  { label: 'Application type', value: item.value?.app_type || 'app' },
  { label: 'Grant types', value: grantTypes.value.length ? grantTypes.value.join(', ') : 'None configured' },
  { label: 'Response types', value: responseTypes.value.length ? responseTypes.value.join(', ') : 'None configured' },
  { label: 'Client ID', value: item.value?.client_id || 'Not issued yet' },
]))
</script>
