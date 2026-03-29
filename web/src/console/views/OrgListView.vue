<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Organizations</h1>
        <p class="text-sm text-muted-foreground">{{ loading ? 'Loading…' : `${orgs.length} organization${orgs.length !== 1 ? 's' : ''}` }}</p>
      </div>
      <Button as-child>
        <router-link to="/orgs/new">
          <Plus class="mr-2 size-4" />
          New Organization
        </router-link>
      </Button>
    </div>

    <!-- Search -->
    <div class="relative w-full max-w-md">
      <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
      <Input
        v-model="searchQuery"
        placeholder="Search organizations…"
        class="pl-9"
      />
    </div>

    <!-- Loading -->
    <div v-if="loading" class="space-y-3">
      <div v-for="i in 3" :key="i" class="h-12 rounded-lg bg-muted animate-pulse" />
    </div>

    <!-- Error -->
    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">{{ error }}</div>

    <!-- Table -->
    <div v-if="!loading && filteredOrgs.length" class="rounded-lg border overflow-hidden">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b bg-muted/50">
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Organization</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">State</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Created</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">ID</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="org in filteredOrgs"
            :key="org.id"
            class="border-b last:border-0 cursor-pointer hover:bg-muted/50 transition-colors"
            @click="$router.push(`/orgs/${org.id}`)"
          >
            <td class="p-4">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-8 h-8 rounded-lg bg-primary text-primary-foreground text-xs font-semibold">
                  {{ (org.name || '?')[0].toUpperCase() }}
                </div>
                <span class="font-medium">{{ org.name }}</span>
              </div>
            </td>
            <td class="p-4">
              <Badge :variant="org.state === 'active' ? 'default' : 'destructive'" class="capitalize">
                {{ org.state || 'active' }}
              </Badge>
            </td>
            <td class="p-4 text-muted-foreground text-xs">{{ formatDate(org.created_at) }}</td>
            <td class="p-4 font-mono text-xs text-muted-foreground">{{ org.id }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Empty -->
    <div v-if="!loading && !error && !filteredOrgs.length" class="flex flex-col items-center justify-center py-16 text-center">
      <div class="text-4xl mb-3 opacity-50">🏢</div>
      <p class="text-sm font-medium">{{ searchQuery ? 'No organizations match your search.' : 'No organizations yet.' }}</p>
      <p v-if="!searchQuery" class="text-xs text-muted-foreground mt-1">Create your first organization to get started.</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { orgApi, type Org } from '@/api/resources'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Plus, Search } from 'lucide-vue-next'

const orgs = ref<Org[]>([])
const loading = ref(false)
const error = ref('')
const searchQuery = ref('')

const filteredOrgs = computed(() => {
  if (!searchQuery.value.trim()) return orgs.value
  const q = searchQuery.value.toLowerCase()
  return orgs.value.filter(o =>
    (o.name || '').toLowerCase().includes(q) ||
    (o.id || '').toLowerCase().includes(q)
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

onMounted(async () => {
  loading.value = true
  try {
    orgs.value = await orgApi.list()
  } catch (e: any) {
    error.value = e?.message || 'Failed to load organizations'
  } finally {
    loading.value = false
  }
})
</script>
