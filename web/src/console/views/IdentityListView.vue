<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">{{ label }}</h1>
        <p class="text-sm text-muted-foreground">{{ identities.length }} {{ label.toLowerCase() }} total</p>
      </div>
      <Button as-child>
        <router-link :to="`/s/${schemaType}/new`">
          <Plus class="mr-2 size-4" />
          New {{ singularLabel }}
        </router-link>
      </Button>
    </div>

    <!-- OIDC Discovery panel (shown for app type) -->
    <Card v-if="schemaType === 'app'" class="bg-muted/50">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm font-medium">OIDC Discovery</CardTitle>
      </CardHeader>
      <CardContent class="space-y-2">
        <div class="flex items-center gap-3">
          <span class="text-sm text-muted-foreground w-20">Issuer</span>
          <code
            class="cursor-pointer rounded bg-primary/10 px-2 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
            @click="copy(issuer)"
          >{{ issuer }}</code>
        </div>
        <div class="flex items-center gap-3">
          <span class="text-sm text-muted-foreground w-20">Discovery</span>
          <code
            class="cursor-pointer rounded bg-primary/10 px-2 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
            @click="copy(issuer + '/.well-known/openid-configuration')"
          >{{ issuer }}/.well-known/openid-configuration</code>
        </div>
      </CardContent>
    </Card>

    <DataTable 
      :columns="columns as any" 
      :data="identities" 
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <!-- Unified Search Bar with Autocomplete Chips -->
          <div class="w-full max-w-lg relative" ref="searchContainerRef">
            <div class="relative w-full">
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
              <Input
                ref="searchInputRef"
                :placeholder="`Search ${label.toLowerCase()} (e.g. name:Alice state:active)`"
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
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('name:')">
                    <span class="font-medium mr-2">name:</span> Search by Display Name
                  </button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('identifier:')">
                    <span class="font-medium mr-2">identifier:</span> Search by Identifier
                  </button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('state:')">
                    <span class="font-medium mr-2">state:</span> Filter by State
                  </button>
                </div>
                
                <div v-if="currentFilterPrefix === 'state:'">
                  <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">State</div>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('state:active ')">Active</button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('state:deactivated ')">Deactivated</button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('state:locked ')">Locked</button>
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

      <template #pagination="{ table }">
        <DataTablePagination :table="table" />
      </template>
    </DataTable>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, h } from 'vue'
import { onClickOutside } from '@vueuse/core'
import { RouterLink } from 'vue-router'
import { type Identity, metaSchemaApi } from '@/api/resources'
import { api } from '@/api/client'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger, DropdownMenuCheckboxItem } from '@/components/ui/dropdown-menu'
import { 
  Plus, Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown, CheckCircle2, XCircle, Ban
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

const props = defineProps<{ schemaType: string }>()

const schemaDisplay = ref<any>({})
const label = computed(() => schemaDisplay.value.alias || props.schemaType.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()) + 's')
const singularLabel = computed(() => schemaDisplay.value.singular || label.value.replace(/s$/, '').replace(/ie$/, 'y'))
const issuer = window.location.origin

const identities = ref<Identity[]>([])
const selectedRows = ref({})
const globalSearch = ref('')
const isSearchOpen = ref(false)

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
   if (lastPart.startsWith('state:')) return 'state:'
   if (lastPart.startsWith('name:')) return 'name:'
   if (lastPart.startsWith('identifier:')) return 'identifier:'
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
      
      if (['name', 'display_name'].includes(key)) filters.push({ id: 'display_name', value })
      else if (['identifier', 'email', 'id'].includes(key)) filters.push({ id: 'identifier', value })
      else if (key === 'state') filters.push({ id: 'state', value })
      else globalText += token + ' '
    } else {
      globalText += token + ' '
    }
  }

  const remainder = globalText.trim()
  if (remainder) {
    if (!filters.find((f: any) => f.id === 'identifier')) {
       filters.push({ id: 'identifier', value: remainder })
    }
  }
  
  if (activeTable) {
    activeTable.setColumnFilters(filters)
  }
}

