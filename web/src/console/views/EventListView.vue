<template>
  <!-- eslint-disable vue/valid-v-for -->
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Events</h1>
      <p class="text-muted-foreground mt-1 text-sm">Audit log of all system events ({{ totalCount }} loaded)</p>
    </div>

    <!-- Stats Bar -->
    <div class="flex items-center justify-between p-4 rounded-lg border text-sm bg-card text-muted-foreground">
      <div class="flex items-center gap-6">
        <div class="flex items-center space-x-2 text-primary">
          <Activity class="w-4 h-4" />
          <span class="font-medium">{{ countApi }} Request events</span>
        </div>
        <div class="w-px h-4 bg-border"></div>
        <div class="flex items-center space-x-2 text-blue-600">
          <Key class="w-4 h-4" />
          <span>{{ countAuth }} Auth events</span>
        </div>
        <div class="w-px h-4 bg-border"></div>
        <div class="flex items-center space-x-2 text-emerald-600">
          <Globe class="w-4 h-4" />
          <span>{{ countSession }} Session events</span>
        </div>
      </div>
      <Button size="sm" :variant="isLive ? 'default' : 'outline'" @click="toggleLive" 
              :class="isLive ? 'bg-red-600 hover:bg-red-700 text-white border-red-600' : ''">
        <Radio class="size-3.5 mr-1.5" :class="isLive ? 'animate-pulse' : ''" />
        {{ isLive ? '● Live' : 'Live' }}
      </Button>
    </div>

    <DataTable 
      v-if="events.length > 0"
      :columns="columns as any" 
      :data="events" 
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
        <div style="display:none">{{ __setTable(table) }}</div>
        <div class="flex items-center justify-between w-full mb-4">
          <!-- Unified Search Bar -->
          <div class="w-full max-w-lg relative" ref="searchContainerRef">
                <div class="relative w-full">
                  <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
                  <Input
                    ref="searchInputRef"
                    placeholder="Search events (e.g. type:api actor:user1)"
                    class="pl-9 bg-background w-full relative z-0"
                    :model-value="globalSearch"
                    @update:model-value="val => applySearchQuery(String(val), table)"
                    @focus="isSearchOpen = true"
                    @keydown.esc="isSearchOpen = false"
                  />
                </div>
              <div 
                v-if="isSearchOpen"
                class="absolute top-full left-0 mt-2 w-[500px] z-50 bg-popover text-popover-foreground rounded-md border shadow-md outline-none overflow-hidden"
              >
                <div class="py-1">
                  <div v-if="!currentFilterPrefix">
                    <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">Filters</div>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('type:')">
                      <span class="font-medium mr-2">type:</span> Filter by Event Type
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('actor:')">
                      <span class="font-medium mr-2">actor:</span> Filter by Actor ID
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('aggregate:')">
                      <span class="font-medium mr-2">aggregate:</span> Filter by Aggregate ID
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('fingerprint:')">
                      <span class="font-medium mr-2">fingerprint:</span> Filter by Device Fingerprint
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('client:')">
                      <span class="font-medium mr-2">client:</span> Filter by Client / App
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('delegation:')">
                      <span class="font-medium mr-2">delegation:</span> Filter by Delegation Type
                    </button>
                  </div>
                  
                  <div v-if="currentFilterPrefix === 'type:'">
                    <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">Event Types</div>
                    <button v-for="(eventType, eventTypeIndex) in eventTypes" :key="eventTypeIndex" class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken(`type:${eventType} `)">
                      {{ eventType }}
                    </button>
                  </div>
                </div>
              </div>
          </div>
          
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" class="ml-auto">
                View <ChevronDown class="ml-2 h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuCheckboxItem v-for="(column, columnIndex) in table.getAllColumns().filter((col: any) => col.getCanHide())" :key="columnIndex" class="capitalize" :checked="table.getState().columnVisibility[column.id] !== false" @update:checked="(val: boolean) => column.toggleVisibility(!!val)">
                {{ column.id.replace('_', ' ') }}
              </DropdownMenuCheckboxItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </template>

      <template #expanded="{ row }">
        <div class="px-8 py-6 bg-muted/10 border-b shadow-inner">
          <div class="space-y-4 max-w-4xl">
            <div>
              <div class="flex items-center gap-2 mb-2">
                 <FileJson class="w-4 h-4 text-muted-foreground" />
                 <h4 class="text-sm font-semibold text-foreground">Payload Data</h4>
              </div>
              <pre class="bg-muted p-4 rounded-md text-xs font-mono overflow-auto border border-border/50 text-foreground/80 leading-relaxed mt-1">{{ JSON.stringify(row.original.payload || {}, null, 2) }}</pre>
            </div>
            <div class="grid grid-cols-2 gap-4 pt-2">
               <div>
                  <h4 class="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1">Internal Reference</h4>
                  <p class="text-xs font-mono">{{ row.original.id }}</p>
               </div>
               <div>
                  <h4 class="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1">Aggregate</h4>
                  <p class="text-xs font-mono">
                    <span class="text-muted-foreground">{{ row.original.aggregate_type }}</span>
                    <span class="text-muted-foreground mx-1">→</span>
                    <RouterLink v-if="row.original.aggregate_type === 'session'" :to="`/console/sessions`" class="text-primary hover:underline">{{ row.original.aggregate_id }}</RouterLink>
                    <RouterLink v-else-if="row.original.aggregate_type === 'user' || row.original.aggregate_type === 'identity'" :to="`/users/${row.original.aggregate_id}`" class="text-primary hover:underline">{{ row.original.aggregate_id }}</RouterLink>
                    <span v-else>{{ row.original.aggregate_id }}</span>
                  </p>
               </div>
            </div>
            <!-- Wide Event Context (ADR-023) -->
            <div class="grid grid-cols-3 gap-4 pt-2" v-if="row.original.request_id || row.original.client_id || row.original.delegation_type">
               <div v-if="row.original.request_id">
                  <h4 class="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1">Request ID</h4>
                  <RouterLink :to="`/console/traces?id=${row.original.request_id}`" class="text-xs font-mono text-primary hover:underline">{{ row.original.request_id.slice(0, 16) }}…</RouterLink>
               </div>
               <div v-if="row.original.client_id">
                  <h4 class="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1">Client / App</h4>
                  <p class="text-xs font-mono">{{ row.original.client_id }}</p>
               </div>
               <div v-if="row.original.delegation_type && row.original.delegation_type !== 'direct'">
                  <h4 class="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1">Delegation</h4>
                  <Badge variant="outline" class="text-[10px] text-amber-600 border-amber-200 bg-amber-50 dark:text-amber-400 dark:border-amber-800 dark:bg-amber-950">{{ row.original.delegation_type }}</Badge>
               </div>
               <div v-if="row.original.sdk_name">
                  <h4 class="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1">SDK</h4>
                  <p class="text-xs font-mono">{{ row.original.sdk_name }} {{ row.original.sdk_version }}</p>
               </div>
            </div>
          </div>
        </div>
      </template>

      <template #pagination="{ table }">
        <DataTablePagination :table="table" />
      </template>
    </DataTable>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, h, watch, onUnmounted } from 'vue'
