<template>
  <div class="space-y-6">
    <div v-if="loading" class="flex items-center justify-center py-20">
      <RefreshCw class="size-5 animate-spin text-muted-foreground" />
    </div>
    <template v-else-if="group">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <Button variant="ghost" size="sm" @click="$router.push('/groups')">
            <ArrowLeft class="size-4" />
          </Button>
          <div>
            <h1 class="text-2xl font-semibold tracking-tight">{{ group.name }}</h1>
            <p class="text-sm text-muted-foreground mt-0.5">{{ group.description || 'No description' }}</p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Badge variant="secondary" class="gap-1">
            <UsersRound class="size-3" />
            {{ members.length }} member{{ members.length !== 1 ? 's' : '' }}
          </Badge>
          <Button variant="destructive" size="sm" @click="showDeleteConfirm = true">
            <Trash2 class="size-3.5 mr-1" /> Delete
          </Button>
        </div>
      </div>

      <Tabs v-model="activeTab" class="space-y-4">
        <TabsList class="grid w-full max-w-sm grid-cols-2">
          <TabsTrigger value="members" class="gap-1.5">
            <UsersRound class="size-3.5" /> Members
          </TabsTrigger>
          <TabsTrigger value="settings" class="gap-1.5">
            <Settings class="size-3.5" /> Settings
          </TabsTrigger>
        </TabsList>

        <!-- Members tab -->
        <TabsContent value="members" class="space-y-4">
          <Card>
            <div class="p-4 pb-2 flex items-center justify-between">
              <h3 class="font-medium">Group Members</h3>
              <Button variant="outline" size="sm" @click="showAddMember = true">
                <UserPlus class="size-3.5 mr-1" /> Add Member
              </Button>
            </div>
            <div class="border-t">
              <table class="w-full">
                <thead>
                  <tr class="border-b bg-muted/50">
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">User</th>
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Role</th>
                    <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Added</th>
                    <th class="px-4 py-2 text-right text-xs font-medium text-muted-foreground">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-if="!members.length" class="border-b">
                    <td colspan="4" class="px-4 py-8 text-center text-sm text-muted-foreground">
                      No members yet. Add users to this group.
                    </td>
                  </tr>
                  <tr v-for="m in members" :key="m.user_id" class="border-b hover:bg-muted/30 transition-colors">
                    <td class="px-4 py-2.5">
                      <div class="flex items-center gap-2">
                        <User class="size-4 text-muted-foreground" />
                        <span class="text-sm font-medium">{{ m.display_name || m.user_id }}</span>
                        <code class="text-[10px] bg-muted px-1 rounded">{{ m.user_id }}</code>
                      </div>
                    </td>
                    <td class="px-4 py-2.5">
                      <Badge variant="secondary" class="text-xs">{{ m.role }}</Badge>
                    </td>
                    <td class="px-4 py-2.5 text-xs text-muted-foreground tabular-nums">
                      {{ new Date(m.added_at).toLocaleDateString() }}
                    </td>
                    <td class="px-4 py-2.5 text-right">
                      <Button variant="ghost" size="sm" class="h-7 text-destructive" @click="removeMember(m.user_id)">
                        <Trash2 class="size-3.5" />
                      </Button>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </Card>
        </TabsContent>

        <!-- Settings tab -->
        <TabsContent value="settings" class="space-y-4">
          <Card class="p-4 space-y-4">
            <h3 class="font-medium">Group Settings</h3>
            <div class="space-y-3">
              <div>
                <label class="text-sm font-medium">Name</label>
                <Input v-model="editName" class="mt-1" />
              </div>
              <div>
                <label class="text-sm font-medium">Description</label>
                <Input v-model="editDescription" class="mt-1" />
              </div>
            </div>
            <Button @click="saveSettings" :disabled="!editName">Save Changes</Button>
          </Card>
        </TabsContent>
      </Tabs>
    </template>

    <!-- Add Member Dialog -->
    <Dialog v-model:open="showAddMember">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>Add Member</DialogTitle>
        </DialogHeader>
        <div class="space-y-3 py-2">
          <div>
            <label class="text-sm font-medium">User ID</label>
            <Input v-model="newMemberUserId" placeholder="user ID" class="mt-1" />
          </div>
        </div>
        <div class="flex justify-end gap-2 pt-2">
          <Button variant="outline" @click="showAddMember = false">Cancel</Button>
          <Button @click="addMember" :disabled="!newMemberUserId">
            <UserPlus class="size-3.5 mr-1" /> Add
          </Button>
        </div>
      </DialogContent>
    </Dialog>

    <!-- Delete Confirmation Dialog -->
    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Group</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ group?.name }}</strong>? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" @click="deleteGroup" :disabled="deleting">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { groupApi, type Group, type Member } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { RefreshCw, ArrowLeft, UsersRound, User, UserPlus, Trash2, Settings } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

const loading = ref(true)
const group = ref<Group | null>(null)
const members = ref<Member[]>([])
const activeTab = ref('members')
const showAddMember = ref(false)
const newMemberUserId = ref('')
const editName = ref('')
const editDescription = ref('')
const showDeleteConfirm = ref(false)
const deleting = ref(false)

async function fetchGroup() {
  const id = route.params.id as string
  loading.value = true
  try {
    group.value = await groupApi.get(id)
    editName.value = group.value.name
    editDescription.value = group.value.description
    members.value = await groupApi.listMembers(id)
  } catch (err: any) {
    toast.error('Failed to load group', { description: err.message })
  } finally {
    loading.value = false
  }
}

async function addMember() {
  const id = route.params.id as string
  try {
    await groupApi.addMember(id, newMemberUserId.value)
    toast.success('Member added')
    showAddMember.value = false
    newMemberUserId.value = ''
    members.value = await groupApi.listMembers(id)
  } catch (err: any) {
    toast.error('Failed to add member', { description: err.message })
  }
}

async function removeMember(userId: string) {
  const id = route.params.id as string
  try {
    await groupApi.removeMember(id, userId)
    toast.success('Member removed')
    members.value = await groupApi.listMembers(id)
  } catch (err: any) {
    toast.error('Failed to remove member', { description: err.message })
  }
}

async function saveSettings() {
  const id = route.params.id as string
  try {
    group.value = await groupApi.update(id, { name: editName.value, description: editDescription.value })
    toast.success('Group updated')
  } catch (err: any) {
    toast.error('Failed to update group', { description: err.message })
  }
}

async function deleteGroup() {
  const id = route.params.id as string
  deleting.value = true
  try {
    await groupApi.delete(id)
    toast.success('Group deleted')
    router.push('/groups')
  } catch (err: any) {
    toast.error('Failed to delete group', { description: err.message })
    showDeleteConfirm.value = false
  } finally {
    deleting.value = false
  }
}

onMounted(fetchGroup)
</script>
