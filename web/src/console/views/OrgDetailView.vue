<template>
  <div class="space-y-6 max-w-3xl">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="icon" as-child>
          <router-link to="/orgs"><ArrowLeft class="size-4" /></router-link>
        </Button>
        <div v-if="item">
          <h1 class="text-2xl font-semibold tracking-tight">{{ item.name }}</h1>
          <div class="flex items-center gap-2 text-sm text-muted-foreground">
            <Badge :variant="item.state === 'active' ? 'default' : 'destructive'" class="capitalize text-xs">{{ item.state || 'active' }}</Badge>
            <span>·</span>
            <span class="text-xs">Created {{ formatDate(item.created_at) }}</span>
          </div>
        </div>
      </div>
      <Button v-if="item" variant="destructive" size="sm" @click="showDeleteConfirm = true">
        <Trash2 class="size-3.5 mr-1" /> Delete
      </Button>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="space-y-3">
      <div class="h-12 rounded-lg bg-muted animate-pulse" />
      <div class="h-8 rounded-lg bg-muted animate-pulse w-3/4" />
    </div>

    <!-- Details -->
    <Card v-if="item">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Details</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="space-y-2">
          <Label for="org-name">Name</Label>
          <Input id="org-name" v-model="editName" />
        </div>
        <div class="space-y-2">
          <Label for="org-state">State</Label>
          <Select v-model="editState">
            <SelectTrigger id="org-state"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="active">Active</SelectItem>
              <SelectItem value="inactive">Inactive</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </CardContent>
    </Card>

    <!-- Members -->
    <Card v-if="item">
      <CardHeader class="pb-3 flex flex-row items-center justify-between">
        <CardTitle class="text-sm">Members</CardTitle>
        <Button variant="outline" size="sm" class="h-7 text-xs" @click="showAddMember = true">
          <UserPlus class="size-3 mr-1" /> Add Member
        </Button>
      </CardHeader>
      <CardContent>
        <div v-if="members.length" class="space-y-2">
          <div
            v-for="m in members" :key="m.user_id"
            class="flex items-center justify-between p-2.5 rounded-lg border bg-muted/30"
          >
            <div class="flex items-center gap-3">
              <div class="flex items-center justify-center size-8 rounded-md bg-primary/10 text-primary font-bold text-xs">
                {{ (m.display_name || m.user_id)[0]?.toUpperCase() }}
              </div>
              <div>
                <p class="text-sm font-medium">{{ m.display_name || m.user_id }}</p>
                <p class="text-xs text-muted-foreground">{{ m.role }}</p>
              </div>
            </div>
            <Button variant="ghost" size="icon" class="size-7 text-muted-foreground hover:text-destructive" @click="removeMember(m.user_id)">
              <Trash2 class="size-3.5" />
            </Button>
          </div>
        </div>
        <p v-else class="text-sm text-muted-foreground">No members yet. Add users to this organization.</p>
      </CardContent>
    </Card>

    <!-- System -->
    <Card v-if="item">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">System Information</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="grid grid-cols-2 gap-y-2 text-sm">
          <span class="text-muted-foreground">ID</span>
          <span class="font-mono text-xs break-all">{{ item.id }}</span>
          <span class="text-muted-foreground">Created</span>
          <span>{{ formatDate(item.created_at) }}</span>
          <span class="text-muted-foreground">Updated</span>
          <span>{{ formatDate(item.updated_at) }}</span>
        </div>
      </CardContent>
    </Card>

    <!-- Save -->
    <div v-if="hasChanges" class="flex justify-end gap-3">
      <Button variant="outline" @click="resetEdits">Discard</Button>
      <Button :disabled="saving" @click="saveChanges">{{ saving ? 'Saving…' : 'Save Changes' }}</Button>
    </div>

    <!-- Delete Confirmation Dialog -->
    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Organization</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ item?.name }}</strong>? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" @click="confirmDelete" :disabled="deleting">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Add Member Dialog -->
    <Dialog v-model:open="showAddMember">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>Add Member</DialogTitle>
        </DialogHeader>
        <div class="space-y-3 py-2">
          <div class="space-y-2">
            <Label for="member-user-id">User ID</Label>
            <Input id="member-user-id" v-model="newMemberUserId" placeholder="user ID" />
          </div>
        </div>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showAddMember = false">Cancel</Button>
          <Button @click="addMember" :disabled="!newMemberUserId">
            <UserPlus class="size-3.5 mr-1" /> Add
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { orgApi, orgMembersApi, type Org, type OrgMember } from '@/api/resources'
import { formatDate } from '@/console/utils/format'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { ArrowLeft, Trash2, UserPlus } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

const item = ref<Org | null>(null)
const loading = ref(false)
const saving = ref(false)
const deleting = ref(false)
const showDeleteConfirm = ref(false)
const editName = ref('')
const editState = ref('active')
const members = ref<OrgMember[]>([])
const showAddMember = ref(false)
const newMemberUserId = ref('')

const orgId = computed(() => route.params.id as string)

const hasChanges = computed(() => {
  if (!item.value) return false
  return editName.value !== (item.value.name || '') || editState.value !== (item.value.state || 'active')
})

function resetEdits() {
  if (item.value) {
    editName.value = item.value.name || ''
    editState.value = item.value.state || 'active'
  }
}

async function loadOrg() {
  loading.value = true
  try {
    item.value = await orgApi.get(orgId.value)
    resetEdits()
    members.value = await orgMembersApi.list(orgId.value)
  } catch (e: any) {
    toast.error('Failed to load organization', { description: e?.message })
  } finally {
    loading.value = false
  }
}

async function saveChanges() {
  saving.value = true
  try {
    const changes: Record<string, any> = {}
    if (editName.value !== (item.value?.name || '')) changes.name = editName.value
    if (editState.value !== (item.value?.state || 'active')) changes.state = editState.value
    await orgApi.update(orgId.value, changes)
    await loadOrg()
    toast.success('Organization updated')
  } catch (e: any) {
    toast.error('Failed to save', { description: e?.message })
  } finally {
    saving.value = false
  }
}

async function confirmDelete() {
  deleting.value = true
  try {
    await orgApi.delete(orgId.value)
    toast.success('Organization deleted')
    router.push('/orgs')
  } catch (e: any) {
    toast.error('Failed to delete', { description: e?.message })
    showDeleteConfirm.value = false
  } finally {
    deleting.value = false
  }
}

async function addMember() {
  try {
    await orgMembersApi.add(orgId.value, newMemberUserId.value)
    toast.success('Member added')
    showAddMember.value = false
    newMemberUserId.value = ''
    members.value = await orgMembersApi.list(orgId.value)
  } catch (e: any) {
    toast.error('Failed to add member', { description: e?.message })
  }
}

async function removeMember(userId: string) {
  try {
    await orgMembersApi.remove(orgId.value, userId)
    toast.success('Member removed')
    members.value = await orgMembersApi.list(orgId.value)
  } catch (e: any) {
    toast.error('Failed to remove member', { description: e?.message })
  }
}

onMounted(loadOrg)
watch(orgId, loadOrg)
</script>
