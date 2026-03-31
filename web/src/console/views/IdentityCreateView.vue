<template>
  <div class="space-y-6">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link :to="backRoute"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Create {{ schemaLabel }}</h1>
        <p class="text-sm text-muted-foreground">
          Define fields in the form, inspect canonical JSON, or copy the API request.
        </p>
      </div>
    </div>

    <SchemaTabsEditor
      v-if="schemaContext.schema"
      v-model="formData"
      :schema="schemaContext.schema"
      :curl-snippets="curlSnippets"
      :form-title="`${schemaLabel} Fields`"
      @update:json-valid="(value) => (jsonValid = value)"
    />

    <Card v-else>
      <CardContent class="pt-6 text-sm text-muted-foreground">Loading schema…</CardContent>
    </Card>

    <Card v-if="isUserSchema">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Context</CardTitle>
      </CardHeader>
      <CardContent class="space-y-2 text-sm text-muted-foreground">
        <p>This {{ schemaLabel.toLowerCase() }} will be created in the current org context.</p>
        <p>
          <span class="font-medium text-foreground">Current org:</span>
          {{ currentOrgId || 'All organizations' }}
        </p>
        <p>Additional org memberships can be added later from the detail page.</p>
      </CardContent>
    </Card>

    <Card v-if="showActivationCard">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Access Setup</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div v-if="supportsPassword" class="space-y-2">
          <Label for="initial-password">Initial Password</Label>
          <Input
            id="initial-password"
            v-model="initialPassword"
            type="password"
            placeholder="Optional initial password"
          />
        </div>
        <div v-if="supportsInvite" class="flex items-start gap-3 rounded-lg border bg-muted/30 p-3">
          <Checkbox
            id="send-invite"
            :model-value="sendInvite"
            @update:model-value="(value) => (sendInvite = Boolean(value))"
          />
          <div class="space-y-1">
            <Label for="send-invite" class="text-sm font-medium">{{ inviteLabel }}</Label>
            <p class="text-xs text-muted-foreground">{{ inviteDescription }}</p>
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

    <div
      v-if="error"
      class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive"
    >
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
  import {
    buildUserDetailRoute,
    buildUserListRoute,
    getUserSchemaLabel,
    isUserSchemaType,
  } from '@/console/utils/user-routes'
  import { Button } from '@/components/ui/button'
  import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
  import { Checkbox } from '@/components/ui/checkbox'
  import { Input } from '@/components/ui/input'
  import { Label } from '@/components/ui/label'
  import { notifyMutationError, notifySuccess } from '@/lib/notify'
  import { ArrowLeft } from 'lucide-vue-next'

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
        notifySuccess(`${schemaLabel.value} created`, 'Sign-in link sent')
      } else {
        notifySuccess(`${schemaLabel.value} created`)
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
