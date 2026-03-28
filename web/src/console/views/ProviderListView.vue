<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Providers</h1>
        <p class="text-sm text-muted-foreground">{{ providers.length }} provider{{ providers.length !== 1 ? 's' : '' }} configured</p>
      </div>
      <Button v-if="!showCreate" @click="showCreate = true">
        <Plus class="mr-2 size-4" />
        Add Provider
      </Button>
      <Button v-else variant="outline" @click="showCreate = false; selectedTemplate = null">Cancel</Button>
    </div>

    <!-- Template Picker -->
    <div v-if="showCreate && !selectedTemplate" class="space-y-3">
      <h3 class="text-sm font-semibold">Choose a provider template</h3>
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        <Card
          v-for="t in templates" :key="t.id"
          class="cursor-pointer transition-colors hover:border-primary"
          @click="pickTemplate(t)"
        >
          <CardContent class="relative p-4">
            <div class="text-2xl mb-2">{{ templateIcon(t.id) }}</div>
            <div class="font-semibold text-sm">{{ t.name }}</div>
            <p class="text-xs text-muted-foreground mt-1 leading-relaxed">{{ t.description }}</p>
            <Badge variant="secondary" class="absolute top-3 right-3 text-[10px] uppercase">{{ t.protocol }}</Badge>
          </CardContent>
        </Card>
      </div>
    </div>

    <!-- Create Form -->
    <Card v-if="showCreate && selectedTemplate">
      <CardHeader>
        <CardTitle>Configure {{ selectedTemplate.name }} Provider</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-2">
            <Label for="prov-name">Name</Label>
            <Input id="prov-name" v-model="createForm.name" placeholder="e.g. Google Production" />
          </div>
          <div class="space-y-2">
            <Label for="prov-issuer">Issuer</Label>
            <Input id="prov-issuer" v-model="createForm.issuer" placeholder="https://accounts.google.com" />
          </div>
          <div class="space-y-2">
            <Label for="prov-client">Client ID</Label>
            <Input id="prov-client" v-model="createForm.client_id" placeholder="your-client-id" />
          </div>
          <div class="space-y-2">
            <Label for="prov-secret">Client Secret</Label>
            <Input id="prov-secret" v-model="createForm.client_secret" type="password" placeholder="your-client-secret" />
          </div>
          <div class="space-y-2">
            <Label for="prov-scopes">Scopes</Label>
            <Input id="prov-scopes" v-model="createForm.scopes" placeholder="openid email profile" />
          </div>
          <div class="flex items-center gap-2 self-end pb-0.5">
            <input type="checkbox" id="prov-auto" v-model="createForm.auto_register" class="accent-primary" />
            <Label for="prov-auto" class="font-normal cursor-pointer">Auto-register new users</Label>
          </div>
        </div>

        <div v-if="selectedTemplate.claim_overrides && Object.keys(selectedTemplate.claim_overrides).length" class="rounded-lg bg-muted p-3 space-y-1">
          <h4 class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Default Claim Overrides</h4>
          <div v-for="(expr, field) in selectedTemplate.claim_overrides" :key="field" class="text-sm">
            <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ field }}</code>
            →
            <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ expr }}</code>
          </div>
        </div>

        <Separator />

        <div class="flex justify-end gap-3">
          <Button variant="outline" @click="selectedTemplate = null">← Back</Button>
          <Button @click="createProvider" :disabled="!createForm.name || !createForm.issuer || !createForm.client_id">
            Create Provider
          </Button>
        </div>
        <p v-if="createError" class="text-sm text-destructive">{{ createError }}</p>
      </CardContent>
    </Card>

    <!-- Provider List (DataTable) -->
    <DataTable 
      :columns="columns as any" 
      :data="providers" 
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
                placeholder="Search providers (e.g. name:Google protocol:oidc)"
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
                    <span class="font-medium mr-2">name:</span> Search by Name
                  </button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('protocol:')">
                    <span class="font-medium mr-2">protocol:</span> Filter by Protocol
                  </button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors" @mousedown.prevent="appendSearchToken('status:')">
                    <span class="font-medium mr-2">status:</span> Filter by Status
                  </button>
                </div>
                
                <div v-if="currentFilterPrefix === 'status:'">
                  <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">Status</div>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('status:enabled ')">Enabled</button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('status:disabled ')">Disabled</button>
                </div>

                <div v-if="currentFilterPrefix === 'protocol:'">
                  <div class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">Protocol</div>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('protocol:oidc ')">OIDC</button>
                  <button class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors" @mousedown.prevent="appendSearchToken('protocol:saml ')">SAML</button>
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

    <!-- Detail Panel -->
    <Card v-if="detailProvider">
      <CardHeader class="flex-row items-center justify-between space-y-0">
        <CardTitle>{{ detailProvider.name }}</CardTitle>
        <Button variant="outline" size="sm" @click="detailProvider = null">Close</Button>
      </CardHeader>
      <CardContent>
        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">ID</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm break-all">{{ detailProvider.id }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Issuer</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm break-all">{{ detailProvider.config?.issuer || '—' }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Client ID</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm break-all">{{ detailProvider.config?.client_id || '—' }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Scopes</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm">{{ detailProvider.config?.scopes || '—' }}</code>
          </div>
          <div v-if="detailProvider.claim_overrides && Object.keys(detailProvider.claim_overrides).length" class="col-span-2 space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Claim Overrides</span>
            <div v-for="(expr, field) in detailProvider.claim_overrides" :key="field" class="text-sm">
              <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ field }}</code>
              →
              <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ expr }}</code>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, h } from 'vue'
