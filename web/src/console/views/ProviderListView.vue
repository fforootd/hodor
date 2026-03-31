<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Providers</h1>
        <p class="text-sm text-muted-foreground">{{ providers.length }} providers configured</p>
      </div>
      <Button as-child>
        <router-link to="/providers/new">
          <Plus class="mr-2 size-4" />
          New Provider
        </router-link>
      </Button>
    </div>

    <Empty v-if="providers.length === 0">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Link2 />
        </EmptyMedia>
        <EmptyTitle>No Providers Yet</EmptyTitle>
        <EmptyDescription>
          Create your first configured identity provider to enable federated sign-in.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button as-child>
          <router-link to="/providers/new">
            <Plus class="mr-2 size-4" />
            New Provider
          </router-link>
        </Button>
      </EmptyContent>
    </Empty>

    <DataTable
      v-if="providers.length > 0"
      :columns="columns as any"
      :data="providers"
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <div class="w-full max-w-lg relative" ref="searchContainerRef">
            <div class="relative w-full">
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
              <Input
                ref="searchInputRef"
                placeholder="Search providers (e.g. name:Google protocol:oidc)"
                class="pl-9 bg-background w-full relative z-0"
                :model-value="globalSearch"
                @update:model-value="(val) => applySearchQuery(String(val), table)"
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
                  <div
                    class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider"
                  >
                    Filters
                  </div>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors"
                    @mousedown.prevent="appendSearchToken('name:')"
                  >
                    <span class="font-medium mr-2">name:</span> Search by instance name
                  </button>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors"
                    @mousedown.prevent="appendSearchToken('template:')"
                  >
                    <span class="font-medium mr-2">template:</span> Filter by template origin
                  </button>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors"
                    @mousedown.prevent="appendSearchToken('protocol:')"
                  >
                    <span class="font-medium mr-2">protocol:</span> Filter by protocol
                  </button>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer flex items-center transition-colors"
                    @mousedown.prevent="appendSearchToken('status:')"
                  >
                    <span class="font-medium mr-2">status:</span> Filter by status
                  </button>
                </div>

                <div v-if="currentFilterPrefix === 'status:'">
                  <div
                    class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider"
                  >
                    Status
                  </div>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors"
                    @mousedown.prevent="appendSearchToken('status:enabled ')"
                  >
                    Enabled
                  </button>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors"
                    @mousedown.prevent="appendSearchToken('status:disabled ')"
                  >
                    Disabled
                  </button>
                </div>

                <div v-if="currentFilterPrefix === 'protocol:'">
                  <div
                    class="px-2 py-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider"
                  >
                    Protocol
                  </div>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors"
                    @mousedown.prevent="appendSearchToken('protocol:oidc ')"
                  >
                    OIDC
                  </button>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors"
                    @mousedown.prevent="appendSearchToken('protocol:oauth2 ')"
                  >
                    OAuth2
                  </button>
                  <button
                    class="w-full text-left px-2 py-1.5 text-sm hover:bg-muted cursor-pointer transition-colors"
                    @mousedown.prevent="appendSearchToken('protocol:saml ')"
                  >
                    SAML
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

    <Dialog :open="providerPendingDelete !== null" @update:open="handleDeleteDialogOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Provider</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete
            <strong>{{ providerPendingDelete ? providerDisplayName(providerPendingDelete) : 'this provider' }}</strong
            >? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="providerPendingDelete = null">Cancel</Button>
          <Button variant="destructive" :disabled="deleting" @click="confirmDeleteProvider">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
  import { ref, onMounted, computed, h } from 'vue'
  import { RouterLink } from 'vue-router'
  import { onClickOutside } from '@vueuse/core'
  import { api } from '@/api/client'
  import type { ProviderRecord } from '@/console/utils/provider-utils'
  import {
    humanizeProviderLinking,
    providerDisplayName,
    providerIcon,
    providerTemplateLabel,
  } from '@/console/utils/provider-utils'
  import DataTable from '@/components/ui/data-table/DataTable.vue'
  import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
  import { Input } from '@/components/ui/input'
  import { Checkbox } from '@/components/ui/checkbox'
  import { Badge } from '@/components/ui/badge'
  import { Button } from '@/components/ui/button'
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog'
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
    DropdownMenuCheckboxItem,
  } from '@/components/ui/dropdown-menu'
  import { notifyMutationError, notifyMutationSuccess, notifySuccess } from '@/lib/notify'
  import {
    Plus,
    Search,
    ChevronDown,
    ArrowUpDown,
    ArrowUp,
    ArrowDown,
    CheckCircle2,
    Ban,
    MoreHorizontal,
    Link2,
  } from 'lucide-vue-next'
  import {
    Empty,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
    EmptyDescription,
    EmptyContent,
  } from '@/components/ui/empty'
  import { createColumnHelper } from '@tanstack/vue-table'

  const providers = ref<ProviderRecord[]>([])
  const providerPendingDelete = ref<ProviderRecord | null>(null)
  const selectedRows = ref({})
  const globalSearch = ref('')
  const isSearchOpen = ref(false)
  const deleting = ref(false)

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
    if (lastPart.startsWith('name:')) return 'name:'
    if (lastPart.startsWith('template:')) return 'template:'
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
    const filters: { id: string; value: string }[] = []

    const tokens = query.match(/(?:[^\s"]+|"[^"]*")+/g) || []
    let globalText = ''

    for (const token of tokens) {
      if (token.includes(':') && !token.startsWith('::')) {
        const parts = token.split(':')
        const key = parts[0].toLowerCase()
        const value = parts
          .slice(1)
          .join(':')
          .replace(/(^"|"$)/g, '')

        if (key === 'name') filters.push({ id: 'display_name', value })
        else if (key === 'template') filters.push({ id: 'template', value })
        else if (key === 'protocol') filters.push({ id: 'protocol', value })
        else if (key === 'status') filters.push({ id: 'status', value })
        else globalText += token + ' '
      } else {
        globalText += token + ' '
      }
    }

    const remainder = globalText.trim()
    if (remainder && !filters.find((filter) => filter.id === 'display_name')) {
      filters.push({ id: 'display_name', value: remainder })
    }

    if (activeTable) {
      activeTable.setColumnFilters(filters)
    }
  }

  async function loadProviders() {
    try {
      const data = await api.get<any>('/v1/providers')
      providers.value = data.providers || []
    } catch {
      /* ignore */
    }
  }

  onMounted(loadProviders)

  async function toggleEnabled(provider: ProviderRecord) {
    try {
      await api.patch(`/v1/providers/${provider.id}`, { enabled: !provider.enabled })
      notifySuccess(provider.enabled ? 'Provider disabled' : 'Provider enabled')
      await loadProviders()
    } catch (err: any) {
      notifyMutationError('Provider', 'toggle', err)
    }
  }

  function requestDeleteProvider(provider: ProviderRecord) {
    providerPendingDelete.value = provider
  }

  function handleDeleteDialogOpen(next: boolean) {
    if (!next) providerPendingDelete.value = null
  }

  async function confirmDeleteProvider() {
    if (!providerPendingDelete.value) return
    deleting.value = true
    try {
      await api.delete(`/v1/providers/${providerPendingDelete.value.id}`)
      notifyMutationSuccess('Provider', 'delete')
      providerPendingDelete.value = null
      await loadProviders()
    } catch (err: any) {
      notifyMutationError('Provider', 'delete', err)
    } finally {
      deleting.value = false
    }
  }

  const columnHelper = createColumnHelper<ProviderRecord>()

  const columns = [
    columnHelper.display({
      id: 'select',
      header: ({ table }) =>
        h(Checkbox, {
          checked:
            table.getIsAllPageRowsSelected() ||
            (table.getIsSomePageRowsSelected() && ('indeterminate' as any)),
          'onUpdate:checked': (value: boolean) => table.toggleAllPageRowsSelected(!!value),
          ariaLabel: 'Select all',
        }),
      cell: ({ row }) =>
        h(Checkbox, {
          checked: row.getIsSelected(),
          'onUpdate:checked': (value: boolean) => row.toggleSelected(!!value),
          ariaLabel: 'Select row',
        }),
      meta: { class: 'w-12 border-r-0' },
      enableSorting: false,
      enableHiding: false,
    }),
    columnHelper.accessor((row) => providerDisplayName(row), {
      id: 'display_name',
      header: ({ column }) =>
        h(
          Button,
          {
            variant: 'ghost',
            class: '-ml-4',
            onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
          },
          () => ['Name', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })],
        ),
      cell: (info) =>
        h(
          RouterLink,
          {
            to: `/providers/${info.row.original.id}`,
            class: 'flex items-center gap-2 font-medium hover:underline',
          },
          () => [
            h(
              'span',
              { class: 'text-base' },
              providerIcon(providerTemplateLabel(info.row.original)),
            ),
            h('span', {}, info.getValue()),
          ],
        ),
      filterFn: (row, id, filterValue) =>
        String(row.getValue(id)).toLowerCase().includes(String(filterValue).toLowerCase()),
    }),
    columnHelper.accessor('protocol', {
      header: 'Protocol',
      cell: (info) =>
        h(Badge, { variant: 'outline', class: 'text-xs uppercase' }, () => info.getValue()),
      filterFn: (row, id, filterValue) =>
        String(row.getValue(id)).toLowerCase().includes(String(filterValue).toLowerCase()),
    }),
    columnHelper.accessor((row) => providerTemplateLabel(row), {
      id: 'template',
      header: 'Template',
      cell: (info) => h('span', { class: 'text-sm text-muted-foreground' }, info.getValue()),
      filterFn: (row, id, filterValue) =>
        String(row.getValue(id)).toLowerCase().includes(String(filterValue).toLowerCase()),
    }),
    columnHelper.accessor((row) => (row.enabled ? 'enabled' : 'disabled'), {
      id: 'status',
      header: ({ column }) =>
        h(
          Button,
          {
            variant: 'ghost',
            class: '-ml-4',
            onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
          },
          () => ['Status', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })],
        ),
      cell: ({ row }) => {
        const enabled = row.original.enabled
        const colorClass = enabled
          ? 'text-green-700 bg-green-100 border-green-200'
          : 'text-red-700 bg-red-100 border-red-200'
        const Icon = enabled ? CheckCircle2 : Ban
        return h(
          Badge,
          {
            variant: 'outline',
            class: `font-normal flex items-center space-x-1 ${colorClass} capitalize whitespace-nowrap`,
          },
          () => [
            h(Icon, { class: 'w-3 h-3 mr-1 shrink-0' }),
            h('span', enabled ? 'enabled' : 'disabled'),
          ],
        )
      },
      filterFn: (row, id, filterValue) =>
        String(row.getValue(id)).toLowerCase().includes(String(filterValue).toLowerCase()),
    }),
    columnHelper.accessor(
      (row) => humanizeProviderLinking(row.linking?.mode, row.linking?.match_by),
      {
        id: 'linking',
        header: 'Linking',
        cell: (info) =>
          h(Badge, { variant: 'outline', class: 'text-xs font-normal' }, () => info.getValue()),
      },
    ),
    columnHelper.accessor('created_at', {
      header: ({ column }) =>
        h(
          Button,
          {
            variant: 'ghost',
            class: '-ml-4',
            onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
          },
          () => ['Created', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })],
        ),
      cell: (info) => {
        if (!info.getValue()) return h('span', '—')
        const date = new Date(String(info.getValue()))
        return h('div', { class: 'flex flex-col text-sm whitespace-nowrap' }, [
          h('span', date.toLocaleDateString()),
          h(
            'span',
            { class: 'text-xs text-muted-foreground' },
            date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
          ),
        ])
      },
    }),
    columnHelper.display({
      id: 'actions',
      header: () => null,
      cell: ({ row }) =>
        h('div', { class: 'flex items-center justify-end' }, [
          h(DropdownMenu, {}, () => [
            h(DropdownMenuTrigger, { asChild: true }, () =>
              h(
                'button',
                {
                  class:
                    'text-muted-foreground hover:text-foreground hover:bg-muted p-1.5 rounded-md transition-colors',
                },
                [h(MoreHorizontal, { class: 'w-4 h-4' })],
              ),
            ),
            h(DropdownMenuContent, { align: 'end' }, () => [
              h(DropdownMenuItem, { asChild: true }, () =>
                h(
                  RouterLink,
                  {
                    to: `/providers/${row.original.id}`,
                  },
                  () => 'Open',
                ),
              ),
              h(
                DropdownMenuItem,
                {
                  class: 'cursor-pointer',
                  onClick: () => toggleEnabled(row.original),
                },
                () => (row.original.enabled ? 'Disable' : 'Enable'),
              ),
              h(
                DropdownMenuItem,
                {
                  class: 'text-destructive font-medium cursor-pointer',
                  onClick: () => requestDeleteProvider(row.original),
                },
                () => 'Delete',
              ),
            ]),
          ]),
        ]),
      meta: { class: 'w-16 text-right' },
    }),
  ]
</script>
