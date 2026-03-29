<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Sessions</h1>
      <p class="text-muted-foreground mt-1 text-sm">User authentication events ({{ activeCount }} active of {{ totalCount }} total)</p>
    </div>

    <!-- Stats Bar -->
    <div class="flex items-center gap-6 p-4 rounded-lg border text-sm bg-card text-muted-foreground">
      <div class="flex items-center space-x-2 text-green-700">
        <CheckCircle2 class="w-4 h-4" />
        <span class="font-medium">{{ activeCount }} active</span>
      </div>
      <div class="w-px h-4 bg-border"></div>
      <div class="flex items-center space-x-2">
        <Clock class="w-4 h-4" />
        <span>{{ expiredCount }} expired</span>
      </div>
      <div class="w-px h-4 bg-border"></div>
      <div class="flex items-center space-x-2 text-red-600">
        <Ban class="w-4 h-4" />
        <span>{{ revokedCount }} revoked</span>
      </div>
    </div>

    <DataTable 
      :columns="columns as any" 
      :data="sessions" 
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
        <div style="display:none">{{ __setTable(table) }}</div>
        <div class="flex items-center justify-between w-full mb-4">
          <!-- Unified Search Bar with Autocomplete Chips -->
          <div class="w-full max-w-lg relative" ref="searchContainerRef">
                <div class="relative w-full">
                  <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
                  <Input
                    ref="searchInputRef"
                    placeholder="Search (e.g. user:james IP:192.168 device:Chrome)"
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
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('user:')">
                      <span class="font-medium mr-2">user:</span> Search by User
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('ip:')">
                      <span class="font-medium mr-2">ip:</span> Filter by IP Address
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('device:')">
                      <span class="font-medium mr-2">device:</span> Filter by User Agent
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('org:')">
                      <span class="font-medium mr-2">org:</span> Filter by Organization
                    </button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('status:')">
                      <span class="font-medium mr-2">status:</span> Filter by State
                    </button>
                  </div>
                  
                  <div v-if="currentFilterPrefix === 'status:'">
                    <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">Status</div>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('status:active ')">Active</button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('status:expired ')">Expired</button>
                    <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('status:revoked ')">Revoked</button>
                  </div>
                </div>
              </div>
          </div>
          
          <!-- View Columns Dropdown -->
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
        <div class="grid grid-cols-2 lg:grid-cols-4 gap-4 text-sm">
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Session ID</span>
            <code class="block rounded bg-muted px-2 py-1 text-xs font-mono break-all">{{ row.original.id }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Entity ID</span>
            <RouterLink 
              :to="`/console/s/human_user/${row.original.entity_id || row.original.identity_id}`" 
              class="block rounded bg-muted px-2 py-1 text-xs font-mono break-all text-primary hover:underline"
            >{{ row.original.entity_id || row.original.identity_id || '—' }}</RouterLink>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">IP Address</span>
            <code class="block rounded bg-muted px-2 py-1 text-xs font-mono">{{ row.original.ip_address || '—' }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">User Agent</span>
            <p class="rounded bg-muted px-2 py-1 text-xs break-all">{{ row.original.user_agent || '—' }}</p>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Created</span>
            <p class="text-xs">{{ row.original.created_at ? new Date(row.original.created_at).toLocaleString() : '—' }}</p>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Expires</span>
            <p class="text-xs">{{ row.original.expires_at ? new Date(row.original.expires_at).toLocaleString() : '—' }}</p>
          </div>
          <div class="space-y-1" v-if="row.original.revoked_at">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Revoked</span>
            <p class="text-xs text-destructive">{{ new Date(row.original.revoked_at).toLocaleString() }}</p>
          </div>
          <div class="space-y-1" v-if="row.original.auth_method">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Auth Method</span>
            <div class="flex flex-wrap gap-1">
              <Badge v-for="m in (Array.isArray(row.original.auth_method) ? row.original.auth_method : [row.original.auth_method])" :key="m" variant="outline" class="text-xs">{{ m }}</Badge>
            </div>
          </div>
          <div class="space-y-1" v-if="row.original.geo">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Geo</span>
            <p class="text-xs">{{ row.original.geo.city || '' }} {{ row.original.geo.country || '' }}</p>
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
import { RouterLink, useRoute } from 'vue-router'
import { sessionApi, entityApi, type Session } from '@/api/resources'
import type { IdentityResponse } from '@zitadel/client-js'

/** Session with a computed state field and optional server-side extras. */
type SessionWithState = Session & { state: string } & Record<string, any>
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Popover, PopoverAnchor, PopoverTrigger, PopoverContent } from '@/components/ui/popover'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { 
  Key, Monitor, Search, ChevronRight,
  CheckCircle2, XCircle, Clock, MoreHorizontal, Ban, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, DropdownMenuCheckboxItem } from '@/components/ui/dropdown-menu'

const sessions = ref<SessionWithState[]>([])
const selectedRows = ref({})
const userDict = ref<Record<string, {name: string, identifier: string}>>({})
const globalSearch = ref('')
const isSearchOpen = ref(false)

const route = useRoute()

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
   if (lastPart.startsWith('status:')) return 'status:'
   if (lastPart.startsWith('user:')) return 'user:'
   if (lastPart.startsWith('ip:')) return 'ip:'
   if (lastPart.startsWith('device:')) return 'device:'
   if (lastPart.startsWith('org:')) return 'org:'
   return ''
})