import { onClickOutside } from '@vueuse/core'
import { api } from '@/api/client'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, DropdownMenuCheckboxItem } from '@/components/ui/dropdown-menu'
import { 
  Plus, Trash2, Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown, CheckCircle2, Ban, MoreHorizontal
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

interface Template {
  id: string; name: string; protocol: string; description: string;
  default_config?: Record<string, any>; claim_overrides?: Record<string, string>;
}
interface Provider {
  id: string; name: string; protocol: string; template: string;
  enabled: boolean; auto_register: boolean; config?: Record<string, any>;
  claim_overrides?: Record<string, string>; created_at: string;
}

const providers = ref<Provider[]>([])
const templates = ref<Template[]>([])
const showCreate = ref(false)
const selectedTemplate = ref<Template | null>(null)
const detailProvider = ref<Provider | null>(null)
const createError = ref('')
const selectedRows = ref({})
const globalSearch = ref('')
const isSearchOpen = ref(false)

const searchInputRef = ref<any>(null)
const searchContainerRef = ref<HTMLElement | null>(null)

onClickOutside(searchContainerRef, () => {
  isSearchOpen.value = false
})

let activeTable: any = null

const createForm = ref({
  name: '', issuer: '', client_id: '', client_secret: '', scopes: 'openid email profile', auto_register: true
})

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
   if (lastPart.startsWith('name:')) return 'name:'
   if (lastPart.startsWith('protocol:')) return 'protocol:'
   if (lastPart.startsWith('status:')) return 'status:'
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
      
      if (key === 'name') filters.push({ id: 'name', value })
      else if (key === 'protocol') filters.push({ id: 'protocol', value })
      else if (key === 'status') filters.push({ id: 'status', value })
      else globalText += token + ' '
    } else {
      globalText += token + ' '
    }
  }

  const remainder = globalText.trim()
  if (remainder) {
    if (!filters.find((f: any) => f.id === 'name')) {
       filters.push({ id: 'name', value: remainder })
    }
  }
  
  if (activeTable) {
    activeTable.setColumnFilters(filters)
  }
}

onMounted(async () => {
  await Promise.all([fetchProviders(), fetchTemplates()])
})

async function fetchProviders() {
  try {
    const data = await api.get<any>('/v1/providers')
    providers.value = data.providers || []
  } catch { /* ignore */ }
}

async function fetchTemplates() {
  try {
    const data = await api.get<any>('/v1/providers/templates')
    templates.value = data.templates || []
  } catch { /* ignore */ }
}

function pickTemplate(t: Template) {
  selectedTemplate.value = t
  createForm.value.name = ''
  createForm.value.issuer = t.default_config?.issuer || ''
  createForm.value.scopes = (t.default_config?.scopes as string) || 'openid email profile'
  createForm.value.client_id = ''
  createForm.value.client_secret = ''
  createError.value = ''
}

