<template>
  <div v-if="identity" class="space-y-6">
    <div class="flex items-start gap-4">
      <Avatar class="size-12 rounded-xl">
        <AvatarFallback class="rounded-xl bg-primary text-primary-foreground text-lg font-bold">
          {{ identityInitial }}
        </AvatarFallback>
      </Avatar>
      <div class="min-w-0 flex-1">
        <h1 class="truncate text-2xl font-semibold tracking-tight">{{ identityTitle }}</h1>
        <div class="mt-1 flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
          <code class="rounded bg-muted px-1.5 py-0.5 text-xs">{{ identity.identifier }}</code>
          <Badge :variant="identity.state === 'active' ? 'default' : 'secondary'" class="text-xs">
            {{ identity.state }}
          </Badge>
        </div>
      </div>
      <div class="flex gap-2">
        <Button v-if="supportsInvite" variant="outline" size="sm" :disabled="inviting" @click="sendInvite">
          <Mail class="mr-1 size-3.5" />
          {{ inviting ? 'Sending…' : 'Invite' }}
        </Button>
        <Button variant="outline" size="sm" :disabled="saving || !jsonValid" @click="save">
          {{ saving ? 'Saving…' : 'Save' }}
        </Button>
        <Button variant="destructive" size="sm" @click="showDeleteConfirm = true">Delete</Button>
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

    <Card>
      <CardHeader class="pb-3">
        <div class="flex items-center justify-between gap-4">
          <CardTitle class="text-sm">Organizations</CardTitle>
          <DropdownMenu v-if="availableOrgs.length">
            <DropdownMenuTrigger as-child>
              <Button variant="outline" size="sm">
                <Plus class="mr-1 size-3.5" /> Add
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                v-for="orgOption in availableOrgs"
                :key="orgOption.id"
                @click="addToOrg(orgOption.id)"
              >
                {{ orgOption.name }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </CardHeader>
      <CardContent>
        <div v-if="userOrgs.length" class="space-y-2">
          <div
            v-for="membership in userOrgs"
            :key="membership.org_id"
            class="flex items-center justify-between rounded-lg border bg-muted/30 p-3"
          >
            <div>
              <p class="text-sm font-medium">{{ membership.org_name || membership.org_id }}</p>
              <p class="text-xs text-muted-foreground">{{ membership.role }}</p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground hover:text-destructive"
              :disabled="removingOrgId === membership.org_id"
              @click="removeFromOrg(membership.org_id)"
            >
              <X class="size-3.5" />
            </Button>
          </div>
        </div>
        <p v-else class="text-sm text-muted-foreground">Not a member of any organization.</p>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">System Information</CardTitle>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-[100px_1fr] gap-x-4 gap-y-2 text-sm">
          <dt class="text-muted-foreground">ID</dt>
          <dd class="font-mono text-xs break-all">{{ identity.id }}</dd>
          <dt class="text-muted-foreground">Schema</dt>
          <dd>{{ identity.schema_id || '—' }}</dd>
          <dt class="text-muted-foreground">Created</dt>
          <dd>{{ formatDateTime(identity.created_at) }}</dd>
          <dt class="text-muted-foreground">Updated</dt>
          <dd>{{ formatDateTime(identity.updated_at) }}</dd>
        </dl>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Related Activity</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="grid gap-3 md:grid-cols-3">
          <RouterLink
            :to="{ path: '/events', query: { actor: identity.id } }"
            class="rounded-lg border p-3 text-sm transition-colors hover:bg-muted/50"
          >
            Events
          </RouterLink>
          <RouterLink
            :to="{ path: '/sessions', query: { user: identity.id } }"
            class="rounded-lg border p-3 text-sm transition-colors hover:bg-muted/50"
          >
            Sessions
          </RouterLink>
          <RouterLink
            :to="{ path: '/traces', query: { id: identity.id } }"
            class="rounded-lg border p-3 text-sm transition-colors hover:bg-muted/50"
          >
            Traces
          </RouterLink>
        </div>
      </CardContent>
    </Card>

    <div v-if="message" :class="messageClass">
      {{ message }}
    </div>

    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete {{ schemaLabel }}</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ identityTitle }}</strong>? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" :disabled="deleting" @click="deleteIdentity">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Button variant="link" as-child class="px-0 text-muted-foreground">
      <router-link :to="backRoute">← Back to {{ backLabel }}</router-link>
    </Button>
  </div>

  <div v-else class="flex h-48 items-center justify-center text-muted-foreground">Loading…</div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { magicLinkApi, orgApi, orgMembersApi, userApi, type Identity, type Org } from '@/api/resources'
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
import { formatDateTime } from '@/console/utils/format'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Mail, Plus, X } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()
const { currentOrgId } = useOrgContext()

interface OrgMembership {
  org_id: string
  org_name: string
  role: string
  added_at: string
}

const identity = ref<Identity | null>(null)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: '',
  versions: [],
})
const allOrgs = ref<Org[]>([])
const userOrgs = ref<OrgMembership[]>([])
const removingOrgId = ref('')
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const inviting = ref(false)
const message = ref('')
const messageType = ref<'success' | 'error' | 'info'>('success')
const showDeleteConfirm = ref(false)

