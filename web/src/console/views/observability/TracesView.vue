<template>
  <!-- eslint-disable vue/valid-v-for -->
  <div class="space-y-6 pb-12">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Traces</h1>
        <p class="text-sm text-muted-foreground mt-1">Identity activity traces — who did what, and when.</p>
      </div>
      <div class="flex items-center gap-2">
        <Select v-model="groupBy">
          <SelectTrigger class="w-36">
            <SelectValue placeholder="Group by" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="trace">By Request</SelectItem>
            <SelectItem value="session">By Session</SelectItem>
            <SelectItem value="identity">By Identity</SelectItem>
            <SelectItem value="flow">By Flow</SelectItem>
            <SelectItem value="fingerprint">By Fingerprint</SelectItem>
            <SelectItem value="client">By Client/App</SelectItem>
          </SelectContent>
        </Select>
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
    </div>

    <!-- Search Bar -->
    <div class="flex items-center gap-3 max-w-3xl">
      <div class="relative flex-1 group">
        <Search class="absolute left-3.5 top-3 h-5 w-5 text-muted-foreground transition-colors group-focus-within:text-primary" />
        <Input 
          v-model="queryInput" 
          placeholder="Filter by request_id, session_id, fingerprint, client_id, or user identifier..." 
          class="pl-11 h-12 w-full bg-card shadow-sm border-muted transition-all focus-visible:ring-2 focus-visible:ring-ring"
          @keydown.enter="applySearch"
        />
      </div>
      <Button @click="applySearch" :disabled="loading" size="lg" class="h-12 px-6">
        <Loader2 class="w-4 h-4 mr-2 animate-spin" v-if="loading" />
        <Zap class="w-4 h-4 mr-2" v-else />
        {{ loading ? 'Tracing...' : 'Trace' }}
      </Button>
    </div>

    <!-- Error -->
    <div v-if="error" class="p-4 rounded-md border border-destructive/50 bg-destructive/10 text-destructive text-sm flex items-start">
      <AlertCircle class="w-5 h-5 mr-3 shrink-0" />
      <div>
        <h4 class="font-medium mb-1">Trace Error</h4>
        <p>{{ error }}</p>
      </div>
    </div>

    <!-- Loading -->
    <div v-if="loading && traceGroups.length === 0" class="py-12 text-center text-muted-foreground flex flex-col items-center opacity-50">
      <Activity class="size-10 mb-4 animate-pulse" />
      <p>Loading recent traces...</p>
    </div>

    <!-- Traces Table -->
    <Card v-if="traceGroups.length > 0 || hasLoaded" class="overflow-hidden">
      <div v-if="traceGroups.length === 0 && hasLoaded" class="py-16 text-center text-muted-foreground">
        <div class="w-16 h-16 rounded-full bg-muted/40 flex items-center justify-center mb-4 mx-auto">
          <Workflow class="size-8 opacity-40" />
        </div>
        <p class="font-medium text-lg text-foreground">No traces found</p>
        <p class="text-sm mt-1 max-w-sm mx-auto">No matching traces in the selected time range.</p>
      </div>

      <div v-else>
        <!-- Table Header -->
        <div class="grid grid-cols-[2fr_2fr_80px_100px_80px_100px] gap-4 px-4 py-3 border-b bg-muted/30 text-xs font-medium text-muted-foreground uppercase tracking-wider">
          <span>Identity</span>
          <span>Root Event</span>
          <span class="text-center">Events</span>
          <span class="text-right">Duration</span>
          <span class="text-center">Delegation</span>
          <span class="text-right">Time</span>
        </div>

        <!-- Table Rows -->
        <div v-for="(trace, traceIndex) in traceGroups" :key="traceIndex" class="border-b last:border-b-0">
          <!-- Summary Row -->
          <div 
            class="grid grid-cols-[2fr_2fr_80px_100px_80px_100px] gap-4 px-4 py-3 items-center cursor-pointer hover:bg-muted/20 transition-colors"
            :class="expandedTrace === trace.trace_group ? 'bg-muted/30' : ''"
            @click="toggleExpand(trace)"
          >
            <!-- Identity -->
            <div class="flex items-center gap-3 min-w-0">
              <div class="w-8 h-8 rounded-full flex items-center justify-center shrink-0 text-xs font-semibold"
                   :class="trace.identity ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'">
                {{ trace.identity ? initials(trace.identity.display_name || trace.identity.identifier) : '?' }}
              </div>
              <div class="min-w-0">
                <p class="text-sm font-medium truncate">{{ trace.identity?.display_name || trace.identity?.identifier || 'System' }}</p>
                <p class="text-xs text-muted-foreground truncate" v-if="trace.identity?.identifier">{{ trace.identity.identifier }}</p>
              </div>
            </div>

            <!-- Root Span -->
            <div class="flex items-center gap-2 min-w-0">
              <Badge variant="secondary" v-if="trace.method" class="font-mono text-[10px] shrink-0">{{ trace.method }}</Badge>
              <span class="text-sm font-mono truncate text-foreground/80">{{ trace.path || trace.root_event_type || truncateId(trace.trace_group) }}</span>
            </div>

            <!-- Span Count -->
            <div class="text-center">
              <span class="text-sm font-medium flex items-center justify-center gap-1.5">
                <Activity class="w-3.5 h-3.5 text-muted-foreground" />
                {{ trace.span_count }}
              </span>
            </div>

            <!-- Duration -->
            <div class="text-right">
              <span class="text-sm font-mono px-2 py-0.5 rounded"
                    :class="durationColor(trace.duration)">
                {{ trace.duration ? trace.duration + 'ms' : '—' }}
              </span>
            </div>

            <!-- Delegation -->
            <div class="text-center">
              <Badge v-if="trace.delegation_type && trace.delegation_type !== 'direct'" 
                     variant="outline"
                     class="font-mono text-xs shadow-none border-dashed text-amber-600 border-amber-200 bg-amber-50 dark:text-amber-400 dark:border-amber-800 dark:bg-amber-950">
                {{ trace.delegation_type }}
              </Badge>
              <Badge v-else-if="trace.status" 
                     :variant="trace.status >= 400 ? 'destructive' : 'outline'"
                     class="font-mono text-xs shadow-none border-dashed"
                     :class="trace.status < 400 ? 'text-green-600 border-green-200 bg-green-50 dark:text-green-400 dark:border-green-800 dark:bg-green-950' : ''">
                {{ trace.status }}
              </Badge>
              <span v-else class="text-xs text-muted-foreground">Direct</span>
            </div>

            <!-- Time -->
            <div class="text-right">
              <span class="text-xs text-muted-foreground">{{ timeAgo(trace.started_at) }}</span>
            </div>
          </div>

          <!-- Expanded Waterfall -->
          <div v-if="expandedTrace === trace.trace_group" class="border-t bg-muted/5 animate-in fade-in slide-in-from-top-2 duration-300">
            <!-- Loading detail -->
            <div v-if="detailLoading" class="py-8 text-center text-muted-foreground">
              <Loader2 class="w-5 h-5 animate-spin mx-auto mb-2" />
              <p class="text-sm">Loading trace detail...</p>
            </div>

            <div v-else-if="detailEvents.length > 0" class="p-4 space-y-4">
              <!-- Trace Metadata -->
              <div class="flex flex-wrap gap-4">
                <!-- Identity Card -->
                <RouterLink v-if="trace.identity" :to="`/users/${trace.identity.id}`" 
                            class="flex items-center gap-3 bg-card hover:bg-muted/40 transition-colors rounded-lg border hover:border-primary/50 cursor-pointer px-4 py-3 shadow-xs">
                  <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                    <User class="w-5 h-5 text-primary" />
                  </div>
                  <div class="min-w-0">
                    <p class="font-semibold text-sm truncate">{{ trace.identity.display_name || trace.identity.identifier }}</p>
                    <div class="flex items-center gap-2 text-xs text-muted-foreground">
                      <span class="truncate">{{ trace.identity.identifier }}</span>
                      <Badge variant="outline" class="text-[10px] h-4 py-0 px-1.5 shadow-none shrink-0">{{ trace.identity.state }}</Badge>
                    </div>
                  </div>
                </RouterLink>

                <!-- Request / Session / Client IDs -->
                <div class="flex flex-wrap items-center gap-2">
                  <Badge variant="outline" class="font-mono bg-background shadow-xs text-[10px] py-1" v-if="trace.request_id">
                    request: {{ truncateId(trace.request_id, 16) }}
                  </Badge>
                  <Badge variant="outline" class="font-mono bg-background shadow-xs text-[10px] py-1" v-if="trace.session_id">
                    session: {{ truncateId(trace.session_id, 16) }}
                  </Badge>
                  <Badge variant="outline" class="font-mono bg-background shadow-xs text-[10px] py-1" v-if="trace.client_id" title="Client/App ID">
                    client: {{ truncateId(trace.client_id, 16) }}
                  </Badge>
                  <Badge variant="outline" class="font-mono bg-background shadow-xs text-[10px] py-1" v-if="trace.fingerprint" title="Device Fingerprint">
                    <Fingerprint class="w-3 h-3 mr-1 inline-block" /> {{ truncateId(trace.fingerprint, 8) }}
                  </Badge>
                  <Badge variant="secondary" class="text-[10px] py-1">
                    {{ detailEvents.length }} events · {{ totalDuration }}ms
                  </Badge>
                </div>
              </div>

              <!-- Timeline Header -->
              <div class="flex items-center justify-between text-xs text-muted-foreground px-1">
                <span class="font-medium uppercase tracking-wider">Timeline</span>
                <div class="flex items-center gap-4 font-mono">
                  <span>0ms</span>
                  <span>{{ totalDuration }}ms</span>
                </div>
              </div>

              <!-- Waterfall Bars -->
              <div class="rounded-lg border bg-card overflow-hidden">
                <div v-for="(span, spanIdx) in waterfallSpans" :key="spanIdx" class="border-b last:border-b-0 transition-colors" :class="selectedSpan?.id === span.id ? 'bg-primary/5' : 'hover:bg-muted/20'">
                  
                  <!-- Span Row -->
                  <div class="flex items-center gap-0 cursor-pointer" @click="selectSpan(span)">
                    <!-- Label (left third) -->
                    <div class="w-[280px] shrink-0 px-3 py-2.5 flex items-center gap-2 border-r min-w-0">
                      <!-- Tree indent -->
                      <div v-if="span.depth > 0" class="flex items-center shrink-0">
                        <span v-for="(_, depthIndex) in span.depth" :key="depthIndex" class="h-full w-4 border-l border-muted-foreground/20"></span>
                        <span class="text-muted-foreground/40 mr-1">{{ span.isLast ? '└' : '├' }}─</span>
                      </div>
                      <div class="p-1 rounded shrink-0" :class="span.iconBg">
                        <component :is="span.icon" class="w-3 h-3" :class="span.iconColor" />
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-1.5">
                          <Badge variant="secondary" v-if="span.method" class="font-mono text-[9px] px-1 py-0 h-4 shrink-0">{{ span.method }}</Badge>
                          <span class="text-xs font-medium truncate">{{ span.label }}</span>
                        </div>
                        <p class="text-[10px] text-muted-foreground font-mono truncate mt-0.5" v-if="span.path">{{ span.path }}</p>
                      </div>
                    </div>

                    <!-- Bar (right two-thirds) -->
                    <div class="flex-1 px-3 py-2.5 relative h-10">
                      <div class="absolute inset-y-0 flex items-center w-full px-1">
                        <div class="relative w-full h-5 rounded-sm overflow-hidden bg-muted/20">
                          <div class="absolute h-full rounded-sm transition-all"
                               :class="span.barColor"
                               :style="{ left: span.offsetPct + '%', width: Math.max(span.widthPct, 0.5) + '%' }">
                          </div>
                        </div>
                      </div>
                      <!-- Duration label -->
                      <div class="absolute right-4 top-1/2 -translate-y-1/2 text-[10px] font-mono text-muted-foreground">
                        {{ span.duration ? span.duration + 'ms' : '<1ms' }}
                      </div>
                    </div>
                  </div>

                  <!-- Span Detail (inline expand) -->
                  <div v-if="selectedSpan?.id === span.id" class="px-4 py-3 border-t bg-muted/10 animate-in fade-in duration-200">
                    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 text-xs">
                      <div class="space-y-3">
                        <p class="font-medium text-muted-foreground uppercase tracking-wider">Trace Context</p>
                        <div class="grid grid-cols-[110px_1fr] gap-x-2 gap-y-1.5 font-mono">
                          <span class="text-muted-foreground">Event Type</span>
                          <span>{{ span.event_type }}</span>
                          <span class="text-muted-foreground">Request ID</span>
                          <span class="truncate text-primary cursor-pointer hover:underline" @click.stop="jumpToTrace(span.request_id)">{{ span.request_id || '—' }}</span>
                          <span class="text-muted-foreground">Session ID</span>
                          <RouterLink v-if="span.session_id" to="/console/sessions" class="truncate text-primary hover:underline" @click.stop>{{ span.session_id }}</RouterLink>
                          <span v-else>—</span>

                          <template v-if="span.client_id">
                            <span class="text-muted-foreground">Client ID</span>
                            <span class="truncate">{{ span.client_id }}</span>
                          </template>

                          <template v-if="span.delegation_type && span.delegation_type !== 'direct'">
                            <span class="text-muted-foreground">Delegation</span>
                            <span class="truncate text-amber-600 dark:text-amber-400">{{ span.delegation_type }}</span>
                          </template>

                          <template v-if="span.sdk_name">
                            <span class="text-muted-foreground">SDK</span>
                            <span class="truncate">{{ span.sdk_name }} {{ span.sdk_version }}</span>
                          </template>

                          <span class="text-muted-foreground" v-if="span.fingerprint">Fingerprint</span>
                          <RouterLink v-if="span.fingerprint" :to="`/console/events?fingerprint=${span.fingerprint}`" class="truncate font-mono text-primary hover:underline" @click.stop>{{ span.fingerprint }}</RouterLink>

                          <span class="text-muted-foreground" v-if="span.actor_id">Actor ID</span>
                          <RouterLink v-if="span.actor_id" :to="`/users/${span.actor_id}`" class="truncate text-primary hover:underline" @click.stop>{{ span.actor_id }}</RouterLink>
                        </div>
                        <RouterLink :to="`/console/events?id=${span.id}`" 
                                    class="inline-flex items-center gap-1.5 text-[11px] text-primary hover:underline mt-2"
                                    @click.stop>
                          <ExternalLink class="w-3 h-3" />
                          Open in Events
                        </RouterLink>
                      </div>
                      <div v-if="span.payloadStr !== '{}'" class="border rounded-md bg-muted/20 overflow-hidden">
                        <div class="bg-muted px-3 py-1.5 font-medium font-mono border-b flex items-center justify-between text-muted-foreground">
                          Payload
                          <FileJson class="w-3.5 h-3.5" />
                        </div>
                        <div class="p-3 overflow-x-auto text-[11px] font-mono whitespace-pre-wrap leading-relaxed max-h-48 overflow-y-auto">{{ span.payloadStr }}</div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter, RouterLink } from 'vue-router'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Search, Workflow, AlertCircle, Zap, Loader2, Clock, Globe, Key, FileJson, Server, Activity, User, Fingerprint, ExternalLink } from 'lucide-vue-next'
