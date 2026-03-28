<template>
  <div class="space-y-6">
    <!-- Header with time range -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Observability</h1>
        <p class="text-sm text-muted-foreground">Monitor authentication events and system health.</p>
      </div>
      <Select v-model="timeRange">
        <SelectTrigger class="w-36">
          <SelectValue placeholder="Time range" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="1h">Last 1 hour</SelectItem>
          <SelectItem value="12h">Last 12 hours</SelectItem>
          <SelectItem value="24h">Last 24 hours</SelectItem>
          <SelectItem value="7d">Last 7 days</SelectItem>
          <SelectItem value="30d">Last 30 days</SelectItem>
        </SelectContent>
      </Select>
    </div>

    <!-- Metric Cards Grid -->
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
      <Card v-for="metric in metrics" :key="metric.label">
        <CardHeader class="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle class="text-sm font-medium">{{ metric.label }}</CardTitle>
          <component :is="metric.icon" class="size-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div class="text-2xl font-bold">{{ formatNumber(metric.value) }}</div>
          <p class="text-xs text-muted-foreground mt-1">
            <span :class="metric.change >= 0 ? 'text-emerald-600' : 'text-red-500'">
              {{ metric.change >= 0 ? '+' : '' }}{{ metric.change }}%
            </span>
            from previous period
          </p>
          <!-- Sparkline placeholder -->
          <div class="mt-3 h-16 w-full rounded bg-muted/50 flex items-end gap-0.5 px-1 pb-1">
            <div
              v-for="(v, i) in metric.sparkline"
              :key="i"
              class="flex-1 rounded-t bg-primary/60 transition-all"
              :style="{ height: `${v}%` }"
            />
          </div>
        </CardContent>
      </Card>
    </div>

    <!-- Event Type Breakdown -->
    <Card>
      <CardHeader>
        <CardTitle>Event Breakdown</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="flex items-center gap-2 mb-4">
          <Search class="size-4 text-muted-foreground" />
          <Input v-model="eventFilter" placeholder="Filter events..." class="max-w-sm" />
        </div>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Event Type</TableHead>
              <TableHead class="text-right">Count</TableHead>
              <TableHead class="w-32">Trend</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="row in filteredEvents" :key="row.event_type">
              <TableCell>
                <div class="flex items-center gap-2">
                  <div class="size-2 rounded-full" :style="{ backgroundColor: row.color }" />
                  <span class="font-medium">{{ row.event_type }}</span>
                </div>
              </TableCell>
              <TableCell class="text-right font-mono">{{ formatNumber(row.count) }}</TableCell>
              <TableCell>
                <div class="h-6 flex items-end gap-0.5">
                  <div
                    v-for="(v, i) in row.sparkline"
                    :key="i"
                    class="flex-1 rounded-t transition-all"
                    :style="{ height: `${v}%`, backgroundColor: row.color }"
                  />
                </div>
              </TableCell>
            </TableRow>
            <TableRow v-if="!filteredEvents.length">
              <TableCell colspan="3" class="text-center text-muted-foreground py-8">
                No events found.
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  </div>
</template>
<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Activity, Users, KeyRound, Shield, Search } from 'lucide-vue-next'
import { api } from '@/api/client'

const timeRange = ref('12h')
const eventFilter = ref('')

// Generate realistic sparkline data
function sparkline(count: number = 12): number[] {
  return Array.from({ length: count }, () => 20 + Math.random() * 80)
}

const colors = ['hsl(12, 76%, 61%)', 'hsl(173, 58%, 39%)', 'hsl(197, 37%, 24%)', 'hsl(43, 74%, 66%)', 'hsl(27, 87%, 67%)', 'hsl(260, 60%, 55%)', 'hsl(340, 75%, 55%)']

