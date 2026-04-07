<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">{{ title }}</h1>
        <p class="text-sm text-muted-foreground">
          {{ loading ? 'Loading…' : description || `Manage your ${title.toLowerCase()}` }}
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

    <!-- Filters + Search bar -->
    <div v-if="items.length > 0 || searchQuery" class="flex items-center gap-2 flex-wrap">
      <!-- Filter pills -->
      <template v-if="filters.length">
        <Popover v-for="filter in filters" :key="filter.key">
          <PopoverTrigger as-child>
            <Button variant="outline" size="sm" class="h-8 gap-1.5 text-xs">
              <component v-if="filter.icon" :is="filter.icon" class="size-3.5" />
              {{ filter.label }}
              <Badge v-if="activeFilters[filter.key]" variant="secondary" class="text-[10px] h-4 px-1.5 ml-0.5">
                {{ filter.options.find(o => o.value === activeFilters[filter.key])?.label || activeFilters[filter.key] }}
              </Badge>
              <ChevronDown class="size-3 opacity-50" />
            </Button>
          </PopoverTrigger>
          <PopoverContent class="w-48 p-1" align="start">
            <div class="space-y-0.5">
              <button
                v-for="option in filter.options"
                :key="option.value"
                class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm transition-colors hover:bg-accent"
                :class="activeFilters[filter.key] === option.value ? 'bg-accent font-medium' : ''"
                @click="toggleFilter(filter.key, option.value)"
              >
                <span class="flex-1 text-left">{{ option.label }}</span>
                <span v-if="option.count !== undefined" class="text-xs text-muted-foreground">{{ option.count }}</span>
              </button>
              <button
                v-if="activeFilters[filter.key]"
                class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-accent"
                @click="clearFilter(filter.key)"
              >
                Clear filter
              </button>
            </div>
          </PopoverContent>
        </Popover>
      </template>

      <!-- Search -->
      <div class="relative ml-auto w-full max-w-xs">
        <Search class="absolute left-2.5 top-2 h-4 w-4 text-muted-foreground" />
        <Input
          :placeholder="`Search ${title.toLowerCase()}…`"
          class="h-8 pl-9 text-xs bg-background"
          :model-value="searchQuery"
          @update:model-value="searchQuery = String($event)"
        />
      </div>

      <!-- Result count -->
      <p v-if="filteredItems.length !== items.length" class="text-xs text-muted-foreground whitespace-nowrap">
        {{ filteredItems.length }} of {{ items.length }} {{ title.toLowerCase() }}
      </p>
    </div>

    <!-- Empty state -->
    <Empty v-if="!loading && items.length === 0 && !searchQuery" class="border border-dashed">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <slot name="empty-icon">
            <FolderOpen />
          </slot>
        </EmptyMedia>
        <EmptyTitle>No {{ title }} Yet</EmptyTitle>
        <EmptyDescription>
          Create your first {{ singularTitle.toLowerCase() }} to get started.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <slot name="empty-actions">
          <Button as-child>
            <router-link :to="createRoute">
              <Plus class="mr-2 size-4" />
              New {{ singularTitle }}
            </router-link>
          </Button>
        </slot>
      </EmptyContent>
    </Empty>

    <!-- No results for search/filter -->
    <div v-else-if="!loading && items.length > 0 && filteredItems.length === 0" class="flex flex-col items-center justify-center py-12 text-center">
      <Search class="size-8 text-muted-foreground/50 mb-3" />
      <p class="text-sm font-medium">No results</p>
      <p class="text-xs text-muted-foreground mt-1">No {{ title.toLowerCase() }} match your search or filters.</p>
    </div>

    <!-- Data Table -->
    <DataTable
      v-if="filteredItems.length > 0"
      :columns="columns as any"
      :data="filteredItems"
    >
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
import { ref, reactive, computed } from 'vue'
import DataTable from '@/components/ui/data-table/DataTable.vue'
import DataTablePagination from '@/components/ui/data-table/DataTablePagination.vue'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Badge } from '@/components/ui/badge'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import { Plus, Search, ChevronDown, FolderOpen } from 'lucide-vue-next'

export interface FilterOption {
  value: string
  label: string
  count?: number
}

export interface ListFilter {
  key: string
  label: string
  icon?: any
  options: FilterOption[]
}

const props = withDefaults(defineProps<{
  title: string
  singularTitle: string
  createRoute: string
  description?: string
  items: any[]
  loading: boolean
  columns: any[]
  searchFields?: string[]
  filters?: ListFilter[]
}>(), {
  filters: () => [],
})

const searchQuery = defineModel<string>('searchQuery', { default: '' })
const activeFilters = reactive<Record<string, string>>({})

function toggleFilter(key: string, value: string) {
  if (activeFilters[key] === value) {
    delete activeFilters[key]
  } else {
    activeFilters[key] = value
  }
}

function clearFilter(key: string) {
  delete activeFilters[key]
}

const filteredItems = computed(() => {
  let result = props.items

  // Apply text search
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase()
    const fields = props.searchFields || ['name', 'id']
    result = result.filter(item =>
      fields.some(field => {
        const val = item[field]
        return typeof val === 'string' && val.toLowerCase().includes(q)
      })
    )
  }

  // Apply active filters
  for (const [key, value] of Object.entries(activeFilters)) {
    if (value) {
      result = result.filter(item => item[key] === value)
    }
  }

  return result
})

defineExpose({ filteredItems, activeFilters })
</script>