import { api } from '@/api/client'
import { userApi, type Identity } from '@/api/resources'
import {
  buildTraceRouteQuery,
  buildTraceWhereClause,
  escapeSqlLiteral,
  getTraceRouteFilter,
  type TraceRouteFilterMode,
} from '@/console/utils/route-filters'
import type { AcceptableValue } from 'reka-ui'

const route = useRoute()
const router = useRouter()

const initialRouteFilter = getTraceRouteFilter(route.query)
const queryInput = ref(initialRouteFilter?.value || '')
const searchMode = ref<TraceRouteFilterMode>(initialRouteFilter?.mode || 'generic')
const timeRange = ref<AcceptableValue>('24h')
const groupBy = ref<AcceptableValue>('trace')
const loading = ref(false)
const detailLoading = ref(false)
const error = ref('')
const hasLoaded = ref(false)

// Trace list (Zone 1)
interface TraceGroup {
  trace_group: string
  request_id: string
  session_id: string
  flow_id: string
  actor_id: string
  fingerprint?: string
  client_id?: string
  delegation_type?: string
  started_at: string
  span_count: number
  method?: string
  path?: string
  status?: number
  duration?: number
  root_event_type?: string
  identity?: Identity | null
}
const traceGroups = ref<TraceGroup[]>([])

