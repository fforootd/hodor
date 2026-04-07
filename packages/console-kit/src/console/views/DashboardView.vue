<template>
  <div class="space-y-6">
    <!-- Getting Started (root, no instance selected) -->
    <template v-if="isRoot && !hasInstance">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Getting Started</h1>
        <p class="text-sm text-muted-foreground mt-1">New to ZITADEL? Try our onboarding guide to get started.</p>
      </div>

      <!-- Start Building -->
      <Card class="bg-muted/30">
        <CardContent class="pt-6">
          <h2 class="text-lg font-semibold mb-1">Start Building</h2>
          <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
            <div>
              <p class="text-sm font-medium">Integrate ZITADEL into your application</p>
              <p class="text-xs text-muted-foreground mt-0.5">Connect your app or use one of our templates to get started in minutes.</p>
            </div>
            <div class="flex gap-2 shrink-0">
              <Button size="sm" @click="router.push('/applications/new')">Create Application</Button>
              <Button variant="outline" size="sm" as-child>
                <a href="https://zitadel.com/docs/guides/start/quickstart" target="_blank" rel="noopener noreferrer">Learn More</a>
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- Your Next Steps -->
      <div>
        <h3 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-3">Your Next Steps</h3>
        <div class="grid gap-4 md:grid-cols-3">
          <Card v-for="step in nextSteps" :key="step.title" class="cursor-pointer hover:border-primary/50 transition-colors" @click="step.action">
            <CardContent class="pt-5">
              <div class="flex items-start gap-3">
                <div class="flex size-8 items-center justify-center rounded-lg" :class="step.iconBg">
                  <component :is="step.icon" class="size-4 text-white" />
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium">{{ step.title }}</p>
                  <p class="text-xs text-muted-foreground mt-0.5">{{ step.description }}</p>
                  <button class="text-xs text-primary mt-2 hover:underline">{{ step.linkLabel }} →</button>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>

      <!-- Developer Tools -->
      <div>
        <h3 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-3">Developer Tools</h3>
        <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
          <a
            v-for="tool in devTools"
            :key="tool.title"
            :href="tool.href"
            target="_blank"
            rel="noopener noreferrer"
            class="flex items-start gap-3 rounded-lg border p-3.5 transition-colors hover:bg-accent"
          >
            <component :is="tool.icon" class="size-4 text-muted-foreground shrink-0 mt-0.5" />
            <div class="min-w-0">
              <p class="text-sm font-medium">{{ tool.title }}</p>
              <p class="text-xs text-muted-foreground">{{ tool.description }}</p>
            </div>
          </a>
        </div>
      </div>

      <!-- Dismiss hint -->
      <p class="text-xs text-muted-foreground text-center pt-4">
        I'm done with this setup guide.
        <button class="text-primary hover:underline" @click="router.push('/instances')">Hide this →</button>
      </p>
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
import { Button } from '@/components/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import {
  Users, Building2, AppWindow, FileJson, Globe, Activity,
  LogIn, FolderKanban, BookOpen, Code2, Terminal, Rocket, MessageCircle,
} from 'lucide-vue-next'
import { api } from '@/api/client'
import { useInstanceContext } from '@/console/composables/useInstanceContext'
import { countsApi, schemaApi, providerApi, eventApi, orgApi } from '@/api/resources'
import { useRouter } from 'vue-router'

const router = useRouter()
const { currentInstanceId } = useInstanceContext()

const isRoot = ref(false)
const hasInstance = computed(() => !!currentInstanceId.value)

const nextSteps = [
  {
    title: 'Log in to your application',
    description: 'Integrate your application with ZITADEL for authentication and test by logging in with your admin user.',
    linkLabel: 'Log in',
    icon: markRaw(LogIn),
    iconBg: 'bg-rose-500',
    action: () => window.open('https://zitadel.com/docs/guides/start/quickstart', '_blank'),
  },
  {
    title: 'Create a project',
    description: 'Add a project to your portfolio to manage roles and role assignments.',
    linkLabel: 'Create project',
    icon: markRaw(FolderKanban),
    iconBg: 'bg-amber-500',
    action: () => router.push('/projects/new'),
  },
  {
    title: 'Register your application',
    description: 'Register your web, native, or API application and setup authentication.',
    linkLabel: 'Register application',
    icon: markRaw(AppWindow),
    iconBg: 'bg-emerald-500',
    action: () => router.push('/applications/new'),
  },
]

const devTools = [
  { title: 'ZITADEL Community', description: 'Chat and support', icon: markRaw(MessageCircle), href: 'https://zitadel.com/chat' },
  { title: 'API Reference', description: "Explore ZITADEL's APIs", icon: markRaw(Code2), href: 'https://zitadel.com/docs/apis' },
  { title: 'Example Projects', description: 'Pre-built authentication examples', icon: markRaw(Rocket), href: 'https://github.com/zitadel/examples' },
  { title: 'Documentation', description: 'Comprehensive guides and tutorials', icon: markRaw(BookOpen), href: 'https://zitadel.com/docs' },
  { title: 'ZITADEL CLI', description: 'Manage configuration from terminal', icon: markRaw(Terminal), href: 'https://zitadel.com/docs/guides/manage/cli' },
  { title: 'Quick Start Guides', description: 'Get up and running fast', icon: markRaw(Globe), href: 'https://zitadel.com/docs/guides/start/quickstart' },
]
const loading = ref(true)

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
    // Getting started page — no data loading needed.
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
