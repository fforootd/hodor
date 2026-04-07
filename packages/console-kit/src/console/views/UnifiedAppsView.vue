<template>
  <div class="space-y-6">
    <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Applications</h1>
        <p class="text-sm text-muted-foreground">
          Manage applications and client settings{{ totalCount > 0 ? ` (${totalCount} total)` : '' }}
        </p>
      </div>
      <div class="flex flex-col gap-2 sm:flex-row">
        <Button variant="outline" as-child>
          <router-link to="/api-protocols">
            <Link2 class="mr-2 size-4" />
            View API &amp; Protocols
          </router-link>
        </Button>
        <Button @click="showCreate = true">
            <Plus class="mr-2 size-4" />
            New Application
        </Button>
      </div>
    </div>

    <!-- Tabs -->
    <Tabs :default-value="activeTab" @update:model-value="(val: any) => (activeTab = String(val))">
      <TabsList>
        <TabsTrigger value="all">
          All
          <Badge v-if="totalCount > 0" variant="secondary" class="ml-1.5 text-xs px-1.5 py-0">{{
            totalCount
          }}</Badge>
        </TabsTrigger>
        <TabsTrigger value="app">
          <AppWindow class="mr-1.5 size-3.5" />
          OIDC Clients
          <Badge v-if="typeCounts.app" variant="secondary" class="ml-1.5 text-xs px-1.5 py-0">{{
            typeCounts.app
          }}</Badge>
        </TabsTrigger>
      </TabsList>
    </Tabs>

    <!-- Empty state -->
    <Empty v-if="allApps.length === 0 && !loading" class="border border-dashed">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <AppWindow />
        </EmptyMedia>
        <EmptyTitle>No Applications Yet</EmptyTitle>
        <EmptyDescription>
          Create your first application to connect Zitadel to an OIDC or SAML integration.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button @click="showCreate = true">
          <Plus class="mr-2 size-4" />
          New Application
        </Button>
      </EmptyContent>
    </Empty>

    <DataTable
      v-if="allApps.length > 0"
      v-model:row-selection="selectedRows"
      :columns="columns as any"
      :data="filteredApps"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <div class="w-full max-w-lg relative">
            <div class="relative w-full">
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
              <Input
                placeholder="Search applications…"
                class="pl-9 bg-background w-full relative z-0"
                :model-value="globalSearch"
                @update:model-value="(val) => applySearchQuery(String(val), table)"
              />
            </div>
          </div>

          <DropdownMenu>
            <DropdownMenuTrigger as-child>
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

    <AppCreateView
      :open="showCreate"
      @update:open="showCreate = $event"
      @created="loadApps"
    />
  </div>
</template>

