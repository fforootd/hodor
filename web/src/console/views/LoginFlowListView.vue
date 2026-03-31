<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Login Flows</h1>
        <p class="text-sm text-muted-foreground">
          {{ loading ? 'Loading…' : `${flows.length} flow${flows.length !== 1 ? 's' : ''}` }}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button @click="$router.push('/marketplace?type=login_flow')">
          <Store class="size-4 mr-2" />
          Use Template
        </Button>
        <Button variant="outline" @click="showCreateDialog = true">
          <Plus class="size-4 mr-2" />
          Start Manually
        </Button>
      </div>
    </div>

    <Empty v-if="!loading && flows.length === 0" class="border border-dashed">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Shield />
        </EmptyMedia>
        <EmptyTitle>No Login Flows</EmptyTitle>
        <EmptyDescription>Something went wrong — the default flow should always exist.</EmptyDescription>
      </EmptyHeader>
    </Empty>

    <div v-if="loading" class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>

    <DataTable
      v-if="!loading && flows.length > 0"
      :columns="columns as any"
      :data="filteredItems"
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <div class="w-full max-w-lg relative">
            <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
            <Input
              placeholder="Search flows…"
              class="pl-9 bg-background w-full relative z-0"
              :model-value="globalSearch"
              @update:model-value="val => applySearchQuery(String(val), table)"
            />
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

    <Dialog v-model:open="showCreateDialog">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Start Login Flow Manually</DialogTitle>
          <DialogDescription>
            Templates are recommended for complete starting points. Manual setup creates a blank
            flow with the defaults you can refine in the editor.
          </DialogDescription>
        </DialogHeader>
        <form class="space-y-4" @submit.prevent="createFlow">
          <div class="space-y-1.5">
            <Label for="flow-name">Name</Label>
            <Input id="flow-name" v-model="newFlow.name" placeholder="e.g. B2B Beta Login" required />
          </div>
          <div class="space-y-1.5">
            <Label for="flow-state">Start state</Label>
            <select
              id="flow-state"
              v-model="newFlow.state"
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="draft">Draft (not served)</option>
              <option value="testing">Testing (served to user allowlist only)</option>
              <option value="active">Active (served to all matching users)</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <Label for="flow-strategy">Flow Strategy</Label>
            <select
              id="flow-strategy"
              v-model="newFlow.strategy"
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="identifier_first">Identifier first</option>
              <option value="passkey_first">Passkey first</option>
              <option value="sso_only">SSO only</option>
              <option value="custom">Custom</option>
            </select>
            <p class="text-xs text-muted-foreground">
              Layout defaults to centered. You can edit layout and protections after creation.
            </p>
          </div>
          <div class="flex justify-end gap-2 pt-2">
            <Button type="button" variant="outline" @click="showCreateDialog = false">Cancel</Button>
            <Button type="submit" :disabled="creating">
              <Spinner v-if="creating" class="size-4 mr-2" />
              Create flow
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, h, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { api } from '@/api/client'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuTrigger, DropdownMenuCheckboxItem,
} from '@/components/ui/dropdown-menu'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription } from '@/components/ui/empty'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import {
  Plus, Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown,
  Shield, Store, CheckCircle2, Ban, Clock,
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

interface LoginFlow {
  id: string
  name: string
  strategy: string
  is_default: boolean
  enabled: boolean
  state: string
  priority: number
  audience: any
  auth_methods: any
  config: any
  metadata?: any
  created_at: string
  updated_at: string
}

const flows = ref<LoginFlow[]>([])
const loading = ref(true)
const showCreateDialog = ref(false)
const creating = ref(false)
const selectedRows = ref({})
const globalSearch = ref('')
const searchQuery = ref('')

const filteredItems = computed(() => {
  if (!searchQuery.value.trim()) return flows.value
  const q = searchQuery.value.toLowerCase()
  return flows.value.filter(f =>
    f.name?.toLowerCase().includes(q) ||
    f.strategy?.toLowerCase().includes(q) ||
    f.state?.toLowerCase().includes(q)
  )
})

const newFlow = ref({
  name: '',
  state: 'draft',
  strategy: 'identifier_first',
})

let activeTable: any = null

function applySearchQuery(query: string, table: any) {
  if (table) activeTable = table
  globalSearch.value = query
  searchQuery.value = query
  const filters: { id: string; value: string }[] = []
  if (query.trim()) {
    filters.push({ id: 'name', value: query.trim() })
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

function getConfig(flow: LoginFlow): any {
  if (!flow.config) return {}
  if (typeof flow.config === 'string') {
    try { return JSON.parse(flow.config) } catch { return {} }
  }
  return flow.config
}

function formatStrategy(strategy: string) {
  switch (strategy) {
    case 'identifier_first': return 'Identifier first'
    case 'passkey_first': return 'Passkey first'
    case 'sso_only': return 'SSO only'
    case 'custom': return 'Custom'
    default: return strategy
  }
}

function getProtections(flow: LoginFlow): string[] {
  const config = getConfig(flow)
  const parts: string[] = []
  if (config.captcha && config.captcha.mode !== 'never' && config.captcha.provider !== 'none') {
    parts.push(config.captcha.provider || 'Captcha')
  }
  if (config.fingerprint && config.fingerprint.enabled !== false) {
    parts.push('Fingerprint')
  }
  if (config.rate_limit) {
    parts.push('Rate limit')
  }
  if (config.telemetry && config.telemetry.enabled !== false) {
    parts.push('Telemetry')
  }
  return parts
}

function safeJSON(value: string) {
  try { return JSON.parse(value) } catch { return {} }
}

function hasAudience(flow: LoginFlow) {
  if (!flow.audience) return false
  const audience = typeof flow.audience === 'string' ? safeJSON(flow.audience) : flow.audience
  return audience.schema_ids?.length > 0 || audience.user_ids?.length > 0 || audience.org_ids?.length > 0
}

function audienceSummary(flow: LoginFlow): string {
  if (!hasAudience(flow)) return ''
  const audience = typeof flow.audience === 'string' ? safeJSON(flow.audience) : (flow.audience || {})
  const parts: string[] = []
  if (audience.schema_ids?.length) parts.push(`${audience.schema_ids.length} schema${audience.schema_ids.length > 1 ? 's' : ''}`)
  if (audience.user_ids?.length) parts.push(`${audience.user_ids.length} user${audience.user_ids.length > 1 ? 's' : ''}`)
  if (audience.org_ids?.length) parts.push(`${audience.org_ids.length} org${audience.org_ids.length > 1 ? 's' : ''}`)
  return parts.join(', ')
}

const columnHelper = createColumnHelper<LoginFlow>()

const columns = computed(() => [
  columnHelper.accessor('name', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
    }, () => ['Name', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => {
      const flow = info.row.original
      const name = info.getValue() || 'Unnamed Flow'
      return h('div', { class: 'flex items-center gap-3' }, [
        h('div', {
          class: 'flex items-center justify-center size-8 rounded-lg text-xs font-semibold ' +
            (flow.is_default ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'),
        }, [h(Shield, { class: 'size-4' })]),
        h('div', { class: 'min-w-0' }, [
          h('div', { class: 'flex items-center gap-2' }, [
            h(RouterLink, {
              to: `/login-flows/${flow.id}`,
              class: 'font-medium text-primary hover:underline',
            }, () => name),
            ...(flow.is_default ? [h(Badge, { variant: 'default', class: 'text-[10px] px-1.5 py-0' }, () => 'Default')] : []),
          ]),
          h('p', { class: 'text-xs text-muted-foreground mt-0.5 truncate' },
            flow.is_default
              ? 'Fallback for all unmatched users'
              : (audienceSummary(flow) || 'No audience targeting')
          ),
        ]),
      ])
    },
    filterFn: (row, _id, filterValue) => {
      const name = (row.original.name || '').toLowerCase()
      return name.includes(filterValue.toLowerCase())
    },
  }),
  columnHelper.accessor('strategy', {
    header: 'Strategy',
    cell: info => h('span', { class: 'text-sm' }, formatStrategy(info.getValue())),
  }),
  columnHelper.accessor('state', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
    }, () => ['State', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => {
      const state = (info.getValue() as string) || 'draft'
      const variant = state === 'active' ? 'outline' : state === 'archived' ? 'outline' : 'outline'
      const colorClass = state === 'active'
        ? 'text-green-700 bg-green-100 border-green-200'
        : state === 'testing'
          ? 'text-amber-700 bg-amber-100 border-amber-200'
          : state === 'archived'
            ? 'text-red-700 bg-red-100 border-red-200'
            : 'text-muted-foreground'
      const icon = state === 'active' ? CheckCircle2 : state === 'archived' ? Ban : Clock
      return h(Badge, {
        variant,
        class: `font-normal flex items-center gap-1 capitalize ${colorClass}`,
      }, () => [
        h(icon, { class: 'w-3 h-3 shrink-0' }),
        h('span', state),
      ])
    },
  }),
  columnHelper.display({
    id: 'protections',
    header: 'Protections',
    cell: ({ row }) => {
      const parts = getProtections(row.original)
      if (parts.length === 0) return h('span', { class: 'text-xs text-muted-foreground' }, '—')
      return h('span', { class: 'text-xs text-muted-foreground' }, parts.join(' · '))
    },
  }),
  columnHelper.accessor('priority', {
    header: 'Priority',
    cell: info => h('span', { class: 'text-sm tabular-nums' }, String(info.getValue() ?? 0)),
    meta: { defaultHidden: true },
  }),
  columnHelper.accessor('created_at', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
    }, () => ['Created', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => {
      if (!info.getValue()) return h('span', '—')
      const d = new Date(info.getValue()!)
      return h('span', { class: 'text-sm text-muted-foreground whitespace-nowrap' }, d.toLocaleDateString())
    },
    meta: { defaultHidden: true },
  }),
])

async function loadFlows() {
  loading.value = true
  try {
    flows.value = (await api.get<{ items: LoginFlow[] }>('/v1/login-flows')).items || []
  } catch {
    flows.value = []
  } finally {
    loading.value = false
  }
}

async function createFlow() {
  creating.value = true
  try {
    await api.post('/v1/login-flows', {
      name: newFlow.value.name,
      strategy: newFlow.value.strategy,
      state: newFlow.value.state,
      config: {
        captcha: {
          provider: 'altcha',
          mode: 'risk_based',
          difficulty: 3,
          steps: ['identifier', 'password'],
        },
        fingerprint: {
          enabled: true,
          provider: 'thumbmarkjs',
          persist: true,
          steps: ['identifier'],
        },
        rate_limit: {
          max_attempts: 5,
          window_seconds: 300,
          lockout_seconds: 900,
          scope: 'ip',
        },
        telemetry: {
          enabled: true,
          sample_rate: 1.0,
        },
        branding: {
          layout: 'centered',
        },
      },
    })
    showCreateDialog.value = false
    newFlow.value = {
      name: '',
      state: 'draft',
      strategy: 'identifier_first',
    }
    await loadFlows()
  } catch (e: any) {
    console.error('Failed to create login flow:', e)
  } finally {
    creating.value = false
  }
}

onMounted(loadFlows)
</script>
