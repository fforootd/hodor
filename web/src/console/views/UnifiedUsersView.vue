<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Users</h1>
        <p class="text-sm text-muted-foreground">
          Manage all users, service accounts, and AI agents{{ totalCount > 0 ? ` (${totalCount} total)` : '' }}
        </p>
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <Button>
            <Plus class="mr-2 size-4" />
            Create
            <ChevronDown class="ml-2 size-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem @click="isCreateWizardOpen = true">
            <Users class="mr-2 size-4" />
            New User
          </DropdownMenuItem>
          <DropdownMenuItem as-child>
            <router-link to="/s/service_user/new">
              <KeyRound class="mr-2 size-4" />
              New Service Account
            </router-link>
          </DropdownMenuItem>
          <DropdownMenuItem as-child>
            <router-link to="/s/ai_agent/new">
              <Bot class="mr-2 size-4" />
              New AI Agent
            </router-link>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>

    <!-- Tabs -->
    <Tabs :default-value="activeTab" @update:model-value="(val: any) => activeTab = String(val)">
      <TabsList>
        <TabsTrigger value="all">
          All
          <Badge v-if="totalCount > 0" variant="secondary" class="ml-1.5 text-xs px-1.5 py-0">{{ totalCount }}</Badge>
        </TabsTrigger>
        <TabsTrigger value="human_user">
          <Users class="mr-1.5 size-3.5" />
          Users
          <Badge v-if="typeCounts.human_user" variant="secondary" class="ml-1.5 text-xs px-1.5 py-0">{{ typeCounts.human_user }}</Badge>
        </TabsTrigger>
        <TabsTrigger value="service_user">
          <KeyRound class="mr-1.5 size-3.5" />
          Service Accounts
          <Badge v-if="typeCounts.service_user" variant="secondary" class="ml-1.5 text-xs px-1.5 py-0">{{ typeCounts.service_user }}</Badge>
        </TabsTrigger>
        <TabsTrigger value="ai_agent">
          <Bot class="mr-1.5 size-3.5" />
          AI Agents
          <Badge v-if="typeCounts.ai_agent" variant="secondary" class="ml-1.5 text-xs px-1.5 py-0">{{ typeCounts.ai_agent }}</Badge>
        </TabsTrigger>
      </TabsList>
    </Tabs>

    <DataTable
      :columns="columns as any"
      :data="filteredIdentities"
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <div class="w-full max-w-lg relative" ref="searchContainerRef">
            <div class="relative w-full">
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
              <Input
                :placeholder="`Search by name or identifier…`"
                class="pl-9 bg-background w-full relative z-0"
                :model-value="globalSearch"
                @update:model-value="val => applySearchQuery(String(val), table)"
              />
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

      <template #pagination="{ table }">
        <DataTablePagination :table="table" />
      </template>
    </DataTable>
    <CreateUserWizard v-model:open="isCreateWizardOpen" @created="onUserCreated" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, h, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { type Identity, metaSchemaApi } from '@/api/resources'