function appendSearchToken(token: string) {
  const parts = globalSearch.value.split(' ')
  const lastPart = parts[parts.length - 1]
  
  if (currentFilterPrefix.value && token.startsWith(currentFilterPrefix.value)) {
     parts[parts.length - 1] = token
  } else {
     if (lastPart && !lastPart.includes(':')) {
        parts.pop() // remove partial word if replacing with a chip
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
      
      if (['ip', 'ip_address'].includes(key)) filters.push({ id: 'ip_address', value })
      else if (['user', 'email', 'name'].includes(key)) filters.push({ id: 'user', value })
      else if (['device', 'useragent', 'ua'].includes(key)) filters.push({ id: 'user_agent', value })
      else if (key === 'status') filters.push({ id: 'status', value })
      else if (['org', 'organization'].includes(key)) filters.push({ id: 'organization', value })
      else globalText += token + ' '
    } else {
      globalText += token + ' '
    }
  }

  const remainder = globalText.trim()
  if (remainder) {
    if (!filters.find((f: any) => f.id === 'user')) {
       filters.push({ id: 'user', value: remainder })
    }
  }
  
  if (activeTable) {
    activeTable.setColumnFilters(filters)
  }
}

function parseDevice(ua: string | undefined): { icon: any, label: string } {
  if (!ua) return { icon: Monitor, label: 'Unknown' }
  const lowercase = ua.toLowerCase()
  if (lowercase.includes('chrome')) return { icon: Monitor, label: 'Chrome' }
  if (lowercase.includes('safari')) return { icon: Monitor, label: 'Safari' }
  if (lowercase.includes('firefox')) return { icon: Monitor, label: 'Firefox' }
  if (lowercase.includes('edge')) return { icon: Monitor, label: 'Edge' }
  return { icon: Monitor, label: 'Web' }
}

onMounted(async () => {
  try { 
    const [sessRes, entitiesRes] = await Promise.all([
      sessionApi.list(),
      entityApi.list().catch(() => []) 
    ])
    
    sessions.value = sessRes.map((s: Session) => {
      let state = 'active'
      if (s.revoked_at) state = 'revoked'
      else if (s.expires_at && new Date(s.expires_at) < new Date()) state = 'expired'
      return { ...s, state }
    })
    
    const dict: Record<string, {name: string, identifier: string}> = {}
    for (const ent of entitiesRes as IdentityResponse[]) {
      const profileEmail = (ent as any).profile?.email as string | undefined
      dict[ent.id] = { 
        name: ent.display_name || 'Unknown User', 
        identifier: profileEmail || ent.identifier || ent.id
      }
    }
    userDict.value = dict

    const user = route.query.user as string | undefined
    if (user) {
        globalSearch.value = `user:${user} `
        if (activeTable) applySearchQuery(globalSearch.value, activeTable)
    }
  } catch (err) {
    console.error('Failed to load sessions', err)
  }
})

async function revoke(id: string) {
  try {
    await sessionApi.revoke(id)
    const index = sessions.value.findIndex(s => s.id === id)
    if (index !== -1) {
      const newSessions = [...sessions.value]
      newSessions[index] = {
        ...newSessions[index],
        state: 'revoked',
        revoked_at: new Date().toISOString()
      }
      sessions.value = newSessions
    }
  } catch (err) {
    console.error("Failed to revoke session", err)
    // Refresh to sync state if we got a 404 because it was already revoked
    const sessRes = await sessionApi.list()
    sessions.value = sessRes.map((s: Session) => ({
      ...s,
      state: s.revoked_at ? 'revoked' : (s.expires_at && new Date(s.expires_at) < new Date() ? 'expired' : 'active')
    }))
  }
}

function formatDateOnly(ts?: string) { 
  if (!ts) return '—'
  return new Date(ts).toLocaleDateString()
}

const activeCount = computed(() => sessions.value.filter(s => s.state === 'active' || !s.state).length)
const expiredCount = computed(() => sessions.value.filter(s => s.state === 'expired').length)
const revokedCount = computed(() => sessions.value.filter(s => s.state === 'revoked').length)
const totalCount = computed(() => sessions.value.length)

const columnHelper = createColumnHelper<SessionWithState>()

