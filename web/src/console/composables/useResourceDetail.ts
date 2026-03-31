import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  collectSummaryFacts,
  loadResourceSchemaContext,
  normalizeResourceData,
  stringifyResourceData,
  type ResourceSchemaContext,
  type SchemaResourceType,
} from '@/console/utils/schema-resource'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'

export interface UseResourceMemberOptions<TMember> {
  list: (id: string) => Promise<TMember[]>
  add: (id: string, userId: string) => Promise<unknown>
  remove: (id: string, userId: string) => Promise<void>
  label: string
}

export interface UseResourceDetailOptions<TResource extends Record<string, any>, TMember> {
  resourceType: SchemaResourceType
  resourceName: string
  listRoute: string
  apiPath: string
  includeOrgHeader?: boolean
  fetchFn: (id: string) => Promise<TResource>
  updateFn: (id: string, data: Record<string, any>) => Promise<TResource>
  deleteFn: (id: string) => Promise<void>
  members?: UseResourceMemberOptions<TMember>
}

export function useResourceDetail<
  TResource extends Record<string, any>,
  TMember extends { user_id: string } = never,
>(options: UseResourceDetailOptions<TResource, TMember>) {
  const route = useRoute()
  const router = useRouter()
  const { currentOrgId } = useOrgContext()

  const item = ref<TResource | null>(null)
  const members = ref<TMember[]>([])
  const formData = ref<Record<string, any>>({})
  const schemaContext = ref<ResourceSchemaContext>({
    display: {},
    schema: null,
    schemaId: '',
    schemaType: options.resourceType,
    versions: [],
  })
  const loading = ref(false)
  const saving = ref(false)
  const deleting = ref(false)
  const jsonValid = ref(true)
  const jsonContent = ref('{}')
  const jsonError = ref('')
  const loadError = ref('')

  const resourceId = computed(() => String(route.params.id || ''))
  const payload = computed(() =>
    buildResourceWriteBody(
      options.resourceType,
      schemaContext.value.schemaId,
      normalizeResourceData(formData.value),
    ),
  )
  const curlSnippets = computed(() =>
    buildCurlSnippets({
      path: `${options.apiPath}/${encodeURIComponent(resourceId.value)}`,
      body: payload.value,
      includeOrgHeader: options.includeOrgHeader ?? false,
      orgId: currentOrgId.value,
      methods: ['GET', 'PATCH'],
    }),
  )
  const summaryFacts = computed(() =>
    collectSummaryFacts(formData.value, schemaContext.value.schema, { limit: 6 }),
  )
  const overviewFacts = computed(() =>
    collectSummaryFacts(formData.value, schemaContext.value.schema),
  )

  watch(formData, (value) => {
    const next = stringifyResourceData(normalizeResourceData(value))
    if (next !== jsonContent.value) {
      jsonContent.value = next
    }
  }, { deep: true, immediate: true })

  async function load() {
    if (!resourceId.value) return
    loading.value = true
    loadError.value = ''
    try {
      const resource = await options.fetchFn(resourceId.value)
      item.value = resource
      formData.value = normalizeResourceData(resource.data || {})

      const [schemaResult, membersResult] = await Promise.allSettled([
        loadResourceSchemaContext(resource.schema_type || options.resourceType, resource.schema_id || ''),
        options.members ? options.members.list(resourceId.value) : Promise.resolve([] as TMember[]),
      ])

      if (schemaResult.status === 'fulfilled') {
        schemaContext.value = schemaResult.value
      }
      if (membersResult.status === 'fulfilled') {
        members.value = membersResult.value
      }
    } catch (error: any) {
      loadError.value = error?.message || `Failed to load ${options.resourceName}`
    } finally {
      loading.value = false
    }
  }

  async function save() {
    if (!item.value) return
    saving.value = true
    try {
      item.value = await options.updateFn(item.value.id, payload.value)
      formData.value = normalizeResourceData(item.value.data || {})
      notifyMutationSuccess(options.resourceName, 'update')
    } catch (error: any) {
      notifyMutationError(options.resourceName, 'update', error)
    } finally {
      saving.value = false
    }
  }

  async function deleteResource() {
    if (!item.value) return
    deleting.value = true
    try {
      await options.deleteFn(item.value.id)
      notifyMutationSuccess(options.resourceName, 'delete')
      router.push(options.listRoute)
    } catch (error: any) {
      notifyMutationError(options.resourceName, 'delete', error)
    } finally {
      deleting.value = false
    }
  }

  async function addMember(userId: string) {
    if (!item.value || !options.members) return
    try {
      await options.members.add(item.value.id, userId)
      members.value = await options.members.list(item.value.id)
      notifyMutationSuccess(options.members.label, 'add')
    } catch (error: any) {
      notifyMutationError(options.members.label, 'add', error)
    }
  }

  async function removeMember(userId: string) {
    if (!item.value || !options.members) return
    try {
      await options.members.remove(item.value.id, userId)
      members.value = await options.members.list(item.value.id)
      notifyMutationSuccess(options.members.label, 'remove')
    } catch (error: any) {
      notifyMutationError(options.members.label, 'remove', error)
    }
  }

  function onJsonValid(parsed: Record<string, any>) {
    jsonError.value = ''
    jsonValid.value = true
    formData.value = normalizeResourceData(parsed)
  }

  function onJsonError(message: string) {
    jsonError.value = message
    jsonValid.value = false
  }

  onMounted(load)
  watch(resourceId, load)

  return {
    item,
    members,
    formData,
    schemaContext,
    loading,
    saving,
    deleting,
    jsonValid,
    jsonContent,
    jsonError,
    loadError,
    resourceId,
    payload,
    curlSnippets,
    summaryFacts,
    overviewFacts,
    load,
    save,
    deleteResource,
    addMember,
    removeMember,
    onJsonValid,
    onJsonError,
  }
}