import { api } from '@/api/client'
import CreateUserWizard from '@/console/components/CreateUserWizard.vue'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuTrigger,
  DropdownMenuCheckboxItem, DropdownMenuItem,
} from '@/components/ui/dropdown-menu'
import {
  Plus, Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown,
  CheckCircle2, Ban, Users, KeyRound, Bot,
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

interface IdentityWithType extends Identity {
  _schemaType?: string
}

const activeTab = ref('all')
const allIdentities = ref<IdentityWithType[]>([])
const selectedRows = ref({})
const globalSearch = ref('')
const loading = ref(true)
const isCreateWizardOpen = ref(false)

// Schema type display metadata
const typeLabels: Record<string, string> = {
  human_user: 'User',
  service_user: 'Service',
  ai_agent: 'Agent',
}

const typeIcons: Record<string, any> = {
  human_user: Users,
  service_user: KeyRound,
  ai_agent: Bot,
}

const typeBadgeClass: Record<string, string> = {
  human_user: 'bg-blue-50 text-blue-700 border-blue-200',
  service_user: 'bg-amber-50 text-amber-700 border-amber-200',
  ai_agent: 'bg-violet-50 text-violet-700 border-violet-200',
}

// Computed
const typeCounts = computed(() => {
  const counts: Record<string, number> = {}
  for (const item of allIdentities.value) {
    const t = item._schemaType || 'unknown'
    counts[t] = (counts[t] || 0) + 1
  }
  return counts
})

const totalCount = computed(() => allIdentities.value.length)

const filteredIdentities = computed(() => {
  if (activeTab.value === 'all') return allIdentities.value
  return allIdentities.value.filter(i => i._schemaType === activeTab.value)
})

// Search
let activeTable: any = null

function applySearchQuery(query: string, table: any) {
  if (table) activeTable = table
  globalSearch.value = query
  const filters: { id: string; value: string }[] = []
  if (query.trim()) {
    filters.push({ id: 'identifier', value: query.trim() })
  }
  if (activeTable) {
    activeTable.setColumnFilters(filters)
  }
}

function getSortIcon(column: any) {
  const isSorted = column.getIsSorted()
  if (isSorted === 'asc') return ArrowUp
  if (isSorted === 'desc') return ArrowDown
  return ArrowUpDown
}

// Data loading
const identityTypes = ['human_user', 'service_user', 'ai_agent']

async function loadIdentities() {
  loading.value = true
  // Resolve paths from meta schema
  let typePathMap: Record<string, string> = {}
  try {
    const meta = await metaSchemaApi.get() as any
    const catalog = meta['x-catalog'] || {}
    for (const typeName of identityTypes) {
      const entry = catalog[typeName]
      if (entry?.path) {
        typePathMap[typeName] = entry.path
      }
    }
  } catch { /* fallback to defaults */ }

  // Parallel fetch all identity types
  const orgId = localStorage.getItem('zitadel_org')
  const qs = orgId ? `?org_id=${orgId}` : ''

  const results = await Promise.allSettled(
    identityTypes.map(async (typeName) => {
      const path = typePathMap[typeName] || typeName
      const data = await api.get<any>(`/v1/${path}${qs}`)
      return (data.items || []).map((item: any) => ({ ...item, _schemaType: typeName }))
    })
  )

  const merged: IdentityWithType[] = []
  for (const result of results) {
    if (result.status === 'fulfilled') {
      merged.push(...result.value)
    }
  }

  allIdentities.value = merged
  loading.value = false
}

onMounted(async () => {
  await loadIdentities()
})

const onUserCreated = async (id: string) => {
  // optionally navigate or just refresh
  await loadIdentities()
}

// Watch tab changes to reset search
watch(activeTab, () => {
  if (activeTable) {
    activeTable.setColumnFilters([])
    globalSearch.value = ''
  }
})

function getField(item: Identity, field: string): string {
  try {
    const d = typeof item.data === 'string' ? JSON.parse(item.data) : (item.data || {})
    return d[field] || ''
  } catch { return '' }
}

function formatTime(ts: string) {
  if (!ts) return '—'
  const d = new Date(ts)
  return d.toLocaleDateString()
}

const columnHelper = createColumnHelper<IdentityWithType>()

const columns = computed(() => [
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
      to: `/identities/${info.row.original.id}`,
      class: 'font-medium hover:underline'
    }, () => info.getValue()),
  }),
  columnHelper.accessor(row => getField(row, 'display_name') || row.display_name || '—', {
    id: 'display_name',
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Name', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => h('span', { class: 'text-sm' }, info.getValue()),
  }),
  columnHelper.accessor(row => row._schemaType || 'unknown', {
    id: 'type',
    header: 'Type',
    cell: info => {
      const t = info.getValue()
      const label = typeLabels[t] || t
      const cls = typeBadgeClass[t] || 'bg-gray-50 text-gray-700 border-gray-200'
      const Icon = typeIcons[t] || Users
      return h(Badge, { variant: 'outline', class: `font-normal flex items-center gap-1 ${cls} capitalize whitespace-nowrap` }, () => [
        h(Icon, { class: 'w-3 h-3 shrink-0' }),
        h('span', label),
      ])
    },
    enableHiding: true,
  }),
  columnHelper.accessor('state', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Status', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
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
        h('span', state),
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
        h('span', { class: 'text-xs text-muted-foreground' }, d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })),
      ])
    },
  }),
])
</script>
