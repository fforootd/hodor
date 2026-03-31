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

    <!-- Reports Grid -->
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-2">
      <!-- Top Operations -->
      <Card class="overflow-hidden border-muted">
        <CardHeader class="flex flex-row items-center justify-between py-3 border-b bg-muted/30">
          <CardTitle class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Top Operations</CardTitle>
          <RouterLink to="/console/traces" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
        </CardHeader>
        <CardContent class="p-0">
          <div v-for="(item, i) in topOperations" :key="i" class="flex items-center justify-between p-3 border-b border-border/40 hover:bg-muted/20 transition-colors group">
            <span class="text-xs font-mono truncate pr-4 text-foreground/80 group-hover:text-foreground">{{ item.name }}</span>
            <div class="flex items-center gap-3 w-1/3 justify-end shrink-0">
              <span class="text-xs font-medium">{{ formatNumber(item.count) }}</span>
              <div class="w-20 lg:w-24 h-1.5 bg-muted/50 rounded-full overflow-hidden flex justify-start"><div class="h-full bg-blue-500 rounded-full" :style="{ width: `${(item.count / maxOperationCount) * 100}%` }"></div></div>
            </div>
          </div>
          <div v-if="!topOperations.length" class="p-8 text-center text-xs text-muted-foreground">No operations recorded</div>
        </CardContent>
      </Card>

      <!-- Top Users -->
      <Card class="overflow-hidden border-muted">
        <CardHeader class="flex flex-row items-center justify-between py-3 border-b bg-muted/30">
          <CardTitle class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Top Users</CardTitle>
          <RouterLink to="/console/users" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
        </CardHeader>
        <CardContent class="p-0">
          <div v-for="(item, i) in topUsers" :key="i" class="flex items-center justify-between p-3 border-b border-border/40 hover:bg-muted/20 transition-colors group">
            <span class="text-xs font-mono truncate pr-4 text-foreground/80 group-hover:text-foreground" :title="item.name">{{ item.name }}</span>
            <div class="flex items-center gap-3 w-1/3 justify-end shrink-0">
              <span class="text-xs font-medium">{{ formatNumber(item.count) }}</span>
              <div class="w-20 lg:w-24 h-1.5 bg-muted/50 rounded-full overflow-hidden flex justify-start"><div class="h-full bg-blue-500 rounded-full" :style="{ width: `${(item.count / maxUserCount) * 100}%` }"></div></div>
            </div>
          </div>
          <div v-if="!topUsers.length" class="p-8 text-center text-xs text-muted-foreground">No user activity recorded</div>
        </CardContent>
      </Card>

      <!-- Top IPs -->
      <Card class="overflow-hidden border-muted">
        <CardHeader class="flex flex-row items-center justify-between py-3 border-b bg-muted/30">
          <CardTitle class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Top IPs</CardTitle>
          <RouterLink to="/console/sessions" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
        </CardHeader>
        <CardContent class="p-0">
          <div v-for="(item, i) in topIps" :key="i" class="flex items-center justify-between p-3 border-b border-border/40 hover:bg-muted/20 transition-colors group">
            <span class="text-xs font-mono truncate pr-4 text-foreground/80 group-hover:text-foreground">{{ item.name }}</span>
            <div class="flex items-center gap-3 w-1/3 justify-end shrink-0">
              <span class="text-xs font-medium">{{ formatNumber(item.count) }}</span>
              <div class="w-20 lg:w-24 h-1.5 bg-muted/50 rounded-full overflow-hidden flex justify-start"><div class="h-full bg-blue-500 rounded-full" :style="{ width: `${(item.count / maxIpCount) * 100}%` }"></div></div>
            </div>
          </div>
          <div v-if="!topIps.length" class="p-8 text-center text-xs text-muted-foreground">No IP data recorded</div>
        </CardContent>
      </Card>

      <!-- Delegation Distribution -->
      <Card class="overflow-hidden border-muted">
        <CardHeader class="flex flex-row items-center justify-between py-3 border-b bg-muted/30">
          <CardTitle class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Delegation Distribution</CardTitle>
        </CardHeader>
        <CardContent class="p-0">
          <div v-if="delegationData.length" class="p-4">
            <div class="flex items-center justify-center gap-6 mb-4">
              <div v-for="item in delegationData" :key="item.name" class="flex items-center gap-2">
                <div class="w-3 h-3 rounded-full" :style="{ background: delegationColor(item.name) }"></div>
                <span class="text-xs">{{ item.name }}</span>
                <span class="text-xs font-medium text-muted-foreground">{{ item.count }}</span>
              </div>
            </div>
            <!-- Simple horizontal stacked bar -->
            <div class="w-full h-8 rounded-md overflow-hidden flex">
              <div
