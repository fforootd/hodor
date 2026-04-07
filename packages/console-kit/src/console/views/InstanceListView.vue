<template>
  <ResourceListView
    v-model:search-query="searchQuery"
    title="Instances"
    singular-title="Instance"
    description="Manage your ZITADEL instances across all environments"
    create-route="/instances/new"
    :items="items"
    :loading="loading"
    :columns="columns"
    :search-fields="['instance_id', 'primary_domain']"
    :filters="filters"
  >
    <template #header-actions>
      <Button @click="showCreate = true">
        <Plus class="mr-2 size-4" />
        New Instance
      </Button>
    </template>
    <template #empty-icon><Server /></template>
  </ResourceListView>

  <InstanceCreateView
    :open="showCreate"
    @update:open="showCreate = $event"
    @created="fetch"
  />
</template>

<script setup lang="ts">
import { h, ref, computed, onMounted } from 'vue'
import { RouterLink } from 'vue-router'
import { createColumnHelper } from '@tanstack/vue-table'
import { instanceApi, type Instance } from '@/api/resources'
import { useResourceList } from '@/console/composables/useResourceList'
import ResourceListView, { type ListFilter } from '@/console/components/ResourceListView.vue'
import InstanceCreateView from '@/console/views/InstanceCreateView.vue'
import { StateBadge } from '@/components/ui/state-badge'
import { Button } from '@/components/ui/button'
import { Server, Plus, Globe, Activity } from 'lucide-vue-next'

const showCreate = ref(false)

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

// Dynamic filters derived from data
const filters = computed<ListFilter[]>(() => {
  const states = new Map<string, number>()
  const regions = new Map<string, number>()
  const kinds = new Map<string, number>()

  for (const inst of items.value) {
    states.set(inst.state, (states.get(inst.state) || 0) + 1)
    if (inst.region_key) regions.set(inst.region_key, (regions.get(inst.region_key) || 0) + 1)
    if (inst.kind) kinds.set(inst.kind, (kinds.get(inst.kind) || 0) + 1)
  }

  const result: ListFilter[] = []

  if (kinds.size > 1) {
    result.push({
      key: 'kind',
      label: 'Hosting',
      icon: Server,
      options: [...kinds.entries()].map(([value, count]) => ({
        value,
        label: value === 'managed' ? 'Cloud Hosted' : value.charAt(0).toUpperCase() + value.slice(1),
        count,
      })),
    })
  }

  if (states.size > 1) {
    result.push({
      key: 'state',
      label: 'Status',
      icon: Activity,
      options: [...states.entries()].map(([value, count]) => ({
        value,
        label: stateLabels[value] || value,
        count,
      })),
    })
  }

  if (regions.size > 1) {
    result.push({
      key: 'region_key',
      label: 'Region',
      icon: Globe,
      options: [...regions.entries()].map(([value, count]) => ({
        value,
        label: formatRegion(value || null),
        count,
      })),
    })
  }

  return result
})

const regionLabels: Record<string, string> = {
  'eu-frankfurt': 'EU (Frankfurt)',
  'europe-west1': 'EU (Belgium)',
  'us-virginia': 'US (Virginia)',
  'us-central1': 'US (Iowa)',
  'us-oregon': 'US (Oregon)',
  'asia-singapore': 'Asia (Singapore)',
  'asia-southeast1': 'Asia (Singapore)',
  'asia-tokyo': 'Asia (Tokyo)',
  'au-sydney': 'Australia (Sydney)',
}

function formatRegion(key: string | null): string {
  if (!key) return 'Global'
  return regionLabels[key] || key
}

const col = createColumnHelper<Instance>()
const columns = [
  col.accessor('primary_domain', {
    header: 'Name',
    cell: (info) => {
      const inst = info.row.original
      return h(RouterLink, {
        to: `/instances/${inst.instance_id}`,
        class: 'font-medium hover:underline',
      }, () => inst.primary_domain || inst.instance_id)
    },
    filterFn: 'includesString',
  }),
  col.accessor('state', {
    header: 'Status',
    cell: (info) => h(StateBadge, { state: info.getValue(), label: stateLabels[info.getValue()] }),
  }),
  col.accessor('region_key', {
    header: 'Region',
    cell: (info) => h('span', { class: 'text-sm' }, formatRegion(info.getValue())),
  }),
]

onMounted(fetch)
</script>
