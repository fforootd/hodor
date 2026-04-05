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
          <RouterLink to="/traces" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
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
          <RouterLink to="/users" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
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
          <RouterLink to="/sessions" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
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
          <RouterLink to="/applications" class="text-[11px] text-primary hover:underline">View all &rarr;</RouterLink>
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
import { Activity, Users, KeyRound, Shield } from 'lucide-vue-next'
import { api } from '@/api/client'

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

  return buckets.map(count => {
    if (count === 0) return 0;
    return Math.max(5, Math.round((count / maxCount) * 100));
  });
}

function rangeToHours(range: string): number {
  if (range === '1h') return 1;
  if (range === '24h') return 24;
  if (range === '7d') return 24 * 7;
  if (range === '30d') return 24 * 30;
  return 12;
}

async function fetchData() {
  const hours = rangeToHours(timeRange.value);
  const startMs = Date.now() - hours * 3600000;
  const endMs = Date.now();
  const bucketCount = timeRange.value === '1h' ? 12 : timeRange.value === '24h' ? 24 : timeRange.value === '7d' ? 14 : timeRange.value === '30d' ? 30 : 12;

  try {
    const data = await api.get<any>(`/v1/observability/overview?range=${timeRange.value}`);

    const computeChange = (cur: number, prev: number) => {
      if (prev === 0 && cur === 0) return 0;
      if (prev === 0) return 100;
      return Math.round(((cur - prev) / prev) * 100);
    }

    const m = data.metrics;
    metrics.value[0].value = m.auth.current;
    metrics.value[0].change = computeChange(m.auth.current, m.auth.previous);
    metrics.value[0].sparkline = generateBuckets(m.auth.timestamps || [], bucketCount, startMs, endMs);

    metrics.value[1].value = m.sessions.current;
    metrics.value[1].change = computeChange(m.sessions.current, m.sessions.previous);
    metrics.value[1].sparkline = generateBuckets(m.sessions.timestamps || [], bucketCount, startMs, endMs);

    metrics.value[2].value = m.tokens.current;
    metrics.value[2].change = computeChange(m.tokens.current, m.tokens.previous);
    metrics.value[2].sparkline = generateBuckets(m.tokens.timestamps || [], bucketCount, startMs, endMs);

    metrics.value[3].value = m.failed.current;
    metrics.value[3].change = computeChange(m.failed.current, m.failed.previous);
    metrics.value[3].sparkline = generateBuckets(m.failed.timestamps || [], bucketCount, startMs, endMs);

    const b = data.breakdowns;
    topOperations.value = b.operations || [];
    topUsers.value = b.users || [];
    topIps.value = b.ips || [];
    topClients.value = b.clients || [];
    topSdks.value = b.sdks || [];
    delegationData.value = b.delegation || [];

  } catch (err) {
    console.error("Failed to load overview data:", err)
  }
}

watch(timeRange, fetchData)

onMounted(fetchData)
</script>