const columns = [
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
  columnHelper.display({
    id: 'expander',
    header: () => null,
    cell: ({ row }) => h('button', {
      class: 'p-1 rounded-md hover:bg-muted transition-all',
      onClick: (e: Event) => { e.stopPropagation(); row.toggleExpanded() },
    }, [
      h(ChevronRight, { class: `w-4 h-4 text-muted-foreground transition-transform duration-200 ${row.getIsExpanded() ? 'rotate-90' : ''}` })
    ]),
    meta: { class: 'w-8 px-0' },
    enableSorting: false,
    enableHiding: false,
  }),
  columnHelper.accessor('id', {
    header: 'Session ID',
    cell: info => h('span', { class: 'text-sm font-mono text-muted-foreground truncate max-w-[120px] inline-block', title: info.getValue() }, info.getValue() || '—'),
  }),
  columnHelper.accessor(row => row.entity_id, {
    id: 'user',
    header: 'User',
    cell: ({ row, getValue }) => {
      const fallbackId = getValue() as string
      const entityId = row.original.entity_id || ''
      const entInfo = userDict.value[entityId]
      
      const displayName = entInfo?.name || 'Unknown User'
      const displaySub = entInfo?.identifier || fallbackId
      
      return h('div', { class: 'flex items-center space-x-3 py-1' }, [
        h('div', { class: 'p-1.5 bg-muted rounded-md shrink-0' }, [h(Key, { class: 'w-4 h-4 text-muted-foreground' })]),
        h('div', { class: 'flex flex-col min-w-0 max-w-[200px]' }, [
          h(RouterLink, { to: `/identities/${entityId}`, class: 'text-sm font-medium hover:underline truncate' }, () => displayName),
          h('span', { class: 'text-xs text-muted-foreground truncate', title: displaySub }, displaySub)
        ])
      ])
    },
  }),
  columnHelper.accessor(() => '', {
    id: 'organization',
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Organization', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: () => h(Badge, { variant: 'secondary', class: 'font-normal bg-muted text-muted-foreground whitespace-nowrap' }, () => 'Acme Corp'),
  }),
  columnHelper.accessor('ip_address', {
    id: 'ip_address',
    header: 'IP Address',
    cell: info => h('span', { class: 'text-sm font-mono whitespace-nowrap' }, info.getValue() || '—'),
  }),
  columnHelper.accessor('user_agent', {
    header: 'Device',
    cell: info => {
      const device = parseDevice(info.getValue())
      return h('div', { class: 'flex items-center space-x-2 text-muted-foreground text-sm whitespace-nowrap' }, [
        h(device.icon, { class: 'w-4 h-4 shrink-0' }),
        h('span', {}, device.label)
      ])
    },
  }),
  columnHelper.accessor(row => row.state || 'active', {
    id: 'status',
    header: 'Status',
    cell: info => {
      const state = info.getValue() as string
      let Icon = CheckCircle2
      let colorClass = 'text-green-700 bg-green-100 border-green-200'
      
      if (state === 'revoked') {
        Icon = Ban
        colorClass = 'text-red-700 bg-red-100 border-red-200'
      } else if (state === 'expired') {
        Icon = Clock
        colorClass = 'text-gray-700 bg-gray-100 border-gray-200 animate-pulse'
      }
      
      return h(Badge, { variant: 'outline', class: `font-normal flex items-center space-x-1 ${colorClass} capitalize whitespace-nowrap` }, () => [
        h(Icon, { class: 'w-3 h-3 mr-1 shrink-0' }),
        h('span', state)
      ])
    },
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
        h('span', { class: 'text-xs text-muted-foreground' }, d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }))
      ])
    },
  }),
  columnHelper.accessor('expires_at', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Expires', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => h('span', { class: 'text-sm whitespace-nowrap' }, formatDateOnly(info.getValue())),
  }),
  columnHelper.display({
    id: 'actions',
    header: () => null,
    cell: ({ row }) => h('div', { class: 'flex items-center space-x-1 justify-end' }, [
      row.original.state === 'active' ? h('button', {
        class: 'text-red-500 hover:text-red-700 flex items-center justify-center p-1.5 rounded-md hover:bg-red-50 transition-colors',
        title: 'Revoke',
        onClick: () => revoke(row.original.id)
      }, [h(XCircle, { class: 'w-4 h-4' })]) : null,
      h(DropdownMenu, {}, () => [
        h(DropdownMenuTrigger, { asChild: true }, () => 
          h('button', { class: 'text-muted-foreground hover:text-foreground hover:bg-muted p-1.5 rounded-md transition-colors' }, [
             h(MoreHorizontal, { class: 'w-4 h-4' })
          ])
        ),
        h(DropdownMenuContent, { align: 'end' }, () => [
          row.original.state === 'active' ? h(DropdownMenuItem, { class: 'text-destructive font-medium cursor-pointer', onClick: () => revoke(row.original.id) }, () => 'Revoke Session') : null,
          h(DropdownMenuItem, { asChild: true, class: 'cursor-pointer' }, () => 
            h(RouterLink, { 
              to: {
                path: '/events',
                query: { session_id: row.original.id }
              }
            }, () => 'View Events')
          ),
          h(DropdownMenuItem, { asChild: true, class: 'cursor-pointer' }, () => 
            h(RouterLink, { 
              to: {
                path: '/traces',
                query: { id: row.original.id }
              }
            }, () => 'View Traces')
          )
        ])
      ])
    ]),
    meta: { class: 'w-24 text-right' }
  })
]
</script>