// Expanded trace detail (Zone 2)
const expandedTrace = ref<string | null>(null)
const detailEvents = ref<any[]>([])
const selectedSpan = ref<any | null>(null)

// Identity cache
const identityCache = ref<Record<string, Identity | null>>({})

const timeRangeMs: Record<string, number> = {
  '1h': 3600000,
  '12h': 43200000,
  '24h': 86400000,
  '7d': 604800000,
  '30d': 2592000000,
}

onMounted(() => {
  if (queryInput.value) {
    fetchFilteredTraces()
  } else {
    fetchRecentTraces()
  }
})

watch([timeRange, groupBy], () => {
  if (queryInput.value) {
    fetchFilteredTraces()
  } else {
    fetchRecentTraces()
  }
})

const groupExpr = computed(() => {
  switch (String(groupBy.value)) {
    case 'session': return 'session_id'
    case 'identity': return 'actor_id'
    case 'flow': return 'flow_id'
    case 'fingerprint': return 'fingerprint'
    case 'client': return 'client_id'
    default: return "COALESCE(NULLIF(request_id, ''), NULLIF(session_id, ''), NULLIF(flow_id, ''), NULLIF(fingerprint, ''))"
  }
})

watch(() => [route.query.actor_id, route.query.id], () => {
  const nextFilter = getTraceRouteFilter(route.query)
  if (nextFilter && (nextFilter.value !== queryInput.value || nextFilter.mode !== searchMode.value)) {
    queryInput.value = nextFilter.value
    searchMode.value = nextFilter.mode
    fetchFilteredTraces()
  } else if (!nextFilter && queryInput.value) {
    queryInput.value = ''
    searchMode.value = 'generic'
    expandedTrace.value = null
    fetchRecentTraces()
  }
})

