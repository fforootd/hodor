<template>
  <ResourceListView
    v-model:search-query="searchQuery"
    title="Projects"
    singular-title="Project"
    create-route="/projects/new"
    :items="items"
    :loading="loading"
    :columns="columns"
    :search-fields="['name', 'id']"
  >
    <template #header-actions>
      <Button @click="showCreate = true">
        <Plus class="mr-2 size-4" />
        New Project
      </Button>
    </template>
    <template #empty-icon><FolderKanban /></template>
  </ResourceListView>

  <ProjectCreateView
    :open="showCreate"
    @update:open="showCreate = $event"
    @created="fetch"
  />
</template>

<script setup lang="ts">
import { h, ref, onMounted, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { createColumnHelper } from '@tanstack/vue-table'
import { projectApi, type Project } from '@/api/resources'
import { useResourceList } from '@/console/composables/useResourceList'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { formatDate } from '@/console/utils/format'
import ResourceListView from '@/console/components/ResourceListView.vue'
import ProjectCreateView from '@/console/views/ProjectCreateView.vue'
import { StateBadge } from '@/components/ui/state-badge'
import { Badge } from '@/components/ui/badge'
import { Checkbox } from '@/components/ui/checkbox'
import { Button } from '@/components/ui/button'
import { FolderKanban, Plus } from 'lucide-vue-next'

const showCreate = ref(false)

const { currentOrgId } = useOrgContext()
const { items, loading, searchQuery, fetch } = useResourceList<Project>({
  fetchFn: () => projectApi.list(currentOrgId.value || undefined),
  resourceName: 'projects',
  searchFields: ['name', 'id'],
})

const col = createColumnHelper<Project>()
const columns = [
  col.display({
    id: 'select',
    header: ({ table }) =>
      h(Checkbox, {
        modelValue: table.getIsAllPageRowsSelected(),
        'onUpdate:modelValue': (val: boolean | 'indeterminate') => table.toggleAllPageRowsSelected(!!val),
        ariaLabel: 'Select all',
      }),
    cell: ({ row }) =>
      h(Checkbox, {
        modelValue: row.getIsSelected(),
        'onUpdate:modelValue': (val: boolean | 'indeterminate') => row.toggleSelected(!!val),
        ariaLabel: 'Select row',
      }),
    enableSorting: false,
    enableHiding: false,
  }),
  col.accessor('name', {
    header: 'Name',
    cell: (info) => h(RouterLink, {
      to: `/projects/${info.row.original.id}`,
      class: 'font-medium hover:underline',
    }, () => info.getValue() || info.row.original.id),
    filterFn: 'includesString',
  }),
  col.accessor('member_count', {
    header: 'Members',
    cell: (info) => h(Badge, { variant: 'secondary', class: 'text-xs' }, () => `${info.getValue() ?? 0}`),
  }),
  col.accessor('state', {
    header: 'State',
    cell: (info) => h(StateBadge, { state: info.getValue() }),
  }),
  col.accessor('created_at', {
    header: 'Created',
    cell: (info) => formatDate(info.getValue()),
  }),
  col.accessor('id', {
    header: 'ID',
    cell: (info) => h('code', { class: 'text-xs text-muted-foreground' }, info.getValue()?.slice(0, 8) + '…'),
  }),
]

onMounted(fetch)
watch(currentOrgId, fetch)
</script>
