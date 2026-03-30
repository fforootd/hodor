<template>
  <div class="space-y-6">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link :to="backRoute"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Create {{ schemaLabel }}</h1>
        <p class="text-sm text-muted-foreground">Define fields in the form, inspect canonical JSON, or copy the API request.</p>
      </div>
    </div>

    <SchemaTabsEditor
      v-if="schemaContext.schema"
      v-model="formData"
      :schema="schemaContext.schema"
      :curl-snippets="curlSnippets"
      :form-title="`${schemaLabel} Fields`"
      @update:json-valid="(value) => jsonValid = value"
    />

    <Card v-else>
      <CardContent class="pt-6 text-sm text-muted-foreground">Loading schema…</CardContent>
    </Card>

    <Card v-if="showActivationCard">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Activation</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div v-if="supportsPassword" class="space-y-2">
          <Label for="initial-password">Initial Password</Label>
          <Input id="initial-password" v-model="initialPassword" type="password" placeholder="Optional initial password" />
        </div>
        <div v-if="supportsInvite" class="flex items-start gap-3 rounded-lg border bg-muted/30 p-3">
          <Checkbox
            id="send-invite"
            :model-value="sendInvite"
            @update:model-value="(value) => sendInvite = Boolean(value)"
          />
          <div class="space-y-1">
            <Label for="send-invite" class="text-sm font-medium">Send invite link after creation</Label>
            <p class="text-xs text-muted-foreground">
              Uses the identifier email to send a registration or sign-in link.
            </p>
          </div>
        </div>
      </CardContent>
    </Card>

    <div class="flex justify-end gap-3">
      <Button variant="outline" as-child>
        <router-link :to="backRoute">Cancel</router-link>
      </Button>
      <Button :disabled="submitting || !jsonValid" @click="submit">
        {{ submitting ? 'Creating…' : `Create ${schemaLabel}` }}
      </Button>
    </div>

    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      {{ error }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { magicLinkApi, userApi } from '@/api/resources'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  formatFieldLabel,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ArrowLeft } from 'lucide-vue-next'

const props = defineProps<{ schemaType: string }>()

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

const schemaLabel = computed(() =>
  String(schemaContext.value.display.singular || formatFieldLabel(props.schemaType.replace(/_/g, ' '))),
)
const backRoute = computed(() => `/s/${props.schemaType}`)
const payload = computed(() => buildResourceWriteBody('user', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: '/v1/users',
  body: payload.value,
  includeOrgHeader: true,
  orgId: currentOrgId.value,
  methods: ['POST'],
}))
const authMethods = computed<Record<string, { enabled?: boolean; interactive?: boolean }>>(
  () => (schemaContext.value.schema?.['x-auth-methods'] as Record<string, { enabled?: boolean; interactive?: boolean }>) || {},
)
const supportsPassword = computed(() => authMethods.value.password?.enabled !== false && Boolean(authMethods.value.password))
const supportsInvite = computed(() =>
  Object.values(authMethods.value).some((method) => method?.interactive !== false && method?.enabled !== false),
)
const showActivationCard = computed(() => supportsPassword.value || supportsInvite.value)

async function loadSchema() {
  if (dedicatedRoutes[props.schemaType]) {
    router.replace(dedicatedRoutes[props.schemaType])
    return
  }
  error.value = ''
  try {
    schemaContext.value = await loadResourceSchemaContext(props.schemaType)
    formData.value = {}
  } catch (err: any) {
    error.value = err?.message || 'Failed to load schema'
  }
}

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const created = await userApi.create(payload.value)
    if (initialPassword.value.trim()) {
      await userApi.setPassword(created.id, initialPassword.value.trim())
    }
    if (sendInvite.value && String(payload.value.identifier || '').includes('@')) {
      await magicLinkApi.send(String(payload.value.identifier))
    }
    router.push(`/s/${props.schemaType}/${created.id}`)
  } catch (err: any) {
    error.value = err?.message || `Failed to create ${schemaLabel.value.toLowerCase()}`
  } finally {
    submitting.value = false
  }
}

onMounted(loadSchema)
watch(() => props.schemaType, loadSchema)
</script>