function applySearch() {
  if (queryInput.value.trim()) {
    router.replace({
      query: buildTraceRouteQuery(route.query, queryInput.value.trim(), searchMode.value),
    }).catch(() => {})
    fetchFilteredTraces()
  } else {
    router.replace({ query: buildTraceRouteQuery(route.query, '', searchMode.value) }).catch(() => {})
    fetchRecentTraces()
  }
}

async function fetchRecentTraces() {
  loading.value = true
  error.value = ''
  expandedTrace.value = null
  
  const cutoff = new Date(Date.now() - timeRangeMs[String(timeRange.value) || '24h']).toISOString()

  const ge = groupExpr.value
  const sql = `
    SELECT 
      ${ge} as trace_group, 
      MAX(request_id) as request_id, MAX(session_id) as session_id, MAX(flow_id) as flow_id,
      MAX(NULLIF(actor_id, '')) as actor_id,
      MAX(NULLIF(fingerprint, '')) as fingerprint,
      MAX(NULLIF(client_id, '')) as client_id,
      MAX(NULLIF(delegation_type, '')) as delegation_type,
      MIN(event_type) as root_event_type,
      MIN(created_at) as started_at, 
      COUNT(*) as span_count,
      MAX(payload) as sample_payload
    FROM events 
    WHERE created_at >= '${cutoff}'
      AND ((request_id != '' AND request_id IS NOT NULL) OR (session_id != '' AND session_id IS NOT NULL) OR (actor_id != '' AND actor_id IS NOT NULL) OR (flow_id != '' AND flow_id IS NOT NULL) OR (fingerprint != '' AND fingerprint IS NOT NULL))
    GROUP BY ${ge}
    ORDER BY started_at DESC 
    LIMIT 50
  `
  try {
    const data = await api.post<any>('/v1/analytics/query', { sql, limit: 50 })
    if (!data.error && data.rows && data.columns) {
      const cols: string[] = data.columns.map((c: string) => c.toLowerCase())
      const raw = data.rows.map((rowArr: any[]) => {
        const r: any = {}
        cols.forEach((c, i) => { r[c] = rowArr[i] })
        return r
      })

      traceGroups.value = await Promise.all(raw.map(async (r: any) => {
        let payload: any = {}
        try { payload = JSON.parse(r.sample_payload || '{}') } catch {}

        const group: TraceGroup = {
          trace_group: r.trace_group,
          request_id: r.request_id || '',
          session_id: r.session_id || '',
          flow_id: r.flow_id || '',
          fingerprint: r.fingerprint || '',
          client_id: r.client_id || '',
          delegation_type: r.delegation_type || '',
          actor_id: r.actor_id || '',
          started_at: r.started_at,
          span_count: Number(r.span_count),
          method: payload.method,
          path: payload.path,
          status: payload.status,
          duration: payload.duration_ms,
          root_event_type: r.root_event_type,
          identity: null
        }

        if (group.actor_id && (!group.actor_id.includes('-') || group.actor_id === group.request_id)) {
          group.actor_id = ''
        }

        if (group.actor_id) {
          group.identity = await resolveIdentity(group.actor_id)
        }
        return group
      }))
    } else {
      traceGroups.value = []
    }
    hasLoaded.value = true
  } catch (err: any) {
    error.value = err.message || 'Failed to load traces'
  } finally {
    loading.value = false
  }
}

