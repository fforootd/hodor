<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Projects</h1>
        <p class="text-sm text-muted-foreground">{{ loading ? 'Loading…' : `${projects.length} project${projects.length !== 1 ? 's' : ''}` }}</p>
      </div>
      <Button @click="showCreate = true">
        <Plus class="mr-2 size-4" />
        New Project
      </Button>
    </div>

    <!-- Search -->
    <div class="relative w-full max-w-md">
      <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
      <Input v-model="searchQuery" placeholder="Search projects…" class="pl-9" />
    </div>

    <!-- Loading -->
    <div v-if="loading" class="space-y-3">
      <div v-for="i in 3" :key="i" class="h-12 rounded-lg bg-muted animate-pulse" />
    </div>

    <!-- Error -->
    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">{{ error }}</div>

    <!-- Table -->
    <div v-if="!loading && filteredProjects.length" class="rounded-lg border overflow-hidden">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b bg-muted/50">
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Project</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Description</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Members</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">State</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Created</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">ID</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="p in filteredProjects" :key="p.id"
            class="border-b last:border-0 cursor-pointer hover:bg-muted/50 transition-colors"
            @click="$router.push(`/projects/${p.id}`)"
          >
            <td class="p-4">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-8 h-8 rounded-lg bg-primary text-primary-foreground text-xs font-semibold">
                  {{ (p.name || '?')[0].toUpperCase() }}
                </div>
                <span class="font-medium">{{ p.name }}</span>
              </div>
            </td>
            <td class="p-4 text-muted-foreground truncate max-w-[200px]">{{ p.description || '—' }}</td>
            <td class="p-4">
              <Badge variant="secondary">{{ p.member_count }}</Badge>
            </td>
            <td class="p-4">
              <Badge :variant="p.state === 'active' ? 'default' : 'destructive'" class="capitalize">
                {{ p.state || 'active' }}
              </Badge>
            </td>
            <td class="p-4 text-muted-foreground text-xs">{{ formatDate(p.created_at) }}</td>
            <td class="p-4 font-mono text-xs text-muted-foreground">{{ p.id }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Empty -->
    <Empty v-if="!loading && !error && !filteredProjects.length">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <FolderKanban />
        </EmptyMedia>
        <EmptyTitle>{{ searchQuery ? 'No Results' : 'No Projects Yet' }}</EmptyTitle>
        <EmptyDescription>
          {{ searchQuery ? 'No projects match your search.' : 'Create your first project to organize apps and resources.' }}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent v-if="!searchQuery">
        <Button @click="showCreate = true">
          <Plus class="mr-2 size-4" />
          New Project
        </Button>
      </EmptyContent>
    </Empty>

    <!-- Create Dialog -->
    <Dialog v-model:open="showCreate">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>Create Project</DialogTitle>
        </DialogHeader>
        <div class="space-y-3 py-2">
          <div>
            <label class="text-sm font-medium">Name</label>
            <Input v-model="newProject.name" placeholder="my-app" class="mt-1" />
          </div>
          <div>
            <label class="text-sm font-medium">Description</label>
            <Input v-model="newProject.description" placeholder="Customer-facing web application" class="mt-1" />
          </div>
        </div>
        <div class="flex justify-end gap-2 pt-2">
          <Button variant="outline" @click="showCreate = false">Cancel</Button>
          <Button @click="createProject" :disabled="!newProject.name">
            <Plus class="size-3.5 mr-1" /> Create
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, reactive } from 'vue'
import { projectApi, type Project } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Plus, Search, FolderKanban } from 'lucide-vue-next'

const projects = ref<Project[]>([])
const loading = ref(false)
const error = ref('')
const searchQuery = ref('')
const showCreate = ref(false)
const newProject = reactive({ name: '', description: '' })

const filteredProjects = computed(() => {
  if (!searchQuery.value.trim()) return projects.value
  const q = searchQuery.value.toLowerCase()
  return projects.value.filter(p =>
    (p.name || '').toLowerCase().includes(q) ||
    (p.description || '').toLowerCase().includes(q) ||
    (p.id || '').toLowerCase().includes(q)
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

async function fetchProjects() {
  loading.value = true
  try {
    projects.value = await projectApi.list()
  } catch (e: any) {
    error.value = e?.message || 'Failed to load projects'
  } finally {
    loading.value = false
  }
}

async function createProject() {
  try {
    await projectApi.create({ name: newProject.name, description: newProject.description })
    toast.success('Project created')
    showCreate.value = false
    newProject.name = ''
    newProject.description = ''
    await fetchProjects()
  } catch (err: any) {
    toast.error('Failed to create project', { description: err.message })
  }
}

onMounted(fetchProjects)
</script>
