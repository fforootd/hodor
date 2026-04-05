<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">All Instances</h1>
      <p class="text-sm text-muted-foreground">Operator view — all child instances across all organizations.</p>
    </div>

    <!-- Search -->
    <div class="w-full max-w-lg relative">
      <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground z-10" />
      <Input
        placeholder="Search by domain, org, or instance ID..."
        class="pl-9 bg-background"
        v-model="searchQuery"
      />
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex h-48 items-center justify-center">
      <Spinner class="size-6 text-muted-foreground" />
    </div>

    <!-- Instance list -->
    <Card v-else>
      <div class="divide-y">
        <div
          v-for="inst in filteredInstances"
          :key="inst.instance_id"
          class="flex items-center justify-between px-4 py-3"
        >
          <div class="flex items-center gap-3">
            <Server class="size-4 text-muted-foreground" />
            <div>
              <router-link
                :to="`/instances/${inst.instance_id}`"
                class="text-sm font-medium hover:underline"
              >
                {{ inst.primary_domain || inst.instance_id }}
              </router-link>
              <p class="text-xs text-muted-foreground">
                Owner: {{ inst.owner_org_id }} &middot; {{ inst.region_key || 'Global' }}
              </p>
            </div>
          </div>
          <div class="flex items-center gap-2">
            <StateBadge :state="inst.state" />
            <Badge variant="outline">{{ inst.kind }}</Badge>
          </div>
        </div>
        <div v-if="filteredInstances.length === 0" class="px-4 py-8 text-center text-sm text-muted-foreground">
          {{ searchQuery ? 'No instances match your search.' : 'No instances found.' }}
        </div>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { instanceApi, type Instance } from '@/api/resources'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { StateBadge } from '@/components/ui/state-badge'
import { Spinner } from '@/components/ui/spinner'
import { Server, Search } from 'lucide-vue-next'

const loading = ref(true)
const instances = ref<Instance[]>([])
const searchQuery = ref('')

const filteredInstances = computed(() => {
  if (!searchQuery.value.trim()) return instances.value
  const q = searchQuery.value.toLowerCase()
  return instances.value.filter(
    (i) =>
      i.instance_id.toLowerCase().includes(q) ||
      (i.primary_domain || '').toLowerCase().includes(q) ||
      i.owner_org_id.toLowerCase().includes(q),
  )
})

onMounted(async () => {
  try {
    // TODO: operator endpoint should be unscoped (/v1/admin/instances).
    // For POC, using the regular endpoint which shows caller's org instances.
    const res = await instanceApi.list({ limit: 200 })
    instances.value = res.items ?? []
  } catch {
    // Silently handle — may not have operator access.
  }
  loading.value = false
})
</script>