async function fetchFilteredTraces() {
  loading.value = true
  error.value = ''
  expandedTrace.value = null

  const val = queryInput.value.trim()
  const cutoff = new Date(Date.now() - timeRangeMs[String(timeRange.value) || '24h']).toISOString()

  const ge = groupExpr.value
  const sql = `
    SELECT 
      ${ge} as trace_group, 
      MAX(request_id) as request_id, MAX(session_id) as session_id, MAX(flow_id) as flow_id,
      MAX(NULLIF(actor_id, '')) as actor_id,
      MAX(NULLIF(fingerprint, '')) as fingerprint,
      MAX(NULLIF(client_id, '')) as client_id,
      MAX(NULLIF(delegation_type, '')) as delegation_type,
      MIN(event_type) as root_event_type,
      MIN(created_at) as started_at, 
      COUNT(*) as span_count,
      MAX(payload) as sample_payload
    FROM events 
    WHERE created_at >= '${cutoff}'
      AND ${buildTraceWhereClause(val, searchMode.value)}
    GROUP BY ${ge}
    ORDER BY started_at DESC 
    LIMIT 50
  `
  try {
    const data = await api.post<any>('/v1/analytics/query', { sql, limit: 50 })
    if (!data.error && data.rows && data.columns) {
      const cols: string[] = data.columns.map((c: string) => c.toLowerCase())
      const raw = data.rows.map((rowArr: any[]) => {
        const r: any = {}
        cols.forEach((c, i) => { r[c] = rowArr[i] })
        return r
      })

      traceGroups.value = await Promise.all(raw.map(async (r: any) => {
        let payload: any = {}
        try { payload = JSON.parse(r.sample_payload || '{}') } catch {}

        const group: TraceGroup = {
          trace_group: r.trace_group,
          request_id: r.request_id || '',
          session_id: r.session_id || '',
          flow_id: r.flow_id || '',
          fingerprint: r.fingerprint || '',
          client_id: r.client_id || '',
          delegation_type: r.delegation_type || '',
          actor_id: r.actor_id || '',
          started_at: r.started_at,
          span_count: Number(r.span_count),
          method: payload.method,
          path: payload.path,
          status: payload.status,
          duration: payload.duration_ms,
          root_event_type: r.root_event_type,
          identity: null
        }

        if (group.actor_id && (!group.actor_id.includes('-') || group.actor_id === group.request_id)) {
          group.actor_id = ''
        }

        if (group.actor_id) {
          group.identity = await resolveIdentity(group.actor_id)
        }
        return group
      }))
    } else {
      traceGroups.value = []
    }
    hasLoaded.value = true
  } catch (err: any) {
    error.value = err.message || 'Failed to search traces'
  } finally {
    loading.value = false
  }
}

