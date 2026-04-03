<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="icon" class="size-8" @click="$router.push('/actions')">
          <ArrowLeft class="size-4" />
        </Button>
        <div>
          <h1 class="text-xl font-semibold">{{ action?.name || 'Action' }}</h1>
          <div class="flex items-center gap-2 mt-1">
            <Badge variant="outline" class="text-[10px]">{{ action?.hook }}</Badge>
            <Badge variant="outline" class="text-[10px]">{{ action?.action_type }}</Badge>
            <Badge
              :variant="action?.enabled ? 'default' : 'secondary'"
              class="text-[10px]"
            >
              {{ action?.enabled ? 'enabled' : 'disabled' }}
            </Badge>
          </div>
        </div>
      </div>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" @click="toggleEnabled">
          {{ action?.enabled ? 'Disable' : 'Enable' }}
        </Button>
        <Button variant="destructive" size="sm" @click="deleteAction">Delete</Button>
      </div>
    </div>

    <div v-if="loading" class="flex h-40 items-center justify-center text-muted-foreground">
      Loading...
    </div>

    <template v-if="action && !loading">
      <!-- Trigger -->
      <Card>
        <CardHeader>
          <CardTitle class="text-sm">Trigger</CardTitle>
        </CardHeader>
        <CardContent>
          <div class="space-y-3">
            <div>
              <label class="text-xs font-medium text-muted-foreground">Hook</label>
              <p class="text-sm font-mono mt-1">{{ action.hook }}</p>
            </div>
            <div>
              <label class="text-xs font-medium text-muted-foreground">Trigger Expression (CEL)</label>
              <pre
                class="mt-1 p-3 rounded-md bg-muted text-xs font-mono whitespace-pre-wrap"
              >{{ action.trigger_expr }}</pre>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- Configuration -->
      <Card>
        <CardHeader>
          <CardTitle class="text-sm">Configuration</CardTitle>
        </CardHeader>
        <CardContent>
          <pre
            class="p-3 rounded-md bg-muted text-xs font-mono whitespace-pre-wrap overflow-auto max-h-80"
          >{{ JSON.stringify(action.config, null, 2) }}</pre>
        </CardContent>
      </Card>

      <!-- Settings -->
      <Card>
        <CardHeader>
          <CardTitle class="text-sm">Settings</CardTitle>
        </CardHeader>
        <CardContent>
          <div class="grid grid-cols-2 gap-4 text-sm">
            <div>
              <label class="text-xs font-medium text-muted-foreground">Priority</label>
              <p>{{ action.priority }}</p>
            </div>
            <div>
              <label class="text-xs font-medium text-muted-foreground">Fail Open</label>
              <p>{{ action.fail_open ? 'Yes' : 'No' }}</p>
            </div>
            <div>
              <label class="text-xs font-medium text-muted-foreground">Created</label>
              <p>{{ action.created_at }}</p>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- Origin tracking -->
      <p
        v-if="catalogOrigin"
        class="text-xs text-muted-foreground"
      >
        Added from <span class="font-medium">{{ catalogOrigin }}</span> template
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import { toast } from 'vue-sonner'
  import { api } from '@/api/client'
  import { Button } from '@/components/ui/button'
  import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
  import { Badge } from '@/components/ui/badge'
  import { ArrowLeft } from 'lucide-vue-next'

  interface ActionRecord {
    id: string
    name: string
    hook: string
    action_type: string
    trigger_expr: string
    config: Record<string, any>
    priority: number
    enabled: boolean
    fail_open: boolean
    metadata: Record<string, any>
    created_at: string
  }

  const route = useRoute()
  const router = useRouter()
  const action = ref<ActionRecord | null>(null)
  const loading = ref(true)

  const catalogOrigin = computed(() => {
    const catalog = action.value?.metadata?._catalog
    if (!catalog?.template_id) return null
    const version = catalog.template_version ? ` v${catalog.template_version}` : ''
    return `${catalog.template_id}${version}`
  })

  onMounted(async () => {
    try {
      action.value = await api.get<ActionRecord>(`/v1/actions/${route.params.id}`)
    } catch {
      toast.error('Action not found')
      router.push('/actions')
    } finally {
      loading.value = false
    }
  })

  async function toggleEnabled() {
    if (!action.value) return
    try {
      // For now, re-fetch — PATCH not implemented yet
      toast.info('Toggle not yet implemented in backend')
    } catch (e: any) {
      toast.error('Failed', { description: e.message })
    }
  }

  async function deleteAction() {
    if (!action.value) return
    try {
      await api.delete(`/v1/actions/${action.value.id}`)
      toast.success('Action deleted')
      router.push('/actions')
    } catch (e: any) {
      toast.error('Failed', { description: e.message })
    }
  }
</script>
