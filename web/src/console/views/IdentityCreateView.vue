<template>
  <ResourceCreateView
    :singular-title="schemaLabel"
    :back-route="backRoute"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    @submit="submit"
  >
    <!-- Access setup -->
    <div v-if="showActivationCard" class="space-y-2">
      <Input
        v-if="supportsPassword"
        id="initial-password"
        v-model="initialPassword"
        type="password"
        placeholder="Initial password (optional)"
      />
      <div v-if="supportsInvite" class="flex items-center gap-2 py-0.5">
        <Checkbox
          id="send-invite"
          :model-value="sendInvite"
          @update:model-value="(value) => (sendInvite = Boolean(value))"
        />
        <Label for="send-invite" class="text-sm font-normal leading-none">{{ inviteLabel }}</Label>
      </div>
    </div>
  </ResourceCreateView>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { magicLinkApi, userApi } from '@/api/resources'
import ResourceCreateView from '@/console/components/ResourceCreateView.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  formatFieldLabel,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import {
  buildUserDetailRoute,
  buildUserListRoute,
  getUserSchemaLabel,
  isUserSchemaType,
} from '@/console/utils/user-routes'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'

const props = defineProps<{ schemaType?: string }>()

const router = useRouter()
const { currentOrgId } = useOrgContext()

const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: '',
  versions: [],
})
const formData = ref<Record<string, any>>({})
const jsonValid = ref(true)
const submitting = ref(false)
const error = ref('')
const initialPassword = ref('')
const sendInvite = ref(true)

const dedicatedRoutes: Record<string, string> = {
  app: '/applications/new',
  org: '/orgs/new',
  group: '/groups/new',
  project: '/projects/new',
}

const resolvedSchemaType = computed(() => props.schemaType || 'human_user')
const isUserSchema = computed(() => isUserSchemaType(resolvedSchemaType.value))
const schemaLabel = computed(() => {
  if (isUserSchema.value) {
    return getUserSchemaLabel(resolvedSchemaType.value)
  }
  return String(
    schemaContext.value.display.singular ||
      formatFieldLabel(resolvedSchemaType.value.replace(/_/g, ' ')),
  )
})
const backRoute = computed(() =>
  isUserSchema.value ? buildUserListRoute() : `/s/${resolvedSchemaType.value}`,
)
const payload = computed(() =>
  buildResourceWriteBody(
    'user',
    schemaContext.value.schemaId,
    normalizeResourceData(formData.value),
  ),
)
const curlSnippets = computed(() =>
  buildCurlSnippets({
    path: '/v1/users',
    body: payload.value,
    includeOrgHeader: true,
    orgId: currentOrgId.value,
    methods: ['POST'],
  }),
)
const authMethods = computed<Record<string, { enabled?: boolean; interactive?: boolean }>>(
  () =>
    (schemaContext.value.schema?.['x-auth-methods'] as Record<
      string,
      { enabled?: boolean; interactive?: boolean }
    >) || {},
)
const supportsPassword = computed(
  () => authMethods.value.password?.enabled !== false && Boolean(authMethods.value.password),
)
const supportsInvite = computed(() =>
  Object.values(authMethods.value).some(
    (method) => method?.interactive !== false && method?.enabled !== false,
  ),
)
const showActivationCard = computed(() => supportsPassword.value || supportsInvite.value)
const inviteLabel = computed(() =>
  resolvedSchemaType.value === 'human_user'
    ? 'Send invite link after creation'
    : 'Send sign-in link after creation',
)
const inviteDescription = computed(() =>
  resolvedSchemaType.value === 'human_user'
    ? 'Uses the identifier email to send a registration or sign-in link.'
    : 'Uses the identifier email when the selected auth methods support interactive sign-in.',
)

async function loadSchema() {
  if (dedicatedRoutes[resolvedSchemaType.value]) {
    router.replace(dedicatedRoutes[resolvedSchemaType.value])
    return
  }

  error.value = ''
  try {
    schemaContext.value = await loadResourceSchemaContext(resolvedSchemaType.value)
    formData.value = {}
  } catch (err: any) {
    error.value = err?.message || 'Failed to load schema'
  }
}

async function submit() {
  submitting.value = true
  try {
    const created = await userApi.create(payload.value)
    if (initialPassword.value.trim()) {
      await userApi.setPassword(created.id, initialPassword.value.trim())
    }
    if (sendInvite.value && String(payload.value.identifier || '').includes('@')) {
      await magicLinkApi.send(String(payload.value.identifier))
      notifyMutationSuccess(schemaLabel.value, 'create', 'Sign-in link sent')
    } else {
      notifyMutationSuccess(schemaLabel.value, 'create')
    }
    router.push(
      isUserSchema.value
        ? buildUserDetailRoute(created.id)
        : `/s/${resolvedSchemaType.value}/${created.id}`,
    )
  } catch (err: any) {
    notifyMutationError(schemaLabel.value, 'create', err)
  } finally {
    submitting.value = false
  }
}

onMounted(loadSchema)
watch(() => resolvedSchemaType.value, loadSchema)
</script>