<script setup lang="ts">
  import { ref, onMounted, computed, h, watch } from 'vue'
  import { RouterLink } from 'vue-router'
  import { appApi, type App } from '@/api/resources'
  import { useOrgContext } from '@/console/composables/useOrgContext'
  import DataTable from '@/components/ui/data-table/DataTable.vue'
  import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
  import { Input } from '@/components/ui/input'
  import { Checkbox } from '@/components/ui/checkbox'
  import { Badge } from '@/components/ui/badge'
  import { Button } from '@/components/ui/button'
  import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuTrigger,
    DropdownMenuCheckboxItem,
  } from '@/components/ui/dropdown-menu'
  import {
    Empty,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
    EmptyDescription,
    EmptyContent,
  } from '@/components/ui/empty'
  import {
    Plus,
    Search,
    ChevronDown,
    ArrowUpDown,
    ArrowUp,
    ArrowDown,
    CheckCircle2,
    Ban,
    AppWindow,
    Link2,
  } from 'lucide-vue-next'
  import { createColumnHelper } from '@tanstack/vue-table'
  import AppCreateView from '@/console/views/AppCreateView.vue'

  const showCreate = ref(false)
  const activeTab = ref('all')
  const allApps = ref<App[]>([])
  const selectedRows = ref({})
  const globalSearch = ref('')
  const loading = ref(true)
  // Computed
  const totalCount = computed(() => allApps.value.length)

  const typeCounts = computed(() => {
    const counts: Record<string, number> = {}
    for (const item of allApps.value) {
      const t = item.schema_type || 'app'
      counts[t] = (counts[t] || 0) + 1
    }
    return counts
  })

  const filteredApps = computed(() => {
    if (activeTab.value === 'all') return allApps.value
    return allApps.value.filter((a) => (a.schema_type || 'app') === activeTab.value)
  })

  // Search
  let activeTable: any = null

  function applySearchQuery(query: string, table: any) {
    if (table) activeTable = table
    globalSearch.value = query
    const filters: { id: string; value: string }[] = []
    if (query.trim()) {
      filters.push({ id: 'client_id', value: query.trim() })
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

  function formatUris(app: App): string {
    const uris = app.redirect_uris || []
    if (uris.length === 0) return '—'
    if (uris.length === 1) return uris[0]
    return `${uris[0]} +${uris.length - 1} more`
  }

  const { currentOrgId } = useOrgContext()

  async function loadApps() {
    loading.value = true
    try {
      allApps.value = await appApi.list({ org_id: currentOrgId.value || undefined })
    } catch {
      /* ignore */
    } finally {
      loading.value = false
    }
  }

  onMounted(() => loadApps())

  watch(currentOrgId, () => loadApps())

  watch(activeTab, () => {
    if (activeTable) {
      activeTable.setColumnFilters([])
      globalSearch.value = ''
    }
  })

  const columnHelper = createColumnHelper<App>()

  const columns = computed(() => [
    columnHelper.display({
      id: 'select',
      header: ({ table }) =>
        h(Checkbox, {
          checked:
            table.getIsAllPageRowsSelected() ||
            (table.getIsSomePageRowsSelected() && ('indeterminate' as any)),
          'onUpdate:checked': (val: boolean) => table.toggleAllPageRowsSelected(!!val),
          ariaLabel: 'Select all',
        }),
      cell: ({ row }) =>
        h(Checkbox, {
          checked: row.getIsSelected(),
          'onUpdate:checked': (val: boolean) => row.toggleSelected(!!val),
          ariaLabel: 'Select row',
        }),
      meta: { class: 'w-12 border-r-0' },
      enableSorting: false,
      enableHiding: false,
    }),
    columnHelper.accessor('client_id', {
      header: ({ column }) =>
        h(
          Button,
          {
            variant: 'ghost',
            class: '-ml-4',
            onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
          },
          () => ['Client ID', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })],
        ),
      cell: (info) =>
        h(
          RouterLink,
          {
            to: `/applications/${info.row.original.id}`,
            class: 'font-mono text-sm text-primary hover:underline',
          },
          () => info.getValue(),
        ),
    }),
    columnHelper.accessor('name', {
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
      cell: (info) => h('span', { class: 'text-sm' }, info.getValue() || '—'),
    }),
    columnHelper.accessor((row) => row.schema_type || 'app', {
      id: 'schema_type',
      header: 'Schema',
      cell: (info) =>
        h(
          Badge,
          { variant: 'outline', class: 'text-xs uppercase' },
          () => info.getValue() === 'app' ? 'OIDC Client' : info.getValue(),
        ),
    }),
    columnHelper.display({
      id: 'redirect_uris',
      header: 'Redirect URIs',
      cell: ({ row }) =>
        h(
          'span',
          { class: 'text-sm text-muted-foreground truncate max-w-[300px] inline-block' },
          formatUris(row.original),
        ),
    }),
    columnHelper.accessor('state', {
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
      cell: (info) => {
        const state = info.getValue() as string
        let colorClass = 'text-green-700 bg-green-100 border-green-200'
        let Icon = CheckCircle2
        if (state === 'deactivated' || state === 'locked') {
          colorClass = 'text-red-700 bg-red-100 border-red-200'
          Icon = Ban
        }
        return h(
          Badge,
          {
            variant: 'outline',
            class: `font-normal flex items-center space-x-1 ${colorClass} capitalize whitespace-nowrap`,
          },
          () => [h(Icon, { class: 'w-3 h-3 mr-1 shrink-0' }), h('span', state)],
        )
      },
    }),
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
        const d = new Date(info.getValue())
        return h('div', { class: 'flex flex-col text-sm whitespace-nowrap' }, [
          h('span', d.toLocaleDateString()),
          h(
            'span',
            { class: 'text-xs text-muted-foreground' },
            d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
          ),
        ])
      },
    }),
  ])
</script>