const identityId = computed(() => String(route.params.id || ''))
const routeSchemaType = computed(() => String(route.params.schemaType || ''))
const schemaLabel = computed(() =>
  String(schemaContext.value.display.singular || formatFieldLabel((identity.value?.schema_type || routeSchemaType.value || 'user').replace(/_/g, ' '))),
)
const identityTitle = computed(() => String(formData.value.display_name || identity.value?.display_name || identity.value?.identifier || schemaLabel.value))
const identityInitial = computed(() => identityTitle.value.charAt(0).toUpperCase() || 'U')
const backRoute = computed(() => routeSchemaType.value ? `/s/${routeSchemaType.value}` : '/users')
const backLabel = computed(() => routeSchemaType.value ? schemaLabel.value + 's' : 'Users')
const payload = computed(() => buildResourceWriteBody('user', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/users/${encodeURIComponent(identityId.value)}`,
  body: payload.value,
  includeOrgHeader: true,
  orgId: currentOrgId.value,
  methods: ['GET', 'PATCH'],
}))
const authMethods = computed<Record<string, { enabled?: boolean; interactive?: boolean }>>(
  () => (schemaContext.value.schema?.['x-auth-methods'] as Record<string, { enabled?: boolean; interactive?: boolean }>) || {},
)
const supportsInvite = computed(() =>
  Object.values(authMethods.value).some((method) => method?.interactive !== false && method?.enabled !== false),
)
const availableOrgs = computed(() => {
  const memberOrgIds = new Set(userOrgs.value.map((membership) => membership.org_id))
  return allOrgs.value.filter((orgOption) => !memberOrgIds.has(orgOption.id))
})
const messageClass = computed(() => [
  'rounded-lg border px-4 py-3 text-sm',
  messageType.value === 'success'
    ? 'border-green-200 bg-green-50 text-green-700'
    : messageType.value === 'info'
      ? 'border-blue-200 bg-blue-50 text-blue-700'
      : 'border-destructive/50 bg-destructive/10 text-destructive',
])

async function loadIdentity() {
  if (!identityId.value) return
  message.value = ''
  try {
    const loadedIdentity = await userApi.get(identityId.value)
    identity.value = loadedIdentity
    formData.value = normalizeResourceData(loadedIdentity.data || {})
    schemaContext.value = await loadResourceSchemaContext(
      loadedIdentity.schema_type || routeSchemaType.value || 'human_user',
      loadedIdentity.schema_id || '',
    )
    userOrgs.value = ((loadedIdentity.orgs as OrgMembership[] | undefined) || []).map((membership) => ({ ...membership }))
    allOrgs.value = await orgApi.list()
  } catch (err: any) {
    message.value = err?.message || 'Failed to load identity'
    messageType.value = 'error'
  }
}

async function refreshIdentity() {
  if (!identity.value) return
  const loadedIdentity = await userApi.get(identity.value.id)
  identity.value = loadedIdentity
  formData.value = normalizeResourceData(loadedIdentity.data || {})
  userOrgs.value = ((loadedIdentity.orgs as OrgMembership[] | undefined) || []).map((membership) => ({ ...membership }))
}

async function save() {
  if (!identity.value) return
  saving.value = true
  message.value = ''
  try {
    identity.value = await userApi.update(identity.value.id, payload.value)
    formData.value = normalizeResourceData(identity.value.data || {})
    userOrgs.value = ((identity.value.orgs as OrgMembership[] | undefined) || []).map((membership) => ({ ...membership }))
    message.value = `${schemaLabel.value} updated`
    messageType.value = 'success'
  } catch (err: any) {
    message.value = err?.message || 'Failed to update identity'
    messageType.value = 'error'
  } finally {
    saving.value = false
  }
}

async function sendInvite() {
  if (!identity.value) return
  inviting.value = true
  message.value = ''
  try {
    await magicLinkApi.send(identity.value.identifier)
    message.value = 'Invite sent'
    messageType.value = 'info'
  } catch (err: any) {
    message.value = err?.message || 'Failed to send invite'
    messageType.value = 'error'
  } finally {
    inviting.value = false
  }
}

async function addToOrg(orgId: string) {
  if (!identity.value) return
  try {
    await orgMembersApi.add(orgId, identity.value.id)
    await refreshIdentity()
    message.value = 'Added to organization'
    messageType.value = 'success'
  } catch (err: any) {
    message.value = err?.message || 'Failed to add to organization'
    messageType.value = 'error'
  }
}

async function removeFromOrg(orgId: string) {
  if (!identity.value) return
  removingOrgId.value = orgId
  try {
    await orgMembersApi.remove(orgId, identity.value.id)
    await refreshIdentity()
    message.value = 'Removed from organization'
    messageType.value = 'success'
  } catch (err: any) {
    message.value = err?.message || 'Failed to remove from organization'
    messageType.value = 'error'
  } finally {
    removingOrgId.value = ''
  }
}

async function deleteIdentity() {
  if (!identity.value) return
  deleting.value = true
  try {
    await userApi.delete(identity.value.id)
    router.push(backRoute.value)
  } catch (err: any) {
    message.value = err?.message || 'Failed to delete identity'
    messageType.value = 'error'
    showDeleteConfirm.value = false
  } finally {
    deleting.value = false
  }
}

onMounted(loadIdentity)
watch(identityId, loadIdentity)
</script>