v-for="item in delegationData" :key="item.name"
                   class="h-full transition-all flex items-center justify-center text-[9px] font-semibold text-white"
                   :style="{ width: `${(item.count / totalDelegation) * 100}%`, background: delegationColor(item.name) }">
                {{ Math.round((item.count / totalDelegation) * 100) }}%
              </div>
            </div>
          </div>
          <div v-else class="p-8 text-center text-xs text-muted-foreground flex flex-col items-center">
            No delegation data recorded
          </div>
        </CardContent>
      </Card>

      <!-- Top Clients/Apps -->
      <Card class="overflow-hidden border-muted">
        <CardHeader class="flex flex-row items-center justify-between py-3 border-b bg-muted/30">
          <CardTitle class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Top Clients / Apps</CardTitle>
          <RouterLink to="/console/applications" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
        </CardHeader>
        <CardContent class="p-0">
          <div v-for="(item, i) in topClients" :key="i" class="flex items-center justify-between p-3 border-b border-border/40 hover:bg-muted/20 transition-colors group">
            <span class="text-xs font-mono truncate pr-4 text-foreground/80 group-hover:text-foreground">{{ item.name }}</span>
            <div class="flex items-center gap-3 w-1/3 justify-end shrink-0">
              <span class="text-xs font-medium">{{ formatNumber(item.count) }}</span>
              <div class="w-20 lg:w-24 h-1.5 bg-muted/50 rounded-full overflow-hidden flex justify-start"><div class="h-full bg-violet-500 rounded-full" :style="{ width: `${(item.count / maxClientCount) * 100}%` }"></div></div>
            </div>
          </div>
          <div v-if="!topClients.length" class="p-8 text-center text-xs text-muted-foreground">No client data recorded</div>
        </CardContent>
      </Card>

      <!-- Top SDKs -->
      <Card class="overflow-hidden border-muted">
        <CardHeader class="flex flex-row items-center justify-between py-3 border-b bg-muted/30">
          <CardTitle class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Top SDKs</CardTitle>
        </CardHeader>
        <CardContent class="p-0">
          <div v-for="(item, i) in topSdks" :key="i" class="flex items-center justify-between p-3 border-b border-border/40 hover:bg-muted/20 transition-colors group">
            <span class="text-xs font-mono truncate pr-4 text-foreground/80 group-hover:text-foreground">{{ item.name }}</span>
            <div class="flex items-center gap-3 w-1/3 justify-end shrink-0">
              <span class="text-xs font-medium">{{ formatNumber(item.count) }}</span>
              <div class="w-20 lg:w-24 h-1.5 bg-muted/50 rounded-full overflow-hidden flex justify-start"><div class="h-full bg-teal-500 rounded-full" :style="{ width: `${(item.count / maxSdkCount) * 100}%` }"></div></div>
            </div>
          </div>
          <div v-if="!topSdks.length" class="p-8 text-center text-xs text-muted-foreground">No SDK data recorded</div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>
<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Activity, Users, KeyRound, Shield, Search } from 'lucide-vue-next'
import { api } from '@/api/client'
import { RouterLink } from 'vue-router'

const timeRange = ref('12h')

// Generate realistic sparkline data
function sparkline(count: number = 12): number[] {
  return Array.from({ length: count }, () => 20 + Math.random() * 80)
}

