<template>
  <div v-if="group" class="space-y-6">
    <div class="flex items-start justify-between gap-4">
      <div class="flex items-start gap-3">
        <Button variant="ghost" size="icon" as-child>
          <router-link to="/groups"><ArrowLeft class="size-4" /></router-link>
        </Button>
        <div>
          <h1 class="text-2xl font-semibold tracking-tight">{{ groupTitle }}</h1>
          <div class="mt-1 flex items-center gap-2 text-sm text-muted-foreground">
            <Badge variant="secondary" class="text-xs">{{ members.length }} members</Badge>
            <Badge :variant="group.state === 'active' ? 'default' : 'secondary'" class="capitalize text-xs">
              {{ group.state || 'active' }}
            </Badge>
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
      form-title="Group Fields"
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
        <p v-else class="text-sm text-muted-foreground">No members yet. Add users to this group.</p>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">System Information</CardTitle>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-[100px_1fr] gap-x-4 gap-y-2 text-sm">
          <dt class="text-muted-foreground">ID</dt>
          <dd class="font-mono text-xs break-all">{{ group.id }}</dd>
          <dt class="text-muted-foreground">Org</dt>
          <dd>{{ group.org_id || '—' }}</dd>
          <dt class="text-muted-foreground">Schema</dt>
          <dd>{{ group.schema_id || '—' }}</dd>
          <dt class="text-muted-foreground">Created</dt>
          <dd>{{ formatDateTime(group.created_at) }}</dd>
          <dt class="text-muted-foreground">Updated</dt>
          <dd>{{ formatDateTime(group.updated_at) }}</dd>
        </dl>
      </CardContent>
    </Card>

    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      {{ error }}
    </div>

    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Group</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ groupTitle }}</strong>? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" :disabled="deleting" @click="deleteGroup">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog v-model:open="showAddMember">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add Member</DialogTitle>
          <DialogDescription>Add a user by ID to this group.</DialogDescription>
        </DialogHeader>
        <div class="space-y-2 py-2">
          <Label for="group-member-user-id">User ID</Label>
          <Input id="group-member-user-id" v-model="newMemberUserId" placeholder="user ID" />
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
      <router-link to="/groups">← Back to Groups</router-link>
    </Button>
  </div>

  <div v-else class="flex h-48 items-center justify-center text-muted-foreground">Loading…</div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { groupApi, type Group, type Member } from '@/api/resources'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { formatDateTime } from '@/console/utils/format'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { ArrowLeft, Trash2, UserPlus } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()
const { currentOrgId } = useOrgContext()

const group = ref<Group | null>(null)
const members = ref<Member[]>([])
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: 'group',
  versions: [],
})
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const error = ref('')
const showDeleteConfirm = ref(false)
const showAddMember = ref(false)
const newMemberUserId = ref('')

const groupId = computed(() => String(route.params.id || ''))
const groupTitle = computed(() => String(formData.value.name || group.value?.name || 'Group'))
const payload = computed(() => buildResourceWriteBody('group', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/groups/${encodeURIComponent(groupId.value)}`,
  body: payload.value,
  includeOrgHeader: true,
  orgId: currentOrgId.value,
  methods: ['GET', 'PATCH'],
}))

async function loadGroup() {
  if (!groupId.value) return
  error.value = ''
  try {
    const [loadedGroup, loadedMembers] = await Promise.all([
      groupApi.get(groupId.value),
      groupApi.listMembers(groupId.value),
    ])
    group.value = loadedGroup
    members.value = loadedMembers
    formData.value = normalizeResourceData(loadedGroup.data || {})
    schemaContext.value = await loadResourceSchemaContext(loadedGroup.schema_type || 'group', loadedGroup.schema_id || '')
  } catch (err: any) {
    error.value = err?.message || 'Failed to load group'
  }
}

async function save() {
  if (!group.value) return
  saving.value = true
  error.value = ''
  try {
    group.value = await groupApi.update(group.value.id, payload.value)
    formData.value = normalizeResourceData(group.value.data || {})
  } catch (err: any) {
    error.value = err?.message || 'Failed to update group'
  } finally {
    saving.value = false
  }
}

async function deleteGroup() {
  if (!group.value) return
  deleting.value = true
  try {
    await groupApi.delete(group.value.id)
    router.push('/groups')
  } catch (err: any) {
    error.value = err?.message || 'Failed to delete group'
    showDeleteConfirm.value = false
  } finally {
    deleting.value = false
  }
}

async function addMember() {
  if (!group.value) return
  try {
    await groupApi.addMember(group.value.id, newMemberUserId.value.trim())
    newMemberUserId.value = ''
    showAddMember.value = false
    members.value = await groupApi.listMembers(group.value.id)
  } catch (err: any) {
    error.value = err?.message || 'Failed to add member'
  }
}

async function removeMember(userId: string) {
  if (!group.value) return
  try {
    await groupApi.removeMember(group.value.id, userId)
    members.value = await groupApi.listMembers(group.value.id)
  } catch (err: any) {
    error.value = err?.message || 'Failed to remove member'
  }
}

onMounted(loadGroup)
watch(groupId, loadGroup)
</script>
