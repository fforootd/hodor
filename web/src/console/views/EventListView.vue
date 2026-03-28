<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Events</h1>
      <p class="text-muted-foreground mt-1 text-sm">Audit log of all system events ({{ totalCount }} loaded)</p>
    </div>

    <!-- Stats Bar -->
    <div class="flex items-center gap-6 p-4 rounded-lg border text-sm bg-card text-muted-foreground">
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

    <DataTable 
      v-if="events.length > 0"
      :columns="columns as any" 
      :data="events" 
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
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
                  </div>
                  
                  <div v-if="currentFilterPrefix === 'type:'">
                    <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">Event Types</div>
                    <button v-for="t in eventTypes" :key="t" class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken(`type:${t} `)">{{ t }}</button>
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
              <DropdownMenuCheckboxItem
                v-for="column in table.getAllColumns().filter((col: any) => col.getCanHide())"
                :key="column.id"
                class="capitalize"
                :checked="table.getState().columnVisibility[column.id] !== false"
                @update:checked="(val: boolean) => column.toggleVisibility(!!val)"
              >
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
                  <h4 class="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-1">Aggregate Topology</h4>
                  <p class="text-xs font-mono">{{ row.original.aggregate_type }} <span class="text-muted-foreground">→</span> <RouterLink :to="`/console/s/${row.original.aggregate_type}/${row.original.aggregate_id}`" class="text-primary hover:underline">{{ row.original.aggregate_id }}</RouterLink></p>
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
import { ref, onMounted, computed, h } from 'vue'
import { onClickOutside } from '@vueuse/core'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { eventApi, type Event } from '@/api/resources'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, DropdownMenuCheckboxItem } from '@/components/ui/dropdown-menu'
import { 
  Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown, Activity, Key, Globe, LayoutList, MoreHorizontal, FileJson, ExternalLink
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

const route = useRoute()
const router = useRouter()

const events = ref<Event[]>([])
const selectedRows = ref({})
const globalSearch = ref('')
const isSearchOpen = ref(false)
const eventTypes = ref<string[]>([])

const searchInputRef = ref<any>(null)
const searchContainerRef = ref<HTMLElement | null>(null)

onClickOutside(searchContainerRef, () => {
  isSearchOpen.value = false
})

let activeTable: any = null

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

onMounted(async () => {
  try { 
    const session_id = route.query.session_id as string | undefined
    const res = await eventApi.list({ limit: 500, session_id })
    events.value = res
    eventTypes.value = [...new Set(res.map(e => e.event_type))]
    
    // Auto-apply session filter chip to the UI if present
    if (session_id) {
       globalSearch.value = `session:${session_id} `
       // Note: we don't need to applySearchQuery filtering on 'session' column locally 
       // because the backend already pre-filtered it. But leaving the chip shows the user it is filtered!
       // But let's actually make sure applying it locally doesn't break if `session` isn't a column!
    }
  } catch (err) {
    console.error('Failed to load events', err)
  }
})

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
        to: `/console/s/${actorType}/${actorId}`,
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
          h(DropdownMenuItem, { asChild: true, class: 'cursor-pointer' }, () => 
            h(RouterLink, { 
              to: {
                path: '/observability/explore',
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
