<script setup lang="ts" generic="TData, TValue">
import { ref } from 'vue'
import type {
  ColumnDef,
  ColumnFiltersState,
  SortingState,
  VisibilityState,
  ExpandedState,
  Updater
} from '@tanstack/vue-table'
import {
  FlexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  getExpandedRowModel,
  useVueTable,
  type RowData,
} from '@tanstack/vue-table'

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

const props = withDefaults(defineProps<{
  columns: ColumnDef<TData, TValue>[]
  data: TData[]
  initialSorting?: SortingState
}>(), {
  initialSorting: () => [],
})

const emit = defineEmits<{
  (e: 'update:rowSelection', val: any): void
}>()

function valueUpdater<T>(updaterOrValue: Updater<T>, target: { value: T }, emitName?: string) {
  const next = typeof updaterOrValue === 'function' ? (updaterOrValue as any)(target.value) : updaterOrValue
  target.value = next
  if (emitName) emit(emitName as any, next)
}

const sorting = ref<SortingState>(props.initialSorting)
const columnFilters = ref<ColumnFiltersState>([])
// Derive initial visibility from column meta — columns with meta.defaultHidden start hidden
const initialVisibility: VisibilityState = {}
for (const col of props.columns) {
  const meta = (col as any).meta
  if (meta?.defaultHidden) {
    initialVisibility[(col as any).id || (col as any).accessorKey || ''] = false
  }
}
const columnVisibility = ref<VisibilityState>(initialVisibility)
const rowSelection = ref({})
const expanded = ref<ExpandedState>({})

const table = useVueTable({
  get data() { return props.data },
  get columns() { return props.columns },
  getCoreRowModel: getCoreRowModel(),
  getPaginationRowModel: getPaginationRowModel(),
  getSortedRowModel: getSortedRowModel(),
  getFilteredRowModel: getFilteredRowModel(),
  getExpandedRowModel: getExpandedRowModel(),
  getRowCanExpand: () => true,
  onSortingChange: updaterOrValue => valueUpdater(updaterOrValue, sorting),
  onColumnFiltersChange: updaterOrValue => valueUpdater(updaterOrValue, columnFilters),
  onColumnVisibilityChange: updaterOrValue => valueUpdater(updaterOrValue, columnVisibility),
  onRowSelectionChange: updaterOrValue => valueUpdater(updaterOrValue, rowSelection, 'update:rowSelection'),
  onExpandedChange: updaterOrValue => valueUpdater(updaterOrValue, expanded),
  state: {
    get sorting() { return sorting.value },
    get columnFilters() { return columnFilters.value },
    get columnVisibility() { return columnVisibility.value },
    get rowSelection() { return rowSelection.value },
    get expanded() { return expanded.value },
  },
})
</script>

<template>
  <div class="space-y-4">
    <div v-if="$slots.toolbar">
      <slot name="toolbar" :table="table" />
    </div>

    <div class="rounded-md border bg-card shadow-sm overflow-hidden" :data-state="JSON.stringify(table.getState())" style="--table-reactive: 1">
      <Table>
        <TableHeader class="bg-muted/30">
          <TableRow v-for="headerGroup in table.getHeaderGroups()" :key="headerGroup.id">
            <TableHead v-for="header in headerGroup.headers" :key="header.id" :class="(header.column.columnDef.meta as any)?.class">
              <FlexRender
                v-if="!header.isPlaceholder"
                :render="header.column.columnDef.header"
                :props="header.getContext()"
              />
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <template v-if="table.getRowModel().rows?.length">
            <template v-for="row in table.getRowModel().rows" :key="row.id">
              <TableRow :data-state="row.getIsSelected() ? 'selected' : undefined">
                <TableCell v-for="cell in row.getVisibleCells()" :key="cell.id" :class="(cell.column.columnDef.meta as any)?.class">
                  <FlexRender
                    :render="cell.column.columnDef.cell"
                    :props="cell.getContext()"
                  />
                </TableCell>
              </TableRow>
              <TableRow v-if="row.getIsExpanded()">
                <TableCell :colspan="row.getAllCells().length" class="p-4 bg-muted/10 border-b shadow-inner">
                  <slot name="expanded" :row="row" />
                </TableCell>
              </TableRow>
            </template>
          </template>
          <template v-else>
            <TableRow>
              <TableCell :colspan="columns.length" class="h-24 text-center text-muted-foreground">
                No results found.
              </TableCell>
            </TableRow>
          </template>
        </TableBody>
      </Table>
    </div>

    <div v-if="$slots.pagination">
      <slot name="pagination" :table="table" />
    </div>
  </div>
</template>