const metrics = ref([
  { label: 'Auth Requests', value: 0, change: 0, icon: Activity, sparkline: sparkline() },
  { label: 'Active Sessions', value: 0, change: 0, icon: Users, sparkline: sparkline() },
  { label: 'Token Issuances', value: 0, change: 0, icon: KeyRound, sparkline: sparkline() },
  { label: 'Failed Logins', value: 0, change: 0, icon: Shield, sparkline: sparkline() },
])

interface ReportItem { name: string; count: number }

const topOperations = ref<ReportItem[]>([])
const topUsers = ref<ReportItem[]>([])
const topIps = ref<ReportItem[]>([])
const topClients = ref<ReportItem[]>([])
const topSdks = ref<ReportItem[]>([])
const delegationData = ref<ReportItem[]>([])

const maxOperationCount = computed(() => Math.max(...topOperations.value.map(o => o.count), 1))
const maxUserCount = computed(() => Math.max(...topUsers.value.map(o => o.count), 1))
const maxIpCount = computed(() => Math.max(...topIps.value.map(o => o.count), 1))
const maxClientCount = computed(() => Math.max(...topClients.value.map(o => o.count), 1))
const maxSdkCount = computed(() => Math.max(...topSdks.value.map(o => o.count), 1))
const totalDelegation = computed(() => delegationData.value.reduce((sum, d) => sum + d.count, 0) || 1)

function delegationColor(name: string): string {
  switch (name.toLowerCase()) {
    case 'direct': return '#22c55e'
    case 'delegated': return '#f59e0b'
    case 'pat_shared': return '#ef4444'
    case 'exchanged': return '#3b82f6'
    default: return '#8b5cf6'
  }
}

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
    // Zipped extraction to support arrays
    const rawVal = Array.isArray(r) ? r[0] : Object.values(r)[0];
    return Number(rawVal || 0);
  } catch {
    return 0;
  }
}

async function fetchTimestamps(sql: string): Promise<number[]> {
  try {
    const res = await api.post<any>('/v1/analytics/query', { sql, limit: 10000 });
    if (res.error || !res.rows) return [];
    return res.rows.map((r: any) => {
      const ts = Array.isArray(r) ? r[0] : Object.values(r)[0];
      return new Date(ts).getTime();
    });
  } catch {
    return [];
  }
}

function generateBuckets(timestamps: number[], bucketCount: number, startMs: number, endMs: number): number[] {
  const buckets = new Array(bucketCount).fill(0);
  const interval = (endMs - startMs) / bucketCount;
  if (interval <= 0) return buckets;

  let maxCount = 0;
  for (const ts of timestamps) {
    if (ts < startMs || ts > endMs) continue;
    const idx = Math.min(bucketCount - 1, Math.floor((ts - startMs) / interval));
    buckets[idx]++;
    if (buckets[idx] > maxCount) maxCount = buckets[idx];
  }

  // Convert to percentage (min 5% for visibility if > 0)
  return buckets.map(count => {
    if (count === 0) return 0;
    return Math.max(5, Math.round((count / maxCount) * 100));
  });
}

