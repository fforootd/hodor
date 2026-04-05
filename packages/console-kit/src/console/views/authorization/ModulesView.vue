<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Modules</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Enable authorization paradigms for your applications.
        </p>
      </div>
      <Badge variant="outline" class="gap-1.5">
        <Package class="size-3" />
        Marketplace
      </Badge>
    </div>

    <div class="rounded-lg border border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/30 px-4 py-3 flex items-start gap-3">
      <Info class="size-4 text-blue-500 mt-0.5 shrink-0" />
      <div class="text-sm">
        <p class="font-medium text-blue-900 dark:text-blue-200">Marketplace Modules</p>
        <p class="text-blue-700 dark:text-blue-400 mt-0.5">
          Modules extend the FGA model with additional types, relations, and conditions.
          Enable RBAC, ABAC, or Teams to add structured authorization to your tenant store.
        </p>
      </div>
    </div>

    <div v-if="loading" class="space-y-3">
      <div v-for="i in 3" :key="i" class="h-16 rounded-lg bg-muted animate-pulse" />
    </div>

    <div v-else class="grid grid-cols-1 gap-3">
      <Card v-for="mod in modules" :key="mod.name" class="p-4 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="p-2 rounded-lg" :class="mod.enabled ? 'bg-green-100 dark:bg-green-950/40' : 'bg-muted'">
            <Package class="size-4" :class="mod.enabled ? 'text-green-600' : 'text-muted-foreground'" />
          </div>
          <div>
            <h4 class="font-medium text-sm capitalize">{{ mod.name }}</h4>
            <p class="text-xs text-muted-foreground">{{ mod.description }}</p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Badge :variant="mod.enabled ? 'default' : 'secondary'" class="text-xs">
            {{ mod.enabled ? 'Enabled' : 'Disabled' }}
          </Badge>
          <Button
            :variant="mod.enabled ? 'destructive' : 'default'"
            size="sm"
            @click="toggleModule(mod)"
          >
            {{ mod.enabled ? 'Disable' : 'Enable' }}
          </Button>
        </div>
      </Card>

      <Empty v-if="!loading && !modules.length">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Package />
          </EmptyMedia>
          <EmptyTitle>No Modules Available</EmptyTitle>
          <EmptyDescription>
            Modules will appear here once the marketplace is configured.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { moduleApi, type Module } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription } from '@/components/ui/empty'
import { Package, Info } from 'lucide-vue-next'

const loading = ref(false)
const modules = ref<Module[]>([])

async function toggleModule(mod: Module) {
  try {
    if (mod.enabled) {
      await moduleApi.disable(mod.name)
      toast.success(`Disabled ${mod.name}`)
    } else {
      await moduleApi.enable(mod.name)
      toast.success(`Enabled ${mod.name}`)
    }
    modules.value = await moduleApi.list()
  } catch (err: any) {
    toast.error('Failed to toggle module', { description: err.message })
  }
}

onMounted(async () => {
  loading.value = true
  try {
    modules.value = await moduleApi.list()
  } catch { /* ok */ }
  loading.value = false
})
</script>
