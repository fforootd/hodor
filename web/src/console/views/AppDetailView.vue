<template>
  <ResourceDetailView
    v-model:form-data="formData"
    :resource="app"
    resource-type="app"
    singular-title="Application"
    back-route="/applications"
    :display-title="app?.name || 'Application'"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :saving="saving"
    :deleting="deleting"
    :load-error="loadError"
    :json-valid="jsonValid"
    :show-avatar="true"
    @save="save"
    @delete="deleteApp"
    @update:json-valid="(v) => jsonValid = v"
  >
    <template #header-badges>
      <code class="rounded bg-muted px-1.5 py-0.5 text-xs">{{ app?.client_id }}</code>
      <StateBadge :state="app?.state" />
    </template>
  </ResourceDetailView>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { appApi, type App } from '@/api/resources'
import ResourceDetailView from '@/console/components/ResourceDetailView.vue'
import { StateBadge } from '@/components/ui/state-badge'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets, buildResourceWriteBody,
  loadResourceSchemaContext, normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'

const route = useRoute()
const router = useRouter()
const { currentOrgId } = useOrgContext()

const app = ref<App | null>(null)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {}, schema: null, schemaId: '', schemaType: 'app', versions: [],
})
const saving = ref(false)
const deleting = ref(false)
const showDeleteConfirm = ref(false)
const jsonValid = ref(true)
const loadError = ref('')

const payload = computed(() => buildResourceWriteBody('app', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/apps/${encodeURIComponent(String(route.params.id || ''))}`, body: payload.value,
  includeOrgHeader: true, orgId: currentOrgId.value, methods: ['GET', 'PATCH'],
}))

onMounted(async () => {
  try {
    app.value = await appApi.get(route.params.id as string)
    formData.value = normalizeResourceData(app.value.data || {})
    schemaContext.value = await loadResourceSchemaContext(app.value.schema_type || 'app', app.value.schema_id || '')
  } catch (err: any) { loadError.value = err?.message || 'Failed to load application' }
})

async function save() {
  if (!app.value) return
  saving.value = true
  try {
    app.value = await appApi.update(app.value.id, payload.value)
    formData.value = normalizeResourceData(app.value.data || {})
    notifyMutationSuccess('Application', 'update')
  } catch (err: any) { notifyMutationError('Application', 'update', err) }
  finally { saving.value = false }
}

async function deleteApp() {
  if (!app.value) return
  deleting.value = true
  try {
    await appApi.delete(app.value.id)
    notifyMutationSuccess('Application', 'delete')
    router.push('/applications')
  } catch (err: any) { notifyMutationError('Application', 'delete', err) }
  finally { deleting.value = false }
}
</script>
