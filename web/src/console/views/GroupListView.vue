<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Groups</h1>
        <p class="text-sm text-muted-foreground">{{ loading ? 'Loading…' : `${groups.length} group${groups.length !== 1 ? 's' : ''}` }}</p>
      </div>
      <Button @click="showCreate = true">
        <Plus class="mr-2 size-4" />
        New Group
      </Button>
    </div>

    <!-- Search -->
    <div class="relative w-full max-w-md">
      <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
      <Input v-model="searchQuery" placeholder="Search groups…" class="pl-9" />
    </div>

    <!-- Loading -->
    <div v-if="loading" class="space-y-3">
      <div v-for="i in 3" :key="i" class="h-12 rounded-lg bg-muted animate-pulse" />
    </div>

    <!-- Error -->
    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">{{ error }}</div>

    <!-- Table -->
    <div v-if="!loading && filteredGroups.length" class="rounded-lg border overflow-hidden">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b bg-muted/50">
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Group</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Description</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Members</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">State</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Created</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">ID</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="g in filteredGroups" :key="g.id"
            class="border-b last:border-0 cursor-pointer hover:bg-muted/50 transition-colors"
            @click="$router.push(`/groups/${g.id}`)"
          >
            <td class="p-4">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-8 h-8 rounded-lg bg-primary text-primary-foreground text-xs font-semibold">
                  {{ (g.name || '?')[0].toUpperCase() }}
                </div>
                <span class="font-medium">{{ g.name }}</span>
              </div>
            </td>
            <td class="p-4 text-muted-foreground truncate max-w-[200px]">{{ g.description || '—' }}</td>
            <td class="p-4">
              <Badge variant="secondary">{{ g.member_count }}</Badge>
            </td>
            <td class="p-4">
              <Badge :variant="g.state === 'active' ? 'default' : 'destructive'" class="capitalize">
                {{ g.state || 'active' }}
              </Badge>
            </td>
            <td class="p-4 text-muted-foreground text-xs">{{ formatDate(g.created_at) }}</td>
            <td class="p-4 font-mono text-xs text-muted-foreground">{{ g.id }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Empty -->
    <Empty v-if="!loading && !error && !filteredGroups.length">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <UsersRound />
        </EmptyMedia>
        <EmptyTitle>{{ searchQuery ? 'No Results' : 'No Groups Yet' }}</EmptyTitle>
        <EmptyDescription>
          {{ searchQuery ? 'No groups match your search.' : 'Create your first group to organize user access.' }}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent v-if="!searchQuery">
        <Button @click="showCreate = true">
          <Plus class="mr-2 size-4" />
          New Group
        </Button>
      </EmptyContent>
    </Empty>

    <!-- Create Dialog -->
    <Dialog v-model:open="showCreate">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>Create Group</DialogTitle>
        </DialogHeader>
        <div class="space-y-3 py-2">
          <div>
            <label class="text-sm font-medium">Name</label>
            <Input v-model="newGroup.name" placeholder="engineering" class="mt-1" />
          </div>
          <div>
            <label class="text-sm font-medium">Description</label>
            <Input v-model="newGroup.description" placeholder="Engineering team members" class="mt-1" />
          </div>
        </div>
        <div class="flex justify-end gap-2 pt-2">
          <Button variant="outline" @click="showCreate = false">Cancel</Button>
          <Button @click="createGroup" :disabled="!newGroup.name">
            <Plus class="size-3.5 mr-1" /> Create
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, reactive } from 'vue'
import { groupApi, type Group } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Plus, Search, UsersRound } from 'lucide-vue-next'

const groups = ref<Group[]>([])
const loading = ref(false)
const error = ref('')
const searchQuery = ref('')
const showCreate = ref(false)
const newGroup = reactive({ name: '', description: '' })

const filteredGroups = computed(() => {
  if (!searchQuery.value.trim()) return groups.value
  const q = searchQuery.value.toLowerCase()
  return groups.value.filter(g =>
    (g.name || '').toLowerCase().includes(q) ||
    (g.description || '').toLowerCase().includes(q) ||
    (g.id || '').toLowerCase().includes(q)
  )
})

function formatDate(ts?: string): string {
  if (!ts) return '—'
  try {
    return new Date(ts).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
  } catch {
    return ts
  }
}

async function fetchGroups() {
  loading.value = true
  try {
    groups.value = await groupApi.list()
  } catch (e: any) {
    error.value = e?.message || 'Failed to load groups'
  } finally {
    loading.value = false
  }
}

async function createGroup() {
  try {
    await groupApi.create({ name: newGroup.name, description: newGroup.description })
    toast.success('Group created')
    showCreate.value = false
    newGroup.name = ''
    newGroup.description = ''
    await fetchGroups()
  } catch (err: any) {
    toast.error('Failed to create group', { description: err.message })
  }
}

onMounted(fetchGroups)
</script>