async function toggleExpand(trace: TraceGroup) {
  if (expandedTrace.value === trace.trace_group) {
    expandedTrace.value = null
    detailEvents.value = []
    selectedSpan.value = null
    return
  }

  expandedTrace.value = trace.trace_group
  selectedSpan.value = null
  detailLoading.value = true

  const val = trace.trace_group
  const gb = String(groupBy.value)
  const safeVal = escapeSqlLiteral(val)
  let whereClause: string
  if (gb === 'identity') {
    whereClause = `actor_id = '${safeVal}'`
  } else if (gb === 'session') {
    whereClause = `session_id = '${safeVal}'`
  } else if (gb === 'flow') {
    whereClause = `flow_id = '${safeVal}'`
  } else if (gb === 'fingerprint') {
    whereClause = `fingerprint = '${safeVal}'`
  } else if (gb === 'client') {
    whereClause = `client_id = '${safeVal}'`
  } else {
    whereClause = `request_id = '${safeVal}' OR session_id = '${safeVal}' OR flow_id = '${safeVal}' OR fingerprint = '${safeVal}'`
  }
  const sql = `SELECT * FROM events WHERE ${whereClause} ORDER BY created_at ASC LIMIT 500`

  try {
    const data = await api.post<any>('/v1/analytics/query', { sql, limit: 500 })
    if (!data.error && data.rows && data.columns) {
      const cols: string[] = data.columns.map((c: string) => c.toLowerCase())
      detailEvents.value = data.rows.map((rowArr: any[]) => {
        const obj: any = {}
        cols.forEach((c, i) => { obj[c] = rowArr[i] })
        return obj
      })
    } else {
      detailEvents.value = []
    }
  } catch {
    detailEvents.value = []
  } finally {
    detailLoading.value = false
  }
}

