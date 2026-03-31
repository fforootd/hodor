<template>
  <ResourceDetailView
    :resource="identity"
    resource-type="user"
    :singular-title="schemaLabel"
    :back-route="backRoute"
    :display-title="identityTitle"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :saving="saving"
    :deleting="deleting"
    :load-error="loadError"
    :json-valid="jsonValid"
    :show-avatar="true"
    v-model:form-data="formData"
    @save="save"
    @delete="deleteIdentity"
    @update:json-valid="(v) => jsonValid = v"
  >
    <template #header-badges>
      <code class="rounded bg-muted px-1.5 py-0.5 text-xs">{{ identity?.identifier }}</code>
      <StateBadge :state="identity?.state" />
    </template>

    <template #header-actions>
      <Button v-if="supportsInvite" variant="outline" size="sm" :disabled="inviting" @click="sendInvite">
        <Mail class="mr-1 size-3.5" />
        {{ inviting ? 'Sending…' : 'Invite' }}
      </Button>
    </template>

    <template #after-form>
      <!-- Org Memberships -->
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
                <DropdownMenuItem v-for="o in availableOrgs" :key="o.id" @click="addToOrg(o.id)">
                  {{ o.name }}
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
                variant="ghost" size="icon"
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

      <!-- Related Activity -->
      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-sm">Related Activity</CardTitle>
        </CardHeader>
        <CardContent>
          <div class="grid gap-3 md:grid-cols-3">
            <RouterLink :to="{ path: '/events', query: { actor: identity?.id } }" class="rounded-lg border p-3 text-sm transition-colors hover:bg-muted/50">Events</RouterLink>
            <RouterLink :to="{ path: '/sessions', query: { user: identity?.id } }" class="rounded-lg border p-3 text-sm transition-colors hover:bg-muted/50">Sessions</RouterLink>
            <RouterLink :to="{ path: '/traces', query: { id: identity?.id } }" class="rounded-lg border p-3 text-sm transition-colors hover:bg-muted/50">Traces</RouterLink>
          </div>
        </CardContent>
      </Card>
    </template>
  </ResourceDetailView>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { magicLinkApi, orgApi, orgMembersApi, userApi, type Identity, type Org } from '@/api/resources'
import ResourceDetailView from '@/console/components/ResourceDetailView.vue'
import { StateBadge } from '@/components/ui/state-badge'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets, buildResourceWriteBody, formatFieldLabel,
  loadResourceSchemaContext, normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { notifyError, notifyMutationError, notifyMutationSuccess, notifySuccess } from '@/lib/notify'
import { Mail, Plus, X } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()
const { currentOrgId } = useOrgContext()

interface OrgMembership { org_id: string; org_name: string; role: string; added_at: string }

const identity = ref<Identity | null>(null)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {}, schema: null, schemaId: '', schemaType: '', versions: [],
})
const allOrgs = ref<Org[]>([])
const userOrgs = ref<OrgMembership[]>([])
const removingOrgId = ref('')
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const inviting = ref(false)
const loadError = ref('')

const identityId = computed(() => String(route.params.id || ''))
const routeSchemaType = computed(() => String(route.params.schemaType || ''))
const schemaLabel = computed(() =>
  String(schemaContext.value.display.singular || formatFieldLabel((identity.value?.schema_type || routeSchemaType.value || 'user').replace(/_/g, ' ')))
)
const identityTitle = computed(() => String(formData.value.display_name || identity.value?.display_name || identity.value?.identifier || schemaLabel.value))
const backRoute = computed(() => routeSchemaType.value ? `/s/${routeSchemaType.value}` : '/users')
const payload = computed(() => buildResourceWriteBody('user', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/users/${encodeURIComponent(identityId.value)}`, body: payload.value,
  includeOrgHeader: true, orgId: currentOrgId.value, methods: ['GET', 'PATCH'],
}))
const authMethods = computed<Record<string, { enabled?: boolean; interactive?: boolean }>>(
  () => (schemaContext.value.schema?.['x-auth-methods'] as any) || {},
)
const supportsInvite = computed(() =>
  Object.values(authMethods.value).some(m => m?.interactive !== false && m?.enabled !== false),
)
const availableOrgs = computed(() => {
  const memberOrgIds = new Set(userOrgs.value.map(m => m.org_id))
  return allOrgs.value.filter(o => !memberOrgIds.has(o.id))
})

async function loadIdentity() {
  if (!identityId.value) return
  loadError.value = ''
  try {
    const loaded = await userApi.get(identityId.value)
    identity.value = loaded
    formData.value = normalizeResourceData(loaded.data || {})
    schemaContext.value = await loadResourceSchemaContext(
      loaded.schema_type || routeSchemaType.value || 'human_user', loaded.schema_id || '',
    )
    userOrgs.value = ((loaded.orgs as OrgMembership[] | undefined) || []).map(m => ({ ...m }))
    allOrgs.value = await orgApi.list()
  } catch (err: any) { loadError.value = err?.message || 'Failed to load identity' }
}

async function refreshIdentity() {
  if (!identity.value) return
  const loaded = await userApi.get(identity.value.id)
  identity.value = loaded
  formData.value = normalizeResourceData(loaded.data || {})
  userOrgs.value = ((loaded.orgs as OrgMembership[] | undefined) || []).map(m => ({ ...m }))
}

async function save() {
  if (!identity.value) return
  saving.value = true
  try {
    identity.value = await userApi.update(identity.value.id, payload.value)
    formData.value = normalizeResourceData(identity.value.data || {})
    userOrgs.value = ((identity.value.orgs as OrgMembership[] | undefined) || []).map(m => ({ ...m }))
    notifyMutationSuccess(schemaLabel.value, 'update')
  } catch (err: any) { notifyMutationError(schemaLabel.value, 'update', err) }
  finally { saving.value = false }
}

async function sendInvite() {
  if (!identity.value) return
  inviting.value = true
  try {
    await magicLinkApi.send(identity.value.identifier)
    notifySuccess('Invite sent')
  } catch (err: any) { notifyError('Failed to send invite', err) }
  finally { inviting.value = false }
}

async function addToOrg(orgId: string) {
  if (!identity.value) return
  try {
    await orgMembersApi.add(orgId, identity.value.id)
    await refreshIdentity()
    notifyMutationSuccess('Organization membership', 'add')
  } catch (err: any) { notifyMutationError('Organization membership', 'add', err) }
}

async function removeFromOrg(orgId: string) {
  if (!identity.value) return
  removingOrgId.value = orgId
  try {
    await orgMembersApi.remove(orgId, identity.value.id)
    await refreshIdentity()
    notifyMutationSuccess('Organization membership', 'remove')
  } catch (err: any) { notifyMutationError('Organization membership', 'remove', err) }
  finally { removingOrgId.value = '' }
}

async function deleteIdentity() {
  if (!identity.value) return
  deleting.value = true
  try {
    await userApi.delete(identity.value.id)
    notifyMutationSuccess(schemaLabel.value, 'delete')
    router.push(backRoute.value)
  } catch (err: any) { notifyMutationError(schemaLabel.value, 'delete', err) }
  finally { deleting.value = false }
}

onMounted(loadIdentity)
watch(identityId, loadIdentity)
</script>
