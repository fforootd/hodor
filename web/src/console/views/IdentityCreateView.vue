<template>
  <div class="space-y-6 pb-10">
    <section class="sticky top-0 z-10 rounded-3xl border bg-background/95 p-6 shadow-sm backdrop-blur">
      <div class="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
        <div class="flex items-start gap-4">
          <Button variant="ghost" size="icon" as-child class="mt-1 shrink-0">
            <RouterLink :to="backRoute" aria-label="Back to users">
              <ArrowLeft class="size-4" />
            </RouterLink>
          </Button>

          <div class="min-w-0 space-y-3">
            <div class="space-y-1">
              <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                Identity creation
              </p>
              <h1 class="truncate text-3xl font-semibold tracking-tight">Create {{ schemaLabel }}</h1>
              <p class="max-w-2xl text-sm text-muted-foreground">
                Start with the profile, configure access only when needed, and keep JSON/cURL in a separate developer mode.
              </p>
            </div>

            <div class="flex flex-wrap items-center gap-2">
              <Badge variant="outline" class="text-xs">{{ schemaLabel }}</Badge>
              <Badge variant="secondary" class="text-xs">{{ resolvedSchemaType }}</Badge>
              <Badge v-if="identifierPreview" variant="secondary" class="text-xs">{{ identifierPreview }}</Badge>
            </div>
          </div>
        </div>

        <div class="flex flex-wrap gap-2 xl:justify-end">
          <Button variant="outline" as-child>
            <RouterLink :to="backRoute">Cancel</RouterLink>
          </Button>
          <Button :disabled="submitting || !jsonValid" data-testid="create-user" @click="submit">
            {{ submitting ? 'Creating…' : `Create ${schemaLabel}` }}
          </Button>
        </div>
      </div>
    </section>

    <FormError :error="error" />

    <Tabs v-model="activeTab" class="space-y-6">
      <TabsList class="grid w-full grid-cols-2 gap-2 rounded-2xl bg-muted/50 p-1 md:grid-cols-4">
        <TabsTrigger value="profile">Profile</TabsTrigger>
        <TabsTrigger value="access">Access Setup</TabsTrigger>
        <TabsTrigger value="review">Review & Create</TabsTrigger>
        <TabsTrigger value="api">API</TabsTrigger>
      </TabsList>

      <TabsContent value="profile" class="space-y-6">
        <div class="grid gap-6 xl:grid-cols-[1.35fr_minmax(0,0.95fr)]">
          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Profile</CardTitle>
              <p class="text-sm text-muted-foreground">
                Capture the identity fields that matter operationally first.
              </p>
            </CardHeader>
            <CardContent class="space-y-5">
              <section v-if="needsExplicitIdentifier" class="space-y-2">
                <div class="flex items-center gap-1.5">
                  <Label for="identity-identifier" class="text-[11px] font-medium text-muted-foreground">
                    {{ identifierLabel }}<span class="text-destructive">*</span>
                  </Label>
                  <Badge variant="outline" class="h-4 px-1 text-[9px] uppercase tracking-wide">Identifier</Badge>
                </div>
                <Input
                  id="identity-identifier"
                  v-model="formData.identifier"
                  :placeholder="identifierPlaceholder"
                  autocomplete="off"
                />
                <p class="text-xs text-muted-foreground">
                  {{ identifierDescription }}
                </p>
              </section>

              <Separator v-if="needsExplicitIdentifier" />

              <SchemaFieldEditor
                v-if="schemaContext.schema"
                v-model="formData"
                :fields="schemaFields"
              />
              <div v-else class="flex items-center gap-2 text-sm text-muted-foreground">
                <Spinner class="size-4" />
                Loading schema…
              </div>
            </CardContent>
          </Card>

          <div class="space-y-6">
            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <CardTitle class="text-sm">Operator Summary</CardTitle>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                  <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Identifier preview</p>
                  <p class="mt-1 text-sm font-medium">{{ identifierPreview || emptyIdentifierHint }}</p>
                </div>
                <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                  <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Auth posture</p>
                  <p class="mt-1 text-sm font-medium">{{ securitySummary }}</p>
                </div>
                <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                  <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Profile facts</p>
                  <div v-if="summaryFacts.length" class="mt-2 space-y-2">
                    <div v-for="fact in summaryFacts" :key="fact.label" class="flex items-center justify-between gap-4 text-sm">
                      <span class="text-muted-foreground">{{ fact.label }}</span>
                      <span class="text-right font-medium">{{ fact.value }}</span>
                    </div>
                  </div>
                  <p v-else class="mt-1 text-sm text-muted-foreground">Profile details will appear as you fill the form.</p>
                </div>
              </CardContent>
            </Card>

            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <CardTitle class="text-sm">Supported Login Methods</CardTitle>
              </CardHeader>
              <CardContent>
                <div v-if="authMethodItems.length" class="flex flex-wrap gap-2">
                  <Badge
                    v-for="method in authMethodItems"
                    :key="method.name"
                    variant="outline"
                    class="gap-1.5 border-dashed text-xs"
                    :class="authMethodBadgeClass(method)"
                  >
                    <span>{{ method.label }}</span>
                    <span class="text-[10px] uppercase tracking-wider opacity-75">
                      {{ method.enabled ? (method.interactive ? 'interactive' : 'service') : 'disabled' }}
                    </span>
                  </Badge>
                </div>
                <p v-else class="text-sm text-muted-foreground">No schema-defined auth methods.</p>
              </CardContent>
            </Card>
          </div>
        </div>
      </TabsContent>

      <TabsContent value="access" class="space-y-6">
        <div class="grid gap-6 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Access Setup</CardTitle>
              <p class="text-sm text-muted-foreground">
                Configure onboarding only when the selected schema supports it.
              </p>
            </CardHeader>
            <CardContent class="space-y-5">
              <section class="space-y-3">
                <div class="flex items-center gap-2">
                  <KeyRound class="size-4 text-muted-foreground" />
                  <h2 class="text-sm font-semibold">Password</h2>
                </div>
                <div v-if="supportsPassword" class="space-y-2">
                  <Input
                    id="initial-password"
                    v-model="initialPassword"
                    type="password"
                    placeholder="Initial password (optional)"
                  />
                  <p class="text-xs text-muted-foreground">
                    Leave blank to create the identity without setting a password immediately.
                  </p>
                </div>
                <p v-else class="text-sm text-muted-foreground">This schema does not expose password setup.</p>
              </section>

              <Separator />

              <section class="space-y-3">
                <div class="flex items-center gap-2">
                  <Mail class="size-4 text-muted-foreground" />
                  <h2 class="text-sm font-semibold">Invite / sign-in message</h2>
                </div>
                <div v-if="supportsInvite" class="space-y-3">
                  <div class="flex items-center gap-2 py-0.5">
                    <Checkbox
                      id="send-invite"
                      :model-value="sendInvite"
                      @update:model-value="(value) => (sendInvite = Boolean(value))"
                    />
                    <Label for="send-invite" class="text-sm font-normal leading-none">{{ inviteLabel }}</Label>
                  </div>
                  <p class="text-xs text-muted-foreground">
                    {{ inviteDescription }}
                  </p>
                </div>
                <p v-else class="text-sm text-muted-foreground">Interactive sign-in is not enabled for this schema.</p>
              </section>
            </CardContent>
          </Card>

          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Access Summary</CardTitle>
            </CardHeader>
            <CardContent class="space-y-4">
              <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Creation outcome</p>
                <p class="mt-1 text-sm font-medium">{{ creationOutcome }}</p>
              </div>
              <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Invite behavior</p>
                <p class="mt-1 text-sm font-medium">{{ inviteOutcome }}</p>
              </div>
              <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Next step</p>
                <p class="mt-1 text-sm font-medium">
                  Review the payload before creating if you want to sanity-check the final identity contract.
                </p>
              </div>
            </CardContent>
          </Card>
        </div>
      </TabsContent>

      <TabsContent value="review" class="space-y-6">
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)]">
          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Review</CardTitle>
              <p class="text-sm text-muted-foreground">
                Confirm the identity data and onboarding choices before creating it.
              </p>
            </CardHeader>
            <CardContent class="space-y-5">
              <section class="space-y-3">
                <h2 class="text-sm font-semibold">Profile values</h2>
                <div v-if="reviewRows.length" class="space-y-2">
                  <div
                    v-for="row in reviewRows"
                    :key="row.label"
                    class="flex items-center justify-between gap-4 rounded-2xl border bg-muted/20 px-4 py-3 text-sm"
                  >
                    <span class="text-muted-foreground">{{ row.label }}</span>
                    <span class="text-right font-medium">{{ row.value }}</span>
                  </div>
                </div>
                <p v-else class="text-sm text-muted-foreground">No values captured yet.</p>
              </section>

              <Separator />

              <section class="space-y-3">
                <h2 class="text-sm font-semibold">Activation choices</h2>
                <div class="space-y-2">
                  <div class="flex items-center justify-between gap-4 rounded-2xl border bg-muted/20 px-4 py-3 text-sm">
                    <span class="text-muted-foreground">Password preset</span>
                    <span class="text-right font-medium">{{ initialPassword ? 'Yes' : 'No' }}</span>
                  </div>
                  <div class="flex items-center justify-between gap-4 rounded-2xl border bg-muted/20 px-4 py-3 text-sm">
                    <span class="text-muted-foreground">Invite / sign-in link</span>
                    <span class="text-right font-medium">{{ sendInvite && supportsInvite ? 'Send after creation' : 'Do not send' }}</span>
                  </div>
                </div>
              </section>
            </CardContent>
          </Card>

          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Create {{ schemaLabel }}</CardTitle>
            </CardHeader>
            <CardContent class="space-y-4">
              <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Identifier</p>
                <p class="mt-1 text-sm font-medium">{{ identifierPreview || 'Missing identifier' }}</p>
              </div>
              <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Schema</p>
                <p class="mt-1 text-sm font-medium">{{ schemaLabel }}</p>
              </div>
              <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Developer payload</p>
                <p class="mt-1 text-sm font-medium">{{ payloadSummary }}</p>
              </div>
              <Button class="w-full" :disabled="submitting || !jsonValid" data-testid="review-create-user" @click="submit">
                {{ submitting ? 'Creating…' : `Create ${schemaLabel}` }}
              </Button>
            </CardContent>
          </Card>
        </div>
      </TabsContent>

      <TabsContent value="api" class="space-y-6">
        <Card class="rounded-3xl shadow-sm">
          <CardHeader class="pb-3">
            <CardTitle class="text-lg">Developer Mode</CardTitle>
            <p class="text-sm text-muted-foreground">
              JSON and cURL stay out of the main creation flow, but remain available for deeper inspection.
            </p>
          </CardHeader>
          <CardContent class="space-y-6">
            <JsonEditor
              v-model="jsonContent"
              label="Canonical JSON"
              :schema="schemaContext.schema || undefined"
              height="420px"
              @valid="onJsonValid"
              @error="onJsonError"
            />
            <p v-if="jsonError" class="text-xs text-destructive">{{ jsonError }}</p>
            <CurlSnippetPanel :snippets="curlSnippets" />
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import { magicLinkApi, userApi } from '@/api/resources'
import CurlSnippetPanel from '@/console/components/CurlSnippetPanel.vue'
import JsonEditor from '@/console/components/JsonEditor.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  extractSchemaFields,
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
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { FormError } from '@/components/ui/form-error'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Spinner } from '@/components/ui/spinner'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'
import { ArrowLeft, KeyRound, Mail } from 'lucide-vue-next'

