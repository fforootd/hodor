<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Background Jobs</h1>
      <p class="text-muted-foreground text-sm">
        Effective cleanup schedules, retention windows, and recent runtime status.
      </p>
    </div>

    <div v-if="errorMessage" class="rounded-md border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      {{ errorMessage }}
    </div>

    <div v-if="loading" class="rounded-md border bg-card px-4 py-6 text-sm text-muted-foreground">
      Loading job state…
    </div>

    <DataTable
      v-else
      v-model:row-selection="selectedRows"
      :columns="columns as any"
      :data="jobs"
    >
      <template #toolbar="{ table }">
        <div class="mb-4 flex w-full items-center justify-between gap-4">
          <div class="relative w-full max-w-lg">
            <Search class="absolute left-2.5 top-2.5 z-10 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search jobs..."
              class="relative z-0 w-full bg-background pl-9"
              :model-value="globalSearch"
              @update:model-value="val => { globalSearch = String(val); table.setGlobalFilter(String(val)) }"
            />
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
                {{ column.id.replaceAll('_', ' ') }}
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
import { h, onMounted, ref } from 'vue'
import { createColumnHelper } from '@tanstack/vue-table'
import { ArrowDown, ArrowUp, ArrowUpDown, CheckCircle2, ChevronDown, Clock3, Search, TriangleAlert } from 'lucide-vue-next'
import { jobApi, type JobStatus } from '@/api/resources'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'

const jobs = ref<JobStatus[]>([])
const loading = ref(true)
const errorMessage = ref('')
const selectedRows = ref({})
const globalSearch = ref('')

function getSortIcon(column: any) {
  const isSorted = column.getIsSorted()
  if (isSorted === 'asc') return ArrowUp
  if (isSorted === 'desc') return ArrowDown
  return ArrowUpDown
}

function formatDate(value?: string | null) {
  if (!value) return '—'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString()
}

function statusClass(status: string) {
  switch (status) {
    case 'ok':
    case 'scheduled':
      return 'border-emerald-200 bg-emerald-50 text-emerald-700'
    case 'running':
      return 'border-sky-200 bg-sky-50 text-sky-700'
    case 'error':
      return 'border-amber-200 bg-amber-50 text-amber-700'
    default:
      return 'border-muted bg-muted/40 text-muted-foreground'
  }
}

function strategyClass(strategy: string) {
  return strategy === 'partition_drop'
    ? 'border-violet-200 bg-violet-50 text-violet-700'
    : 'border-slate-200 bg-slate-50 text-slate-700'
}

async function loadJobs() {
  loading.value = true
  errorMessage.value = ''
  try {
    jobs.value = await jobApi.list()
  } catch (error) {
    console.error('Failed to load jobs', error)
    errorMessage.value = 'Unable to load runtime job state.'
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  loadJobs()
})

const columnHelper = createColumnHelper<JobStatus>()

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
  columnHelper.accessor('display_name', {
    id: 'job',
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
    }, () => ['Job', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: ({ row }) => h('div', { class: 'space-y-1' }, [
      h('div', { class: 'font-medium' }, row.original.display_name),
      h('div', { class: 'font-mono text-xs text-muted-foreground' }, row.original.name),
      h('div', { class: 'max-w-[320px] text-xs text-muted-foreground' }, row.original.description),
    ]),
  }),
  columnHelper.accessor('strategy', {
    header: ({ column }) => h(Button, {
      variant: 'ghost',
      class: '-ml-4',
      onClick: () => column.toggleSorting(column.getIsSorted() === 'asc'),
    }, () => ['Strategy', h(getSortIcon(column), { class: 'ml-2 h-4 w-4' })]),
    cell: info => h(Badge, {
      variant: 'outline',
      class: `font-normal capitalize ${strategyClass(info.getValue())}`,
    }, () => info.getValue().replace('_', ' ')),
  }),
  columnHelper.accessor(row => row.targets.join(', '), {
    id: 'targets',
    header: 'Targets',
    cell: info => h('span', { class: 'text-sm text-muted-foreground' }, info.getValue()),
  }),
  columnHelper.accessor('retention', {
    header: 'Retention',
    cell: info => h('span', { class: 'font-mono text-sm' }, info.getValue() || '—'),
  }),
  columnHelper.accessor('schedule', {
    header: 'Schedule',
    cell: ({ row }) => h('div', { class: 'space-y-1' }, [
      h('div', { class: 'font-mono text-sm' }, row.original.schedule),
      h('div', { class: 'text-xs text-muted-foreground' }, `Every ${row.original.cadence || '—'}`),
    ]),
  }),
  columnHelper.accessor('status', {
    header: 'Status',
    cell: ({ row }) => {
      const status = row.original.status
      const icon = status === 'error' ? TriangleAlert : status === 'running' ? Clock3 : CheckCircle2
      return h(Badge, {
        variant: 'outline',
        class: `font-normal capitalize whitespace-nowrap ${statusClass(status)}`,
      }, () => [
        h(icon, { class: 'mr-1 h-3 w-3 shrink-0' }),
        h('span', status),
      ])
    },
  }),
  columnHelper.accessor('last_removed_count', {
    header: 'Last Removed',
    cell: info => h('span', { class: 'font-mono text-sm' }, String(info.getValue())),
  }),
  columnHelper.accessor('last_run_at', {
    header: 'Last Run',
    cell: info => h('span', { class: 'text-sm text-muted-foreground' }, formatDate(info.getValue())),
  }),
  columnHelper.accessor('next_run_at', {
    header: 'Next Run',
    cell: info => h('span', { class: 'text-sm text-muted-foreground' }, formatDate(info.getValue())),
  }),
]
</script>
