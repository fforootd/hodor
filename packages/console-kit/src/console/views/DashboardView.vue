<template>
  <div class="space-y-6">
    <!-- Root Instance Picker (no instance selected) -->
    <template v-if="isRoot && !hasInstance">
      <div class="flex flex-col items-center justify-center py-16">
        <div class="flex size-16 items-center justify-center rounded-xl border bg-muted mb-6">
          <LayoutGrid class="size-8 text-muted-foreground" />
        </div>
        <h1 class="text-2xl font-semibold tracking-tight">Continue to Overview</h1>
        <p class="text-sm text-muted-foreground mt-1">Choose an instance to continue</p>
      </div>

      <div class="mx-auto max-w-lg space-y-3">
        <!-- Search -->
        <div class="relative">
          <Search class="absolute left-3 top-3 size-4 text-muted-foreground" />
          <Input
            v-model="instanceSearch"
            placeholder="Find Instance..."
            class="pl-9"
          />
        </div>

        <!-- Instance list -->
        <Card>
          <div class="divide-y">
            <button
              v-for="inst in filteredInstances"
              :key="inst.instance_id"
              type="button"
              class="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-accent transition-colors"
              @click="pickInstance(inst)"
            >
              <Server class="size-5 text-muted-foreground shrink-0" />
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-medium truncate">{{ inst.primary_domain || inst.instance_id }}</span>
                  <Badge variant="secondary" class="text-[10px] shrink-0">{{ stateLabel(inst.state) }}</Badge>
                </div>
                <span class="text-xs text-muted-foreground truncate block">{{ inst.primary_domain || inst.instance_id }}</span>
              </div>
            </button>

            <div v-if="filteredInstances.length === 0 && !loading" class="px-4 py-8 text-center text-sm text-muted-foreground">
              {{ instanceSearch ? 'No instances match your search.' : 'No instances yet.' }}
            </div>

            <!-- Add Instance -->
            <router-link
              to="/instances/new"
              class="flex w-full items-center gap-3 px-4 py-3 text-left text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <Plus class="size-5 shrink-0" />
              <span>Add Instance</span>
            </router-link>
          </div>
        </Card>
      </div>
    </template>

    <!-- Product Dashboard (instance selected or non-root) -->
    <template v-else>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Dashboard</h1>
        <p class="text-sm text-muted-foreground">Welcome to Zitadel Console.</p>
      </div>

      <!-- Quick Stats -->
      <div class="grid gap-4 md:grid-cols-3 lg:grid-cols-6">
        <Card v-for="stat in stats" :key="stat.label">
          <CardHeader class="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle class="text-sm font-medium">{{ stat.label }}</CardTitle>
            <component :is="stat.icon" class="size-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div class="text-2xl font-bold">{{ stat.value }}</div>
            <p class="text-xs text-muted-foreground">{{ stat.description }}</p>
          </CardContent>
        </Card>
      </div>

      <!-- Recent Events -->
      <Card>
        <CardHeader>
          <CardTitle>Recent Events</CardTitle>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Type</TableHead>
                <TableHead>Subject</TableHead>
                <TableHead>Time</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="event in recentEvents" :key="event.id">
                <TableCell>
                  <Badge variant="outline" class="font-mono text-xs">{{ event.event_type }}</Badge>
                </TableCell>
                <TableCell class="text-sm">{{ event.subject || '—' }}</TableCell>
                <TableCell class="text-sm text-muted-foreground">{{ event.time_ago }}</TableCell>
              </TableRow>
              <TableRow v-if="!recentEvents.length">
                <TableCell colspan="3" class="text-center text-muted-foreground py-8">
                  <Activity class="mx-auto size-8 mb-2 opacity-40" />
                  No recent events
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, markRaw } from 'vue'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Users, Building2, AppWindow, FileJson, Globe, Activity, Server, Plus, Search, LayoutGrid } from 'lucide-vue-next'
import { api, getInstanceContext } from '@/api/client'
import { useInstanceContext } from '@/console/composables/useInstanceContext'
import { countsApi, schemaApi, providerApi, eventApi, orgApi, instanceApi, type Instance } from '@/api/resources'
import { useRouter } from 'vue-router'