const props = defineProps<{ schemaType?: string }>()

interface DisplayFact {
  label: string
  value: string
}

interface AuthMethodDisplay {
  enabled: boolean
  interactive: boolean
  label: string
  name: string
  position: number
}

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
const jsonContent = ref('{}')
const jsonError = ref('')
const jsonValid = ref(true)
const submitting = ref(false)
const error = ref('')
const initialPassword = ref('')
const sendInvite = ref(true)
const activeTab = ref('profile')

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
const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const identifierFields = computed(() => schemaFields.value.filter((field) => field.identifier))
const needsExplicitIdentifier = computed(() => identifierFields.value.length === 0)
const authMethods = computed<Record<string, { enabled?: boolean; interactive?: boolean; position?: number }>>(
  () =>
    (schemaContext.value.schema?.['x-auth-methods'] as Record<
      string,
      { enabled?: boolean; interactive?: boolean; position?: number }
    >) || {},
)
const authMethodItems = computed<AuthMethodDisplay[]>(() =>
  Object.entries(authMethods.value)
    .map(([name, config]) => ({
      enabled: config?.enabled !== false,
      interactive: config?.interactive !== false,
      label: authMethodLabel(name),
      name,
      position: typeof config?.position === 'number' ? config.position : Number.MAX_SAFE_INTEGER,
    }))
    .sort((left, right) => left.position - right.position || left.label.localeCompare(right.label)),
)
const enabledAuthMethodItems = computed(() => authMethodItems.value.filter((method) => method.enabled))
const supportsPassword = computed(
  () => authMethods.value.password?.enabled !== false && Boolean(authMethods.value.password),
)
const supportsInvite = computed(() =>
  Object.values(authMethods.value).some(
    (method) => method?.interactive !== false && method?.enabled !== false,
  ),
)
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
const identifierPreview = computed(() =>
  String(
    formData.value.email
    || formData.value.username
    || formData.value.phone
    || formData.value.identifier
    || payload.value.identifier
    || '',
  ),
)
const identifierLabel = computed(() =>
  resolvedSchemaType.value === 'ai_agent' ? 'Agent Identifier' : 'Identifier',
)
const identifierPlaceholder = computed(() =>
  resolvedSchemaType.value === 'ai_agent'
    ? 'fraud-agent-prod'
    : 'service-sync-prod',
)
const identifierDescription = computed(() =>
  resolvedSchemaType.value === 'ai_agent'
    ? 'Use a stable machine-readable identifier for this agent.'
    : 'Use a stable machine-readable identifier for this service account.',
)
const emptyIdentifierHint = computed(() =>
  needsExplicitIdentifier.value
    ? `Set a ${identifierLabel.value.toLowerCase()}`
    : 'Set an email, username, or phone identifier',
)
const summaryFacts = computed(() => collectSummaryFacts(formData.value, schemaContext.value.schema).slice(0, 6))
const reviewRows = computed(() => collectSummaryFacts(formData.value, schemaContext.value.schema))
const securitySummary = computed(() => {
  if (!enabledAuthMethodItems.value.length) return 'No enabled authentication methods'
  return enabledAuthMethodItems.value.map((method) => method.label).join(', ')
})
const creationOutcome = computed(() => {
  if (initialPassword.value.trim()) {
    return 'User will be created with an initial password'
  }
  return 'User will be created without a preset password'
})
const inviteOutcome = computed(() => {
  if (sendInvite.value && supportsInvite.value && identifierPreview.value.includes('@')) {
    return inviteLabel.value
  }
  if (sendInvite.value && supportsInvite.value) {
    return 'Invite requested, but an email-style identifier is still needed'
  }
  return 'No invite or sign-in link will be sent'
})
const payloadSummary = computed(() => `${Object.keys(payload.value.data || {}).length} schema fields in payload`)

