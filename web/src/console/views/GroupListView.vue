<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Groups</h1>
        <p class="text-sm text-muted-foreground">{{ loading ? 'Loading…' : `${items.length} group${items.length !== 1 ? 's' : ''}` }}</p>
      </div>
      <Button as-child>
        <router-link to="/groups/new">
          <Plus class="mr-2 size-4" />
          New Group
        </router-link>
      </Button>
    </div>

    <!-- Empty state -->
    <Empty v-if="!loading && items.length === 0" class="border border-dashed">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <UsersRound />
        </EmptyMedia>
        <EmptyTitle>{{ searchQuery ? 'No Results' : 'No Groups Yet' }}</EmptyTitle>
        <EmptyDescription>
          {{ searchQuery ? 'No groups match your search.' : 'Create your first group to organize user access.' }}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent v-if="!searchQuery">
        <Button as-child>
          <router-link to="/groups/new">
            <Plus class="mr-2 size-4" />
            New Group
          </router-link>
        </Button>
      </EmptyContent>
    </Empty>

    <DataTable
      v-if="items.length > 0"
      :columns="columns as any"
      :data="filteredItems"
      v-model:rowSelection="selectedRows"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <div class="w-full max-w-lg relative">
            <div class="relative w-full">
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
              <Input
                placeholder="Search groups…"
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, h } from 'vue'
import { RouterLink } from 'vue-router'
import { groupApi, type Group } from '@/api/resources'
import { useResourceList } from '@/console/composables/useResourceList'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuTrigger, DropdownMenuCheckboxItem,
} from '@/components/ui/dropdown-menu'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import {
  Plus, Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown,
  CheckCircle2, Ban, UsersRound,
} from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

const { items, loading, searchQuery, filteredItems, fetch: fetchGroups } = useResourceList<Group>({
  fetchFn: () => groupApi.list(),
  resourceName: 'groups',
  searchFields: ['name', 'description', 'id'],
})

const selectedRows = ref({})
const globalSearch = ref('')

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

onMounted(fetchGroups)

const columnHelper = createColumnHelper<Group>()

const columns = computed(() => [
  columnHelper.accessor('name', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Group', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => h('div', { class: 'flex items-center gap-3' }, [
      h('div', { class: 'flex items-center justify-center w-8 h-8 rounded-lg bg-primary text-primary-foreground text-xs font-semibold' },
        (info.getValue() || '?')[0].toUpperCase()
      ),
      h(RouterLink, {
        to: `/groups/${info.row.original.id}`,
        class: 'font-medium text-primary hover:underline'
      }, () => info.getValue()),
    ]),
  }),
  columnHelper.accessor('description', {
    header: 'Description',
    cell: info => h('span', { class: 'text-sm text-muted-foreground truncate max-w-[200px] inline-block' }, info.getValue() || '—'),
  }),
  columnHelper.accessor('member_count', {
    header: 'Members',
    cell: info => h(Badge, { variant: 'secondary' }, () => String(info.getValue() ?? 0)),
  }),
  columnHelper.accessor('state', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['State', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => {
      const state = (info.getValue() as string) || 'active'
      const isActive = state === 'active'
      return h(Badge, {
        variant: 'outline',
        class: `font-normal flex items-center gap-1 capitalize ${isActive ? 'text-green-700 bg-green-100 border-green-200' : 'text-red-700 bg-red-100 border-red-200'}`
      }, () => [
        h(isActive ? CheckCircle2 : Ban, { class: 'w-3 h-3 shrink-0' }),
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
      const d = new Date(info.getValue()!)
      return h('div', { class: 'flex flex-col text-sm whitespace-nowrap' }, [
        h('span', d.toLocaleDateString()),
        h('span', { class: 'text-xs text-muted-foreground' }, d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })),
      ])
    },
  }),
  columnHelper.accessor('id', {
    header: 'ID',
    cell: info => h('span', { class: 'font-mono text-xs text-muted-foreground' }, info.getValue()),
  }),
])
</script>
