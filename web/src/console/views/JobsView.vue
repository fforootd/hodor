<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Background Jobs</h1>
      <p class="text-muted-foreground text-sm">Registered recurring jobs and their status.</p>
    </div>

    <DataTable 
      v-model:row-selection="selectedRows" 
      :columns="columns as any" 
      :data="jobs"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <div class="w-full max-w-lg relative">
            <div class="relative w-full">
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
              <Input
                placeholder="Search jobs..."
                class="pl-9 bg-background w-full relative z-0"
                :model-value="globalSearch"
                @update:model-value="val => { globalSearch = String(val); table.setGlobalFilter(String(val)) }"
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
  </div>
</template>

<script setup lang="ts">
import { ref, h } from 'vue'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger, DropdownMenuCheckboxItem } from '@/components/ui/dropdown-menu'
import { Search, ChevronDown, ArrowUpDown, ArrowUp, ArrowDown, CheckCircle2 } from 'lucide-vue-next'
import { createColumnHelper } from '@tanstack/vue-table'

interface Job {
  name: string
  status: string
}

const jobList: Job[] = [
  { name: 'lake_writer', status: 'scheduled' },
  { name: 'session_gc', status: 'scheduled' },
  { name: 'event_gc', status: 'scheduled' },
]

const jobs = ref<Job[]>(jobList)
const selectedRows = ref({})
const globalSearch = ref('')

function getSortIcon(column: any) {
  const isSorted = column.getIsSorted()
  if (isSorted === 'asc') return ArrowUp
  if (isSorted === 'desc') return ArrowDown
  return ArrowUpDown
}

const columnHelper = createColumnHelper<Job>()

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
    }, () => ['Job Name', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => h('span', { class: 'font-medium font-mono text-sm' }, info.getValue()),
  }),
  columnHelper.accessor('status', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc')
    }, () => ['Status', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => {
      return h(Badge, { 
        variant: 'outline', 
        class: 'font-normal flex items-center space-x-1 text-green-700 bg-green-100 border-green-200 capitalize whitespace-nowrap' 
      }, () => [
        h(CheckCircle2, { class: 'w-3 h-3 mr-1 shrink-0' }),
        h('span', info.getValue())
      ])
    },
  }),
]
</script>