watch(formData, (value) => {
  const next = JSON.stringify(normalizeResourceData(value), null, 2)
  if (next !== jsonContent.value) {
    jsonContent.value = next
  }
}, { deep: true, immediate: true })

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
  if (!String(payload.value.identifier || '').trim()) {
    error.value = `${identifierLabel.value} is required`
    activeTab.value = 'profile'
    return
  }

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

function authMethodBadgeClass(method: AuthMethodDisplay): string {
  if (!method.enabled) return 'border-muted-foreground/20 text-muted-foreground'
  return method.interactive
    ? 'border-emerald-200 text-emerald-700'
    : 'border-blue-200 text-blue-700'
}

function authMethodLabel(name: string): string {
  const labels: Record<string, string> = {
    api_key: 'API Key',
    magic_link: 'Magic Link',
    pat: 'PAT',
    sso: 'SSO',
  }

  return labels[name] || formatFieldLabel(name)
}

function collectSummaryFacts(
  data: Record<string, any>,
  schema: Record<string, any> | null,
): DisplayFact[] {
  const fields = extractSchemaFields(schema)
  const facts: DisplayFact[] = []

  const visitField = (field: ReturnType<typeof extractSchemaFields>[number]) => {
    if (field.hidden || field.sensitive || facts.length >= 12) return
    if (['avatar_url', 'display_name', 'metadata'].includes(field.name)) return

    const value = getValueAtPath(data, field.path)
    if (value == null || value === '') {
      if (field.properties?.length) field.properties.forEach(visitField)
      return
    }

    if (field.type === 'object') {
      field.properties?.forEach(visitField)
      return
    }

    const displayValue = formatFactValue(value)
    if (!displayValue) return
    facts.push({ label: field.label, value: displayValue })
  }

  fields.forEach(visitField)
  return facts
}

function formatFactValue(value: unknown): string {
  if (value == null || value === '') return ''
  if (typeof value === 'boolean') return value ? 'Yes' : 'No'
  if (Array.isArray(value)) {
    return value
      .map((item) => (typeof item === 'object' ? '' : String(item)))
      .filter(Boolean)
      .join(', ')
  }
  if (typeof value === 'object') return ''
  return String(value)
}

function getValueAtPath(source: Record<string, any>, path: string): unknown {
  return path.split('.').reduce<unknown>((current, segment) => {
    if (!current || typeof current !== 'object') return undefined
    return (current as Record<string, any>)[segment]
  }, source)
}

onMounted(loadSchema)
watch(() => resolvedSchemaType.value, loadSchema)
</script>