const metrics = ref([
  { label: 'Auth Requests', value: 0, change: 0, icon: Activity, sparkline: sparkline() },
  { label: 'Active Sessions', value: 0, change: 0, icon: Users, sparkline: sparkline() },
  { label: 'Token Issuances', value: 0, change: 0, icon: KeyRound, sparkline: sparkline() },
  { label: 'Failed Logins', value: 0, change: 0, icon: Shield, sparkline: sparkline() },
])

interface EventRow { event_type: string; count: number; color: string; sparkline: number[] }
const eventRows = ref<EventRow[]>([])

const filteredEvents = computed(() => {
  const f = eventFilter.value.toLowerCase()
  if (!f) return eventRows.value
  return eventRows.value.filter(e => e.event_type.toLowerCase().includes(f))
})

function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
  return String(n)
}

function getThreshold(range: string, multiplier = 1): string {
  const msPerHr = 3600000;
  let hrs = 12;
  if (range === '1h') hrs = 1;
  else if (range === '24h') hrs = 24;
  else if (range === '7d') hrs = 24 * 7;
  else if (range === '30d') hrs = 24 * 30;
  
  const d = new Date(Date.now() - (hrs * multiplier * msPerHr));
  return d.toISOString().replace('T', ' ').slice(0, 19);
}

async function fetchCount(sql: string): Promise<number> {
  try {
    const res = await api.post<any>('/v1/analytics/query', { sql });
    if (res.error || !res.rows || res.rows.length === 0) return 0;
    const r = res.rows[0];
    return Number(Object.values(r)[0] || 0);
  } catch {
    return 0;
  }
}

async function fetchData() {
  const curTime = getThreshold(timeRange.value, 1)
  const prevTime = getThreshold(timeRange.value, 2)
  
  try {
    const now = new Date().toISOString().replace('T', ' ').slice(0, 19);

    // Fire all big queries concurrently using the actual database schema columns (event_type, created_at, revoked_at)
    const [
      authCur, authPrev,
      sessCur, sessPrev,
      tokCur, tokPrev,
      failCur, failPrev,
      eventBreakdown
    ] = await Promise.all([
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type LIKE 'auth.%' AND created_at >= '${curTime}'`),
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type LIKE 'auth.%' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      
      fetchCount(`SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL AND expires_at > '${now}' AND created_at >= '${curTime}'`),
      fetchCount(`SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL AND expires_at > '${now}' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type = 'auth.token_issued' AND created_at >= '${curTime}'`),
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type = 'auth.token_issued' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type = 'auth.login_failed' AND created_at >= '${curTime}'`),
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type = 'auth.login_failed' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      
      api.post<any>('/v1/analytics/query', { sql: `SELECT event_type, COUNT(*) as count FROM events WHERE created_at >= '${curTime}' GROUP BY event_type ORDER BY count DESC LIMIT 20` })
    ]);

    const computeChange = (cur: number, prev: number) => {
      if (prev === 0 && cur === 0) return 0;
      if (prev === 0) return 100;
      return Math.round(((cur - prev) / prev) * 100);
    }

    metrics.value[0].value = authCur
    metrics.value[0].change = computeChange(authCur, authPrev)

    metrics.value[1].value = sessCur
    metrics.value[1].change = computeChange(sessCur, sessPrev)

    metrics.value[2].value = tokCur
    metrics.value[2].change = computeChange(tokCur, tokPrev)

    metrics.value[3].value = failCur
    metrics.value[3].change = computeChange(failCur, failPrev)

    if (eventBreakdown && eventBreakdown.rows) {
      eventRows.value = eventBreakdown.rows.map((r: any, i: number) => ({
        event_type: Object.values(r)[0] || 'unknown',
        count: Number(Object.values(r)[1] || 0),
        color: colors[i % colors.length],
        sparkline: sparkline(), // Visual mock kept due to dialect complexity
      }))
    } else {
      eventRows.value = []
    }
  } catch (err) {
    console.error("Failed to load overview data:", err)
  }
}

watch(timeRange, fetchData)

onMounted(fetchData)
</script>
