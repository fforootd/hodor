<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Dashboard</h1>
      <p class="text-sm text-muted-foreground">Welcome to Zitadel Console.</p>
    </div>

    <!-- Quick Stats -->
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Users, FileJson, Globe, Activity } from 'lucide-vue-next'
import { api } from '@/api/client'

const stats = ref([
  { label: 'Entities', value: '—', icon: Users, description: 'Total entities' },
  { label: 'Schemas', value: '—', icon: FileJson, description: 'Active schemas' },
  { label: 'Providers', value: '—', icon: Globe, description: 'Configured providers' },
  { label: 'Events', value: '—', icon: Activity, description: 'Last 24 hours' },
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
  try {
    const [identities, schemas, providers, events] = await Promise.allSettled([
      api.get<any>('/v1/identities?limit=0'),
      api.get<any>('/v1/schemas'),
      api.get<any>('/v1/providers'),
      api.get<any>('/v1/events?limit=10&order=desc'),
    ])

    if (identities.status === 'fulfilled') stats.value[0].value = String(identities.value.total ?? identities.value.items?.length ?? 0)
    if (schemas.status === 'fulfilled') stats.value[1].value = String(schemas.value.items?.length ?? 0)
    if (providers.status === 'fulfilled') stats.value[2].value = String(providers.value.items?.length ?? 0)
    if (events.status === 'fulfilled') {
      const items = events.value.items || []
      stats.value[3].value = String(events.value.total ?? items.length)
      recentEvents.value = items.slice(0, 10).map((e: any) => ({
        id: e.id,
        event_type: e.event_type,
        subject: e.identity_identifier || e.aggregate_id,
        time_ago: timeAgo(e.created_at),
      }))
    }
  } catch { /* demo fallback */ }
})
</script>