function selectSpan(span: any) {
  selectedSpan.value = selectedSpan.value?.id === span.id ? null : span
}

function jumpToTrace(requestId: string) {
  if (!requestId) return
  queryInput.value = requestId
  searchMode.value = 'generic'
  router.replace({ query: buildTraceRouteQuery(route.query, requestId, 'generic') }).catch(() => {})
  fetchFilteredTraces()
}

// --- Identity resolution ---
async function resolveIdentity(actorId: string): Promise<Identity | null> {
  if (!actorId || actorId === '') return null
  if (identityCache.value[actorId] !== undefined) return identityCache.value[actorId]

  try {
    const identity = await userApi.get(actorId)
    identityCache.value[actorId] = identity
    return identity
  } catch {
    identityCache.value[actorId] = null
    return null
  }
}

// --- Waterfall computation ---
const totalDuration = computed(() => {
  if (detailEvents.value.length === 0) return 0
  const durations = detailEvents.value.map(e => {
    try {
      const p = JSON.parse(e.payload || '{}')
      return p.duration_ms || 0
    } catch { return 0 }
  })
  return Math.max(...durations, 1)
})

const waterfallSpans = computed(() => {
  if (detailEvents.value.length === 0) return []

  const traceStart = new Date(detailEvents.value[0].created_at).getTime()
  const totalMs = totalDuration.value || 1

  // Build parent-child tree
  const spanMap = new Map<string, any>()
  const rootSpans: any[] = []
  
  // First pass: parse all spans
  const parsed = detailEvents.value.map(raw => {
    let payload: any = {}
    try { payload = JSON.parse(raw.payload || '{}') } catch {}

    const isApiRequest = raw.event_type === 'request.api'
    let icon = Activity
    let iconColor = 'text-blue-500'
    let iconBg = 'bg-blue-500/10'
    let barColor = 'bg-blue-500/70'
    let label = raw.event_type || 'Unknown'

    if (isApiRequest) {
      icon = Globe
      iconColor = payload.status >= 400 ? 'text-red-500' : 'text-emerald-500'
      iconBg = payload.status >= 400 ? 'bg-red-500/10' : 'bg-emerald-500/10'
      barColor = payload.status >= 400 ? 'bg-red-500/70' : 'bg-emerald-500/70'
      label = 'HTTP Request'
    } else if (label.includes('auth') || label.includes('login') || label.includes('session')) {
      icon = Key
      iconColor = 'text-amber-500'
      iconBg = 'bg-amber-500/10'
      barColor = 'bg-amber-500/70'
    } else if (raw.aggregate_type === 'user' || raw.aggregate_type === 'identity') {
      icon = Server
      iconColor = 'text-purple-500'
      iconBg = 'bg-purple-500/10'
      barColor = 'bg-purple-500/70'
    }

    const offset = Math.max(0, new Date(raw.created_at).getTime() - traceStart)
    const duration = payload.duration_ms || 0

    return {
      id: raw.id,
      event_type: raw.event_type,
      request_id: raw.request_id,
      session_id: raw.session_id,
      actor_id: raw.actor_id,
      client_id: raw.client_id,
      delegation_type: raw.delegation_type,
      sdk_name: raw.sdk_name,
      sdk_version: raw.sdk_version,
      fingerprint: raw.fingerprint,
      created_at: raw.created_at,
      icon, iconColor, iconBg, barColor, label,
      method: payload.method,
      path: payload.path,
      status: payload.status,
      duration,
      offsetPct: totalMs > 0 ? (offset / totalMs) * 100 : 0,
      widthPct: totalMs > 0 ? Math.max((duration / totalMs) * 100, 0.5) : 0.5,
      payloadStr: JSON.stringify(payload, null, 2),
      depth: 0,
      isLast: false,
      children: [] as any[]
    }
  })

  // No span tree — flat chronological timeline (ADR-023: wide events, no span hierarchy)
  return parsed
})

// --- Helpers ---
function initials(name: string): string {
  if (!name) return '?'
  const parts = name.split(/[\s@.]/).filter(Boolean)
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase()
  return name.slice(0, 2).toUpperCase()
}

function truncateId(str: string, len = 12): string {
  if (!str) return ''
  if (str.length <= len) return str
  return str.slice(0, len) + '...'
}

function durationColor(ms?: number): string {
  if (!ms) return 'text-muted-foreground'
  if (ms < 200) return 'bg-green-50 text-green-700 dark:bg-green-950 dark:text-green-400'
  if (ms < 1000) return 'bg-amber-50 text-amber-700 dark:bg-amber-950 dark:text-amber-400'
  return 'bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-400'
}

function timeAgo(ts: string): string {
  if (!ts) return ''
  const d = new Date(ts)
  const diff = Date.now() - d.getTime()
  if (diff < 60000) return 'just now'
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`
  return d.toLocaleDateString()
}
</script>
