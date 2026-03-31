/**
 * Generic composable for resource creation views.
 *
 * Handles schema loading, form state, payload building,
 * cURL snippet generation, and submit with notifications.
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
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

export interface UseResourceCreateOptions {
  /** Schema type to load (e.g. 'project', 'group', 'org', 'app') */
  schemaType: SchemaResourceType
  /** API path for cURL snippets (e.g. '/v1/projects') */
  apiPath: string
  /** Human-readable resource name for notifications */
  resourceName: string
  /** Route to navigate back to on cancel or after creation */
  listRoute: string
  /** API create function */
  createFn: (payload: any) => Promise<{ id: string }>
  /** Include X-Org-Id header in cURL snippets */
  includeOrgHeader?: boolean
  /** Default form data values */
  defaultFormData?: Record<string, any>
  /** Custom detail route builder (defaults to `${listRoute}/${id}`) */
  detailRoute?: (id: string) => string
}

export function useResourceCreate(options: UseResourceCreateOptions) {
  const router = useRouter()
  const { currentOrgId } = useOrgContext()

  const submitting = ref(false)
  const error = ref('')
  const jsonValid = ref(true)
  const jsonContent = ref('{}')
  const jsonError = ref('')
  const formData = ref<Record<string, any>>({ ...options.defaultFormData })
  const schemaContext = ref<ResourceSchemaContext>({
    display: {},
    schema: null,
    schemaId: '',
    schemaType: options.schemaType,
    versions: [],
  })

  const payload = computed(() =>
    buildResourceWriteBody(
      options.schemaType,
      schemaContext.value.schemaId,
      normalizeResourceData(formData.value),
    ),
  )

  const curlSnippets = computed(() =>
    buildCurlSnippets({
      path: options.apiPath,
      body: payload.value,
      includeOrgHeader: options.includeOrgHeader ?? false,
      orgId: currentOrgId.value,
      methods: ['POST'],
    }),
  )

  const summaryFacts = computed(() =>
    collectSummaryFacts(formData.value, schemaContext.value.schema, { limit: 6 }),
  )
  const reviewFacts = computed(() =>
    collectSummaryFacts(formData.value, schemaContext.value.schema),
  )

  watch(formData, (value) => {
    const next = stringifyResourceData(normalizeResourceData(value))
    if (next !== jsonContent.value) {
      jsonContent.value = next
    }
  }, { deep: true, immediate: true })

  async function loadSchema() {
    schemaContext.value = await loadResourceSchemaContext(options.schemaType)
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

  async function submit() {
    submitting.value = true
    error.value = ''
    try {
      const created = await options.createFn(payload.value)
      notifyMutationSuccess(options.resourceName, 'create')
      const route = options.detailRoute
        ? options.detailRoute(created.id)
        : `${options.listRoute}/${created.id}`
      router.push(route)
    } catch (err: any) {
      error.value = err?.message || `Failed to create ${options.resourceName}`
      notifyMutationError(options.resourceName, 'create', err)
    } finally {
      submitting.value = false
    }
  }

  onMounted(loadSchema)

  return {
    schemaContext,
    formData,
    jsonValid,
    jsonContent,
    jsonError,
    submitting,
    error,
    payload,
    curlSnippets,
    summaryFacts,
    reviewFacts,
    submit,
    loadSchema,
    onJsonValid,
    onJsonError,
  }
}