async function createProvider() {
  createError.value = ''
  try {
    await api.post('/v1/providers', {
      name: createForm.value.name,
      protocol: selectedTemplate.value?.protocol || 'oidc',
      template: selectedTemplate.value?.id || 'custom',
      config: {
        issuer: createForm.value.issuer,
        client_id: createForm.value.client_id,
        client_secret: createForm.value.client_secret,
        scopes: createForm.value.scopes,
      },
      auto_register: createForm.value.auto_register,
    })
    showCreate.value = false
    selectedTemplate.value = null
    await fetchProviders()
  } catch (e: any) {
    createError.value = e.message || 'Create failed'
  }
}

async function toggleEnabled(p: Provider) {
  await api.patch(`/v1/providers/${p.id}`, { enabled: !p.enabled })
  await fetchProviders()
}

async function deleteProvider(p: Provider) {
  if (!confirm(`Delete provider "${p.name}"?`)) return
  await api.delete(`/v1/providers/${p.id}`)
  if (detailProvider.value?.id === p.id) detailProvider.value = null
  await fetchProviders()
}

async function toggleDetail(p: Provider) {
  if (detailProvider.value?.id === p.id) {
    detailProvider.value = null
    return
  }
  try {
    detailProvider.value = await api.get<Provider>(`/v1/providers/${p.id}`)
  } catch {
    detailProvider.value = p
  }
}

function templateIcon(id: string): string {
  const icons: Record<string, string> = {
    google: '🔵', entraid: '🟦', gitlab: '🦊', apple: '🍎', custom: '⚙'
  }
  return icons[id] || '🔗'
}

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }

const columnHelper = createColumnHelper<Provider>()

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
  columnHelper.accessor('name', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Name', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => h('div', { 
      class: 'flex items-center space-x-2 font-medium cursor-pointer',
      onClick: () => toggleDetail(info.row.original)
    }, [
      h('span', { class: 'mr-1' }, templateIcon(info.row.original.template)),
      h('span', {}, info.getValue())
    ]),
  }),
  columnHelper.accessor('protocol', {
    header: 'Protocol',
    cell: info => h(Badge, { variant: 'outline', class: 'text-xs uppercase' }, () => info.getValue()),
    filterFn: (row, id, filterValue) => {
      return String(row.getValue(id)).toLowerCase().includes(String(filterValue).toLowerCase())
    },
  }),
  columnHelper.accessor('template', {
    header: 'Template',
    cell: info => h('span', { class: 'text-sm' }, info.getValue()),
  }),
  columnHelper.accessor(row => row.enabled ? 'enabled' : 'disabled', {
    id: 'status',
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Status', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: ({ row }) => {
      const enabled = row.original.enabled
      const colorClass = enabled
        ? 'text-green-700 bg-green-100 border-green-200'
        : 'text-red-700 bg-red-100 border-red-200'
      const Icon = enabled ? CheckCircle2 : Ban
      return h(Badge, { variant: 'outline', class: `font-normal flex items-center space-x-1 ${colorClass} capitalize whitespace-nowrap` }, () => [
        h(Icon, { class: 'w-3 h-3 mr-1 shrink-0' }),
        h('span', enabled ? 'enabled' : 'disabled')
      ])
    },
  }),
  columnHelper.accessor(row => row.auto_register ? 'yes' : 'no', {
    id: 'auto_register',
    header: 'Auto Register',
    cell: ({ row }) => h(Badge, { 
      variant: 'outline', 
      class: `text-xs font-normal ${row.original.auto_register ? 'text-green-700 bg-green-100 border-green-200' : 'text-muted-foreground bg-muted border-border'}` 
    }, () => row.original.auto_register ? 'yes' : 'no'),
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
            onClick: () => toggleDetail(row.original) 
          }, () => 'View Details'),
          h(DropdownMenuItem, { 
            class: 'cursor-pointer', 
            onClick: () => toggleEnabled(row.original) 
          }, () => row.original.enabled ? 'Disable' : 'Enable'),
          h(DropdownMenuItem, { 
            class: 'text-destructive font-medium cursor-pointer', 
            onClick: () => deleteProvider(row.original) 
          }, () => 'Delete Provider'),
        ])
      ])
    ]),
    meta: { class: 'w-16 text-right' }
  })
]
</script>
