<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">{{ title }}</h1>
        <p class="text-sm text-muted-foreground">
          {{ loading ? 'Loading…' : `${items.length} ${title.toLowerCase()} total` }}
        </p>
      </div>
      <slot name="header-actions">
        <Button as-child>
          <router-link :to="createRoute">
            <Plus class="mr-2 size-4" />
            New {{ singularTitle }}
          </router-link>
        </Button>
      </slot>
    </div>

    <slot name="after-header" />

    <!-- Empty state -->
    <Empty v-if="!loading && items.length === 0" class="border border-dashed">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <slot name="empty-icon">
            <FolderOpen />
          </slot>
        </EmptyMedia>
        <EmptyTitle>{{ searchQuery ? 'No Results' : `No ${title} Yet` }}</EmptyTitle>
        <EmptyDescription>
          {{ searchQuery ? `No ${title.toLowerCase()} match your search.` : `Create your first ${singularTitle.toLowerCase()} to get started.` }}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent v-if="!searchQuery">
        <Button as-child>
          <router-link :to="createRoute">
            <Plus class="mr-2 size-4" />
            New {{ singularTitle }}
          </router-link>
        </Button>
      </EmptyContent>
    </Empty>

    <!-- Data Table -->
    <DataTable
      v-if="items.length > 0"
      v-model:row-selection="selectedRows"
      :columns="columns as any"
      :data="filteredItems"
    >
      <template #toolbar="{ table }">
        <div class="flex items-center justify-between w-full mb-4">
          <div class="w-full max-w-lg relative">
            <div class="relative w-full">
              <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
              <Input
                :placeholder="`Search ${title.toLowerCase()}…`"
                class="pl-9 bg-background w-full relative z-0"
                :model-value="globalSearch"
                @update:model-value="val => applySearchQuery(String(val), table)"
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

    <!-- Loading spinner -->
    <div v-if="loading && items.length === 0" class="flex h-48 items-center justify-center">
      <Spinner class="size-6 text-muted-foreground" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, type Ref, type Component } from 'vue'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuTrigger, DropdownMenuCheckboxItem,
} from '@/components/ui/dropdown-menu'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import { Plus, Search, ChevronDown, FolderOpen } from 'lucide-vue-next'

const props = defineProps<{
  title: string
  singularTitle: string
  createRoute: string
  items: any[]
  loading: boolean
  columns: any[]
  searchFields?: string[]
}>()

const searchQuery = defineModel<string>('searchQuery', { default: '' })

const selectedRows = ref({})
const globalSearch = ref('')

const filteredItems = computed(() => {
  if (!searchQuery.value.trim()) return props.items
  const q = searchQuery.value.toLowerCase()
  const fields = props.searchFields || ['name', 'id']
  return props.items.filter(item =>
    fields.some(field => {
      const val = item[field]
      return typeof val === 'string' && val.toLowerCase().includes(q)
    })
  )
})

let activeTable: any = null

function applySearchQuery(query: string, table: any) {
  if (table) activeTable = table
  globalSearch.value = query
  searchQuery.value = query
  const filters: { id: string; value: string }[] = []
  if (query.trim()) {
    // Apply filter to the first search field (usually 'name')
    const primaryField = props.searchFields?.[0] || 'name'
    filters.push({ id: primaryField, value: query.trim() })
  }
  if (activeTable) {
    activeTable.setColumnFilters(filters)
  }
}
</script>
