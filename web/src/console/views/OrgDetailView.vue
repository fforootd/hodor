<template>
  <div v-if="org" class="space-y-6">
    <div class="flex items-start justify-between gap-4">
      <div class="flex items-start gap-3">
        <Button variant="ghost" size="icon" as-child>
          <router-link to="/orgs"><ArrowLeft class="size-4" /></router-link>
        </Button>
        <div>
          <h1 class="text-2xl font-semibold tracking-tight">{{ orgTitle }}</h1>
          <div class="mt-1 flex items-center gap-2 text-sm text-muted-foreground">
            <Badge :variant="org.state === 'active' ? 'default' : 'secondary'" class="capitalize text-xs">
              {{ org.state || 'active' }}
            </Badge>
            <span>Created {{ formatDate(org.created_at) }}</span>
          </div>
        </div>
      </div>
      <div class="flex gap-2">
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
      form-title="Organization Fields"
      @update:json-valid="(value) => jsonValid = value"
    />

    <Card v-else>
      <CardContent class="pt-6 text-sm text-muted-foreground">Loading schema…</CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-3">
        <div class="flex items-center justify-between gap-4">
          <CardTitle class="text-sm">Members</CardTitle>
          <Button variant="outline" size="sm" @click="showAddMember = true">
            <UserPlus class="mr-1 size-3.5" /> Add Member
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div v-if="members.length" class="space-y-2">
          <div
            v-for="member in members"
            :key="member.user_id"
            class="flex items-center justify-between rounded-lg border bg-muted/30 p-3"
          >
            <div>
              <p class="text-sm font-medium">{{ member.display_name || member.user_id }}</p>
              <p class="text-xs text-muted-foreground">{{ member.role }}</p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground hover:text-destructive"
              @click="removeMember(member.user_id)"
            >
              <Trash2 class="size-3.5" />
            </Button>
          </div>
        </div>
        <p v-else class="text-sm text-muted-foreground">No members yet. Add users to this organization.</p>
      </CardContent>
    </Card>

    <div v-if="error" role="alert" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      {{ error }}
    </div>

    <SystemInfoCard
      :id="org.id"
      :schema-id="org.schema_id"
      :created-at="org.created_at"
      :updated-at="org.updated_at"
    />

    <ResourceDeleteDialog
      v-model:open="showDeleteConfirm"
      resource-name="Organization"
      :item-name="orgTitle"
      :deleting="deleting"
      @confirm="deleteOrg"
    />

    <Dialog v-model:open="showAddMember">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add Member</DialogTitle>
          <DialogDescription>Add a user by ID to this organization.</DialogDescription>
        </DialogHeader>
        <div class="space-y-2 py-2">
          <Label for="org-member-user-id">User ID</Label>
          <Input id="org-member-user-id" v-model="newMemberUserId" placeholder="user ID" />
        </div>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showAddMember = false">Cancel</Button>
          <Button :disabled="!newMemberUserId.trim()" @click="addMember">
            <UserPlus class="mr-1 size-3.5" /> Add
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Button variant="link" as-child class="px-0 text-muted-foreground">
      <router-link to="/orgs">← Back to Organizations</router-link>
    </Button>
  </div>

  <div v-else class="flex h-48 items-center justify-center text-muted-foreground">Loading…</div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { orgApi, orgMembersApi, type Org, type OrgMember } from '@/api/resources'
import ResourceDeleteDialog from '@/console/components/ResourceDeleteDialog.vue'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import SystemInfoCard from '@/console/components/SystemInfoCard.vue'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { formatDate } from '@/console/utils/format'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
// Dialog still needed for Add Member dialog
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { ArrowLeft, Trash2, UserPlus } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

const org = ref<Org | null>(null)
const members = ref<OrgMember[]>([])
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: 'org',
  versions: [],
})
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const error = ref('')
const showDeleteConfirm = ref(false)
const showAddMember = ref(false)
const newMemberUserId = ref('')

const orgId = computed(() => String(route.params.id || ''))
const orgTitle = computed(() => String(formData.value.display_name || org.value?.name || 'Organization'))
const payload = computed(() => buildResourceWriteBody('org', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/orgs/${encodeURIComponent(orgId.value)}`,
  body: payload.value,
  includeOrgHeader: false,
  methods: ['GET', 'PATCH'],
}))

async function loadOrg() {
  if (!orgId.value) return
  error.value = ''
  try {
    const [loadedOrg, loadedMembers] = await Promise.all([
      orgApi.get(orgId.value),
      orgMembersApi.list(orgId.value),
    ])
    org.value = loadedOrg
    members.value = loadedMembers
    formData.value = normalizeResourceData(loadedOrg.data || {})
    schemaContext.value = await loadResourceSchemaContext(loadedOrg.schema_type || 'org', loadedOrg.schema_id || '')
  } catch (err: any) {
    error.value = err?.message || 'Failed to load organization'
  }
}

async function save() {
  if (!org.value) return
  saving.value = true
  error.value = ''
  try {
    org.value = await orgApi.update(org.value.id, payload.value)
    formData.value = normalizeResourceData(org.value.data || {})
  } catch (err: any) {
    error.value = err?.message || 'Failed to update organization'
  } finally {
    saving.value = false
  }
}

async function deleteOrg() {
  if (!org.value) return
  deleting.value = true
  try {
    await orgApi.delete(org.value.id)
    router.push('/orgs')
  } catch (err: any) {
    error.value = err?.message || 'Failed to delete organization'
    showDeleteConfirm.value = false
  } finally {
    deleting.value = false
  }
}

async function addMember() {
  if (!org.value) return
  try {
    await orgMembersApi.add(org.value.id, newMemberUserId.value.trim())
    newMemberUserId.value = ''
    showAddMember.value = false
    members.value = await orgMembersApi.list(org.value.id)
  } catch (err: any) {
    error.value = err?.message || 'Failed to add member'
  }
}

async function removeMember(userId: string) {
  if (!org.value) return
  try {
    await orgMembersApi.remove(org.value.id, userId)
    members.value = await orgMembersApi.list(org.value.id)
  } catch (err: any) {
    error.value = err?.message || 'Failed to remove member'
  }
}

onMounted(loadOrg)
watch(orgId, loadOrg)
</script>