onMounted(async () => {
  let apiPath = props.schemaType

  try {
    const metaData = await metaSchemaApi.get()

    const catalog = metaData['x-catalog'] || {}
    const entry = catalog[props.schemaType]
    if (entry) {
      schemaDisplay.value = { alias: entry.alias, singular: entry.singular, path: entry.path, icon: entry.icon }
      apiPath = entry.path || props.schemaType
    }
  } catch { /* ignore */ }


  try {
    let url = `/v1/${apiPath}`
    const orgId = localStorage.getItem('zitadel_org')
    if (orgId && apiPath !== 'orgs' && props.schemaType !== 'org') url += `?org_id=${orgId}`

    const data = await api.get<any>(url)

    // Normalize: orgs use `name` whereas the DataTable expects `identifier`/`display_name`.
    const items = data.items || []
    if (props.schemaType === 'org') {
      identities.value = items.map((o: any) => ({
        ...o,
        identifier: o.name || o.identifier || '',
        display_name: o.name || o.display_name || '',
      }))
    } else {
      identities.value = items
    }
  } catch { /* ignore */ }
})

function getField(item: Identity, field: string): string {
  try {
    const d = typeof item.data === 'string' ? JSON.parse(item.data) : (item.data || {})
    return d[field] || ''
  } catch { return '' }
}

function formatUris(item: Identity): string {
  try {
    const d = typeof item.data === 'string' ? JSON.parse(item.data) : (item.data || {})
    const uris = d.redirect_uris || []
    if (uris.length === 0) return '—'
    if (uris.length === 1) return uris[0]
    return `${uris[0]} +${uris.length - 1} more`
  } catch { return '—' }
}

function copy(text: string) { navigator.clipboard.writeText(text) }
function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }

const columnHelper = createColumnHelper<Identity>()

const columns = computed(() => {
  const isApp = props.schemaType === 'app'

  const cols: any[] = [
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
    columnHelper.accessor('identifier', {
      header: ({ column }) => h(Button, {
        variant: 'ghost',
        class: '-ml-4',
        onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
      }, () => ['Identifier', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
      cell: info => h(RouterLink, {
        to: `/users/${info.row.original.id}`,
        class: isApp ? 'font-mono text-sm text-primary hover:underline' : 'font-medium hover:underline'
      }, () => info.getValue()),
    }),
    columnHelper.accessor(row => getField(row, 'client_name') || getField(row, 'display_name') || row.display_name || '—', {
      id: 'display_name',
      header: ({ column }) => h(Button, {
        variant: 'ghost',
        class: '-ml-4',
        onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
      }, () => ['Display Name', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
      cell: info => h('span', { class: 'text-sm' }, info.getValue()),
    }),
  ]

  if (isApp) {
    cols.push(
      columnHelper.display({
        id: 'app_type',
        header: 'Type',
        cell: ({ row }) => h(Badge, { variant: 'outline', class: 'text-xs uppercase' }, () => getField(row.original, 'app_type') || '—'),
      }),
      columnHelper.display({
        id: 'redirect_uris',
        header: 'Redirect URIs',
        cell: ({ row }) => h('span', { class: 'text-sm text-muted-foreground truncate max-w-[300px] inline-block' }, formatUris(row.original)),
      })
    )
  }

  cols.push(
    columnHelper.accessor('state', {
      header: ({ column }) => h(Button, {
        variant: 'ghost',
        class: '-ml-4',
        onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
      }, () => ['State', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
      cell: info => {
        const state = info.getValue() as string
        let colorClass = 'text-green-700 bg-green-100 border-green-200'
        let Icon = CheckCircle2
        
        if (state === 'deactivated' || state === 'locked') {
          colorClass = 'text-red-700 bg-red-100 border-red-200'
          Icon = Ban
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
    })
  )

  return cols
})
</script>