import { onClickOutside } from '@vueuse/core'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { eventApi, type Event } from '@/api/resources'
import { getEventRouteFilters } from '@/console/utils/route-filters'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, DropdownMenuCheckboxItem } from '@/components/ui/dropdown-menu'
import { 
  Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown, Activity, Key, Globe, LayoutList, MoreHorizontal, FileJson, ExternalLink, Link2, Radio
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

const route = useRoute()
const router = useRouter()

const events = ref<Event[]>([])
const selectedRows = ref({})
const globalSearch = ref('')
const isSearchOpen = ref(false)
const eventTypes = ref<string[]>([])
const isLive = ref(false)
let eventSource: EventSource | null = null
const routeFilters = computed(() => getEventRouteFilters(route.query))

function toggleLive() {
  if (isLive.value) {
    eventSource?.close()
    eventSource = null
    isLive.value = false
    return
  }

  // Build SSE URL with current filter context
  const params = new URLSearchParams()
  params.set('cursor', 'now')
  const fp = routeFilters.value.fingerprint
  if (fp) params.set('fingerprint', fp)
  const sid = routeFilters.value.sessionId
  if (sid) params.set('session_id', sid)
  const aggregateId = routeFilters.value.aggregateId
  if (aggregateId) params.set('aggregate_id', aggregateId)

  eventSource = new EventSource(`/v1/events/stream?${params.toString()}`)
  isLive.value = true

  eventSource.onmessage = (e) => {
    try {
      const evt = JSON.parse(e.data) as Event
      events.value = [evt, ...events.value]
      eventTypes.value = [...new Set([evt.event_type, ...eventTypes.value])]
    } catch { /* ignore malformed */ }
  }
  eventSource.onerror = () => {
    eventSource?.close()
    eventSource = null
    isLive.value = false
  }
}
onUnmounted(() => {
  eventSource?.close()
})

const searchInputRef = ref<any>(null)
const searchContainerRef = ref<HTMLElement | null>(null)

onClickOutside(searchContainerRef, () => {
  isSearchOpen.value = false
})

let activeTable: any = null

function __setTable(t: any) {
  if (!activeTable && t) {
    activeTable = t
    if (globalSearch.value) applySearchQuery(globalSearch.value, t)
  }
  return ''
}

function getSortIcon(column: any) {
  const isSorted = column.getIsSorted()
  if (isSorted === 'asc') return ArrowUp
  if (isSorted === 'desc') return ArrowDown
  return ArrowUpDown
}

const currentFilterPrefix = computed(() => {
   if (!globalSearch.value) return ''
   const parts = globalSearch.value.split(' ')
   const lastPart = parts[parts.length - 1].toLowerCase()
   if (lastPart.startsWith('type:')) return 'type:'
   if (lastPart.startsWith('actor:')) return 'actor:'
   if (lastPart.startsWith('aggregate:')) return 'aggregate:'
   if (lastPart.startsWith('session:')) return 'session:'
   if (lastPart.startsWith('fingerprint:')) return 'fingerprint:'
   if (lastPart.startsWith('client:')) return 'client:'
   if (lastPart.startsWith('delegation:')) return 'delegation:'
   return ''
})

function appendSearchToken(token: string) {
  const parts = globalSearch.value.split(' ')
  const lastPart = parts[parts.length - 1]
  
  if (currentFilterPrefix.value && token.startsWith(currentFilterPrefix.value)) {
     parts[parts.length - 1] = token
  } else {
     if (lastPart && !lastPart.includes(':')) {
        parts.pop()
        parts.push(token)
     } else {
        if (!lastPart) parts.pop()
        parts.push(token)
     }
  }
  
  const newVal = parts.join(' ').trim() + (token.endsWith(' ') ? '' : ' ')
  applySearchQuery(newVal, activeTable)
  if (token.endsWith(':')) {
      isSearchOpen.value = true
  } else {
      isSearchOpen.value = false
  }
}

function applySearchQuery(query: string, table: any) {
  if (table) activeTable = table
  globalSearch.value = query
  const filters: { id: string, value: string }[] = []
  
  const tokens = query.match(/(?:[^\s"]+|"[^"]*")+/g) || []
  let globalText = ''

  for (const token of tokens) {
    if (token.includes(':') && !token.startsWith('::')) {
      const parts = token.split(':')
      const key = parts[0].toLowerCase()
      const value = parts.slice(1).join(':').replace(/(^"|"$)/g, '')
      
      if (['type', 'event_type'].includes(key)) filters.push({ id: 'event_type', value })
      else if (['actor', 'user'].includes(key)) filters.push({ id: 'actor', value })
      else if (['aggregate', 'agg'].includes(key)) filters.push({ id: 'aggregate', value })
      else if (['session'].includes(key)) filters.push({ id: 'session', value })
      else if (['fingerprint', 'fp'].includes(key)) filters.push({ id: 'fingerprint', value })
       else if (['client', 'app'].includes(key)) filters.push({ id: 'client_id', value })
       else if (['delegation', 'del'].includes(key)) filters.push({ id: 'delegation', value })
      else globalText += token + ' '
    } else {
      globalText += token + ' '
    }
  }

  const remainder = globalText.trim()
  if (remainder) {
    if (!filters.find((f: any) => f.id === 'event_type')) {
       filters.push({ id: 'event_type', value: remainder })
    }
  }
  
  if (activeTable) {
    activeTable.setColumnFilters(filters)
  }
}

async function loadEvents() {
  try { 
    const res = await eventApi.list({
      limit: 500,
      session_id: routeFilters.value.sessionId || undefined,
      fingerprint: routeFilters.value.fingerprint || undefined,
      aggregate_id: routeFilters.value.aggregateId || undefined,
    }) as Event[]
    events.value = res
    eventTypes.value = [...new Set(res.map(e => e.event_type))]
    
    // Auto-apply filters to the UI if present
    if (routeFilters.value.aggregateId) {
       globalSearch.value = `aggregate:${routeFilters.value.aggregateId} `
       if (activeTable) applySearchQuery(globalSearch.value, activeTable)
    } else if (routeFilters.value.sessionId) {
       globalSearch.value = `session:${routeFilters.value.sessionId} `
       if (activeTable) applySearchQuery(globalSearch.value, activeTable)
    } else if (routeFilters.value.fingerprint) {
       globalSearch.value = `fingerprint:${routeFilters.value.fingerprint} `
       if (activeTable) applySearchQuery(globalSearch.value, activeTable)
    } else if (routeFilters.value.actor) {
       globalSearch.value = `actor:${routeFilters.value.actor} `
       if (activeTable) applySearchQuery(globalSearch.value, activeTable)
    } else {
       globalSearch.value = ''
       if (activeTable) activeTable.setColumnFilters([])
    }
  } catch (err) {
    console.error('Failed to load events', err)
  }

  // Auto-expand event by permalink (?id=xxx)
  const eventId = routeFilters.value.eventId
  if (eventId && activeTable) {
    const idx = events.value.findIndex(e => e.id === eventId)
    if (idx >= 0) {
      setTimeout(() => {
        activeTable?.getRowModel()?.rows[idx]?.toggleExpanded(true)
      }, 100)
    }
  }
}

watch(
  () => [
    routeFilters.value.actor,
    routeFilters.value.aggregateId,
    routeFilters.value.eventId,
    routeFilters.value.fingerprint,
    routeFilters.value.sessionId,
  ],
  () => {
    loadEvents()
  },
  { immediate: true },
)

function formatDateOnly(ts?: string) { 
  if (!ts) return '—'
  return new Date(ts).toLocaleDateString()
}

const totalCount = computed(() => events.value.length)
const countApi = computed(() => events.value.filter(s => s.event_type.includes('request')).length)
const countAuth = computed(() => events.value.filter(s => s.event_type.includes('auth')).length)
const countSession = computed(() => events.value.filter(s => s.event_type.includes('session')).length)

const columnHelper = createColumnHelper<Event>()

const columns = [
  columnHelper.display({
    id: 'expander',
    header: () => null,
    cell: ({ row }) => {
      return h(Button, {
        variant: 'ghost',
        class: 'h-8 w-8 p-0 hover:bg-muted/50 transition-colors',
        onClick: (e: MouseEvent) => {
          e.stopPropagation()
          row.toggleExpanded()
        }
      }, () => h(ChevronDown, {
        class: ['h-4 w-4 transition-transform duration-200 text-muted-foreground', row.getIsExpanded() ? 'rotate-180' : '']
      }))
    },
    meta: { class: 'w-12 border-r-0' },
    enableSorting: false,
    enableHiding: false,
  }),
  columnHelper.display({
    id: 'select',
    header: ({ table }) => h(Checkbox, {
      checked: table.getIsAllPageRowsSelected() || (table.getIsSomePageRowsSelected() && 'indeterminate' as any),
      'onUpdate:checked': (val: boolean) => table.toggleAllPageRowsSelected(!!val),
      ariaLabel: 'Select all',
    }),
    cell: ({ row }) => h(Checkbox, {
      checked: row.getIsSelected(),
      'onUpdate:checked': (val: boolean) => row.toggleSelected(!!val),
      ariaLabel: 'Select row',
    }),
    meta: { class: 'w-12 border-r-0' },
    enableSorting: false,
    enableHiding: false,
  }),
  columnHelper.accessor('event_type', {
    id: 'event_type',
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Event Type', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => {
      const type = info.getValue()
      let variant: any = 'secondary'
      if (type.includes('created') || type.includes('completed')) variant = 'default'
      else if (type.includes('deleted') || type.includes('revoked')) variant = 'destructive'
      return h(Badge, { variant, class: 'font-mono text-[10px]' }, () => type)
    },
  }),
  columnHelper.accessor(row => row.actor_id, {
    id: 'actor',
    header: 'Actor',
    cell: ({ row }) => {
      const actorId = row.original.actor_id
      if (!actorId) return h('span', { class: 'text-xs text-muted-foreground' }, '—')
      const actorType = row.original.actor_type || 'human_user'
      return h(RouterLink, {
        to: `/users/${actorId}`,
        class: 'inline-flex items-center gap-1 text-xs font-mono text-primary hover:underline max-w-[160px] truncate',
        title: actorId,
      }, () => [
        actorId.slice(0, 12) + '…',
        h(ExternalLink, { class: 'size-3 opacity-50 shrink-0' })
      ])
    },
  }),
  columnHelper.accessor(row => `${row.aggregate_type}:${row.aggregate_id}`, {
    id: 'aggregate',
    header: ({ column }) => h('span', { class: 'text-xs whitespace-nowrap' }, 'Aggregate'),
    cell: ({ row }) => h('div', { class: 'flex items-center space-x-2' }, [
      h(LayoutList, { class: 'w-4 h-4 text-muted-foreground shrink-0' }),
      h('div', { class: 'flex flex-col' }, [
        h('span', { class: 'text-[11px] font-mono text-muted-foreground uppercase opacity-80 leading-none' }, row.original.aggregate_type),
        h('span', { class: 'text-xs font-medium truncate max-w-[120px] leading-tight', title: row.original.aggregate_id }, row.original.aggregate_id)
      ])
    ]),
  }),
  columnHelper.accessor('created_at', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Created', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => {
      if (!info.getValue()) return h('span', '—')
      const d = new Date(info.getValue())
      return h('div', { class: 'flex flex-col text-sm whitespace-nowrap' }, [
        h('span', d.toLocaleDateString()),
        h('span', { class: 'text-xs text-muted-foreground' }, d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', fractionalSecondDigits: 3 }))
      ])
    },
  }),
  // Hidden-by-default wide event columns (toggle via View dropdown)
  columnHelper.accessor('request_id', {
    id: 'request_id',
    header: 'Request ID',
    cell: info => {
      const val = info.getValue()
      if (!val) return h('span', { class: 'text-xs text-muted-foreground' }, '—')
      return h(RouterLink, {
        to: `/console/traces?id=${val}`,
        class: 'text-xs font-mono text-primary hover:underline truncate max-w-[120px] block',
        title: val,
      }, () => val.slice(0, 12) + '…')
    },
    enableHiding: true,
    meta: { defaultHidden: true },
  }),
  columnHelper.accessor('client_id', {
    id: 'client_id',
    header: 'Client',
    cell: info => h('span', { class: 'text-xs font-mono truncate max-w-[100px] block' }, info.getValue() || '—'),
    enableHiding: true,
    meta: { defaultHidden: true },
    filterFn: 'includesString',
  }),
  columnHelper.accessor('delegation_type', {
    id: 'delegation',
    header: 'Delegation',
    cell: info => {
      const val = info.getValue()
      if (!val || val === 'direct') return h('span', { class: 'text-xs text-muted-foreground' }, 'direct')
      return h(Badge, { variant: 'outline', class: 'text-[10px] border-dashed text-amber-600 border-amber-200 bg-amber-50' }, () => val)
    },
    enableHiding: true,
    meta: { defaultHidden: true },
    filterFn: 'includesString',
  }),
  columnHelper.accessor(row => row.sdk_name ? `${row.sdk_name} ${row.sdk_version || ''}`.trim() : '', {
    id: 'sdk',
    header: 'SDK',
    cell: info => h('span', { class: 'text-xs font-mono truncate max-w-[100px] block' }, info.getValue() || '—'),
    enableHiding: true,
    meta: { defaultHidden: true },
  }),
  columnHelper.accessor('fingerprint', {
    id: 'fingerprint',
    header: 'Fingerprint',
    cell: info => {
      const val = info.getValue()
      if (!val) return h('span', { class: 'text-xs text-muted-foreground' }, '—')
      return h(RouterLink, {
        to: `/console/events?fingerprint=${val}`,
        class: 'text-xs font-mono text-primary hover:underline truncate max-w-[80px] block',
        title: val,
      }, () => val.slice(0, 8) + '…')
    },
    enableHiding: true,
    meta: { defaultHidden: true },
    filterFn: 'includesString',
  }),
  columnHelper.display({
    id: 'actions',
    header: () => null,
    cell: ({ row }) => h('div', { class: 'flex items-center space-x-1 justify-end' }, [
      h(DropdownMenu, {}, () => [
        h(DropdownMenuTrigger, { asChild: true }, () => 
          h('button', { class: 'text-muted-foreground hover:text-foreground hover:bg-muted p-1.5 rounded-md transition-colors' }, [
             h(MoreHorizontal, { class: 'w-4 h-4' })
          ])
        ),
        h(DropdownMenuContent, { align: 'end' }, () => [
          h(DropdownMenuItem, { 
            class: 'cursor-pointer',
            onClick: () => {
              const url = `${window.location.origin}/console/events?id=${row.original.id}`
              navigator.clipboard.writeText(url)
            }
          }, () => [
            h(Link2, { class: 'w-3.5 h-3.5 mr-2' }),
            'Copy Permalink'
          ]),
          h(DropdownMenuItem, { asChild: true, class: 'cursor-pointer' }, () => 
            h(RouterLink, { 
              to: {
                path: '/console/observability/explore',
                query: {
                  table: 'events',
                  func: 'NONE',
                  mcol: '*',
                  filters: JSON.stringify([{ col: 'event_type', op: '=', val: row.original.event_type }])
                }
              }
            }, () => 'Explore Event Type')
          )
        ])
      ])
    ]),
    meta: { class: 'w-16 text-right' }
  })
]
</script>