const router = useRouter()
const { currentInstanceId, setInstance } = useInstanceContext()

const isRoot = ref(false)
const hasInstance = computed(() => !!currentInstanceId.value)
const loading = ref(true)

// Instance picker
const instances = ref<Instance[]>([])
const instanceSearch = ref('')

const stateLabels: Record<string, string> = {
  active: 'active',
  provisioning: 'setting up',
  deprovisioning: 'removing',
  suspended: 'suspended',
}
function stateLabel(state: string) { return stateLabels[state] || state }

const filteredInstances = computed(() => {
  if (!instanceSearch.value.trim()) return instances.value
  const q = instanceSearch.value.toLowerCase()
  return instances.value.filter(i =>
    (i.primary_domain || '').toLowerCase().includes(q) ||
    i.instance_id.toLowerCase().includes(q),
  )
})

function pickInstance(inst: Instance) {
  setInstance(inst.instance_id, inst.primary_domain || inst.instance_id)
  router.push(`/instances/${inst.instance_id}`)
}

// Product dashboard
const stats = ref([
  { label: 'Users', value: '—', icon: markRaw(Users), description: 'Total users' },
  { label: 'Organizations', value: '—', icon: markRaw(Building2), description: 'Active orgs' },
  { label: 'Applications', value: '—', icon: markRaw(AppWindow), description: 'Registered apps' },
  { label: 'Schemas', value: '—', icon: markRaw(FileJson), description: 'Active schemas' },
  { label: 'Providers', value: '—', icon: markRaw(Globe), description: 'Configured providers' },
  { label: 'Events', value: '—', icon: markRaw(Activity), description: 'Last 1 hour' },
])

const recentEvents = ref<any[]>([])

function timeAgo(ts: string): string {
  const d = Date.now() - new Date(ts).getTime()
  if (d < 60000) return 'just now'
  if (d < 3600000) return `${Math.floor(d / 60000)}m ago`
  if (d < 86400000) return `${Math.floor(d / 3600000)}h ago`
  return `${Math.floor(d / 86400000)}d ago`
}

onMounted(async () => {
  // Detect root mode from bootstrap.
  try {
    const bootstrap = await api.get<{ instance?: { is_root?: boolean } }>('/v1/console/bootstrap')
    isRoot.value = bootstrap.instance?.is_root ?? false
  } catch {
    isRoot.value = false
  }

  if (isRoot.value && !hasInstance.value) {
    // Load instance list for the picker.
    try {
      const res = await instanceApi.list({ limit: 100 })
      instances.value = res.items ?? []
    } catch { /* empty */ }
    loading.value = false
  } else {
    // Product dashboard data.
    loading.value = false
    try {
      const [counts, orgs, schemas, providers, events] = await Promise.allSettled([
        countsApi.get(),
        orgApi.list(),
        schemaApi.list(),
        providerApi.list(),
        eventApi.list({ limit: 10 }),
      ])

      if (counts.status === 'fulfilled') {
        const c = counts.value as Record<string, number>
        const userTotal = (c.human_user ?? 0) + (c.service_user ?? 0) + (c.ai_agent ?? 0)
        stats.value[0].value = String(userTotal || 0)
        stats.value[2].value = String(c.app ?? 0)
      }
      if (orgs.status === 'fulfilled') {
        stats.value[1].value = String(Array.isArray(orgs.value) ? orgs.value.length : 0)
      }
      if (schemas.status === 'fulfilled') stats.value[3].value = String(schemas.value.length ?? 0)
      if (providers.status === 'fulfilled') stats.value[4].value = String(providers.value.length ?? 0)
      if (events.status === 'fulfilled') {
        const items = events.value || []
        const oneHourAgo = Date.now() - 3600000
        const recentCount = items.filter((e: any) => new Date(e.created_at).getTime() > oneHourAgo).length
        stats.value[5].value = String(recentCount)
        recentEvents.value = items.slice(0, 10).map((e: any) => ({
          id: e.id,
          event_type: e.event_type,
          subject: e.identity_identifier || e.aggregate_id,
          time_ago: timeAgo(e.created_at),
        }))
      }
    } catch (err) { console.warn('Dashboard load failed:', err) }
  }
})
</script>
