<template>
  <ResourceListView
    v-model:search-query="searchQuery"
    title="Instances"
    singular-title="Instance"
    create-route="/instances/new"
    :items="items"
    :loading="loading"
    :columns="columns"
    :search-fields="['instance_id', 'primary_domain']"
  >
    <template #empty-icon><Server /></template>
  </ResourceListView>
</template>

<script setup lang="ts">
import { h, onMounted } from 'vue'
import { RouterLink } from 'vue-router'
import { createColumnHelper } from '@tanstack/vue-table'
import { instanceApi, type Instance } from '@/api/resources'
import { useResourceList } from '@/console/composables/useResourceList'
import { formatDate } from '@/console/utils/format'
import ResourceListView from '@/console/components/ResourceListView.vue'
import { StateBadge } from '@/components/ui/state-badge'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { Server } from 'lucide-vue-next'

const stateLabels: Record<string, string> = {
  active: 'Active',
  provisioning: 'Setting up',
  deprovisioning: 'Removing',
  suspended: 'Suspended',
}

const { items, loading, searchQuery, fetch } = useResourceList<Instance>({
  fetchFn: async () => {
    const res = await instanceApi.list()
    return res.items ?? []
  },
  resourceName: 'instances',
  searchFields: ['instance_id', 'primary_domain'],
})

const col = createColumnHelper<Instance>()
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
  col.accessor('primary_domain', {
    header: 'Name',
    cell: (info) =>
      h(RouterLink, {
        to: `/instances/${info.row.original.instance_id}`,
        class: 'font-medium hover:underline',
      }, () => info.getValue() || info.row.original.instance_id),
    filterFn: 'includesString',
  }),
  col.accessor('state', {
    header: 'Status',
    cell: (info) => h(StateBadge, { state: info.getValue(), label: stateLabels[info.getValue()] }),
  }),
  col.accessor('region_key', {
    header: 'Region',
    cell: (info) => info.getValue() || '—',
  }),
  col.accessor('kind', {
    header: 'Type',
    cell: (info) => h(Badge, { variant: 'outline' }, () => info.getValue()),
  }),
  col.accessor('created_at', {
    header: 'Created',
    cell: (info) => formatDate(info.getValue()),
  }),
]

onMounted(fetch)
</script>