async function fetchData() {
  const curTime = getThreshold(timeRange.value, 1)
  const prevTime = getThreshold(timeRange.value, 2)
  const startMs = new Date(curTime).getTime()
  const endMs = new Date().getTime()
  const bucketCount = timeRange.value === '1h' ? 12 : timeRange.value === '24h' ? 24 : timeRange.value === '7d' ? 14 : timeRange.value === '30d' ? 30 : 12;
  
  try {
    const now = new Date().toISOString().replace('T', ' ').slice(0, 19);

    const [
      authPrev,
      sessPrev,
      tokPrev,
      failPrev,
      authTs,
      sessTs,
      tokTs,
      failTs,
      opsRes, usersRes, ipsRes, clientsRes, sdksRes, delegationRes
    ] = await Promise.all([
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type LIKE 'auth.%' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      fetchCount(`SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL AND expires_at > '${now}' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type = 'auth.token_issued' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      fetchCount(`SELECT COUNT(*) FROM events WHERE event_type = 'auth.login_failed' AND created_at >= '${prevTime}' AND created_at < '${curTime}'`),
      
      fetchTimestamps(`SELECT created_at FROM events WHERE event_type LIKE 'auth.%' AND created_at >= '${curTime}'`),
      fetchTimestamps(`SELECT created_at FROM sessions WHERE revoked_at IS NULL AND expires_at > '${now}' AND created_at >= '${curTime}'`),
      fetchTimestamps(`SELECT created_at FROM events WHERE event_type = 'auth.token_issued' AND created_at >= '${curTime}'`),
      fetchTimestamps(`SELECT created_at FROM events WHERE event_type = 'auth.login_failed' AND created_at >= '${curTime}'`),
      
      api.post<any>('/v1/analytics/query', { sql: `SELECT event_type, COUNT(*) as count FROM events WHERE created_at >= '${curTime}' AND event_type != '' AND category != 'log' GROUP BY event_type ORDER BY count DESC LIMIT 8` }),
      api.post<any>('/v1/analytics/query', { sql: `SELECT COALESCE(NULLIF(actor_id, ''), 'Anonymous'), COUNT(*) as count FROM events WHERE created_at >= '${curTime}' AND category != 'log' GROUP BY actor_id ORDER BY count DESC LIMIT 8` }),
      api.post<any>('/v1/analytics/query', { sql: `SELECT ip_address, COUNT(*) as count FROM sessions WHERE created_at >= '${curTime}' AND ip_address IS NOT NULL AND ip_address != '' GROUP BY ip_address ORDER BY count DESC LIMIT 8` }),
      api.post<any>('/v1/analytics/query', { sql: `SELECT COALESCE(NULLIF(client_id, ''), 'Console') as name, COUNT(*) as count FROM events WHERE created_at >= '${curTime}' AND category != 'log' GROUP BY client_id ORDER BY count DESC LIMIT 8` }),
      api.post<any>('/v1/analytics/query', { sql: `SELECT COALESCE(NULLIF(sdk_name, ''), 'Browser') as name, COUNT(*) as count FROM events WHERE created_at >= '${curTime}' AND category != 'log' GROUP BY sdk_name ORDER BY count DESC LIMIT 8` }),
      api.post<any>('/v1/analytics/query', { sql: `SELECT COALESCE(NULLIF(delegation_type, ''), 'direct') as type, COUNT(*) as count FROM events WHERE created_at >= '${curTime}' AND category != 'log' GROUP BY delegation_type ORDER BY count DESC` }),
    ]);

    const computeChange = (cur: number, prev: number) => {
      if (prev === 0 && cur === 0) return 0;
      if (prev === 0) return 100;
      return Math.round(((cur - prev) / prev) * 100);
    }

    metrics.value[0].value = authTs.length
    metrics.value[0].change = computeChange(authTs.length, authPrev)
    metrics.value[0].sparkline = generateBuckets(authTs, bucketCount, startMs, endMs)

    metrics.value[1].value = sessTs.length
    metrics.value[1].change = computeChange(sessTs.length, sessPrev)
    metrics.value[1].sparkline = generateBuckets(sessTs, bucketCount, startMs, endMs)

    metrics.value[2].value = tokTs.length
    metrics.value[2].change = computeChange(tokTs.length, tokPrev)
    metrics.value[2].sparkline = generateBuckets(tokTs, bucketCount, startMs, endMs)

    metrics.value[3].value = failTs.length
    metrics.value[3].change = computeChange(failTs.length, failPrev)
    metrics.value[3].sparkline = generateBuckets(failTs, bucketCount, startMs, endMs)

    // Parse analytics response supporting the 2D array Rows structure
    const parseList = (res: any) => {
      if (res && res.rows) {
        return res.rows.map((r: any) => ({
          name: Array.isArray(r) ? r[0] : Object.values(r)[0],
          count: Number(Array.isArray(r) ? r[1] : Object.values(r)[1])
        }));
      }
      return [];
    }
    
    topOperations.value = parseList(opsRes);
    topUsers.value = parseList(usersRes);
    topIps.value = parseList(ipsRes);
    topClients.value = parseList(clientsRes);
    topSdks.value = parseList(sdksRes);
    delegationData.value = parseList(delegationRes);
    
  } catch (err) {
    console.error("Failed to load overview data:", err)
  }
}

watch(timeRange, fetchData)

onMounted(fetchData)
</script>
