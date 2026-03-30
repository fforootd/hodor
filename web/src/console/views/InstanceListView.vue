<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Instances</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Manage Zitadel instances. Each instance is an isolated tenant with its own users, orgs, and configuration.
        </p>
      </div>
      <Dialog v-model:open="showCreateDialog">
        <DialogTrigger as-child>
          <Button>
            <Plus class="size-4 mr-2" />
            Add Instance
          </Button>
        </DialogTrigger>
        <DialogContent class="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>Create Instance</DialogTitle>
            <DialogDescription>
              Create a new isolated tenant instance with its own users and configuration.
            </DialogDescription>
          </DialogHeader>
          <div class="grid gap-4 py-4">
            <div class="grid gap-2">
              <Label for="instance-name">Name</Label>
              <Input id="instance-name" v-model="newInstanceName" placeholder="e.g. Production" />
            </div>
            <div class="grid gap-2">
              <Label for="instance-domain">Domain (optional)</Label>
              <Input id="instance-domain" v-model="newInstanceDomain" placeholder="e.g. auth.acme.com" />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" @click="showCreateDialog = false">Cancel</Button>
            <Button @click="createInstance" :disabled="!newInstanceName.trim() || creating">
              <Loader2 v-if="creating" class="size-4 mr-2 animate-spin" />
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>

    <!-- Search -->
    <div class="relative">
      <Search class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
      <Input
        v-model="searchQuery"
        placeholder="Find Instance..."
        class="pl-10 max-w-md"
      />
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex items-center gap-2 text-sm text-muted-foreground py-8">
      <Loader2 class="size-4 animate-spin" />
      Loading instances...
    </div>

    <!-- Instance Cards -->
    <div v-else-if="filteredInstances.length" class="grid gap-3">
      <div
        v-for="inst in filteredInstances"
        :key="inst.id"
        class="group flex items-center gap-4 rounded-lg border p-4 transition-colors hover:bg-muted/50 cursor-pointer"
        @click="navigateToInstance(inst)"
      >
        <!-- Icon -->
        <div class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-muted">
          <Shield v-if="inst.is_root" class="size-5 text-primary" />
          <Server v-else class="size-5 text-muted-foreground" />
        </div>

        <!-- Info -->
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <span class="font-medium">{{ inst.name }}</span>
            <Badge v-if="inst.is_root" variant="secondary" class="text-xs">root</Badge>
            <Badge
              :variant="inst.state === 'active' ? 'default' : 'destructive'"
              class="text-xs"
            >
              {{ inst.state }}
            </Badge>
          </div>
          <p class="text-sm text-muted-foreground truncate">
            {{ inst.domain || inst.id }}
          </p>
        </div>

        <!-- Actions -->
        <div class="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
          <Button
            v-if="!inst.is_root"
            variant="ghost"
            size="icon"
            @click.stop="selectForEdit(inst)"
          >
            <Settings class="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            @click.stop="navigateToInstance(inst)"
          >
            <ArrowRight class="size-4" />
          </Button>
        </div>
      </div>

      <!-- Add Instance shortcut -->
      <button
        class="flex items-center gap-3 rounded-lg border border-dashed p-4 text-sm text-muted-foreground hover:bg-muted/50 transition-colors w-full"
        @click="showCreateDialog = true"
      >
        <Plus class="size-4" />
        Add Instance
      </button>
    </div>

    <!-- Empty state -->
    <div v-else class="flex flex-col items-center justify-center py-16 text-center">
      <Server class="size-12 text-muted-foreground/30 mb-4" />
      <h3 class="text-lg font-medium">No instances found</h3>
      <p class="text-sm text-muted-foreground mt-1 max-w-sm">
        {{ searchQuery ? 'No instances match your search.' : 'Create your first instance to get started.' }}
      </p>
      <Button v-if="!searchQuery" class="mt-4" @click="showCreateDialog = true">
        <Plus class="size-4 mr-2" />
        Create Instance
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { instanceApi, switchInstance, type Instance } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Label } from '@/components/ui/label'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter,
  DialogHeader, DialogTitle, DialogTrigger,
} from '@/components/ui/dialog'

import {
  Plus, Search, Shield, Server, Settings, ArrowRight, Loader2,
} from 'lucide-vue-next'

const router = useRouter()

const instances = ref<Instance[]>([])
const loading = ref(true)
const searchQuery = ref('')
const showCreateDialog = ref(false)
const newInstanceName = ref('')
const newInstanceDomain = ref('')
const creating = ref(false)

const filteredInstances = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()
  if (!q) return instances.value
  return instances.value.filter(i =>
    i.name.toLowerCase().includes(q) ||
    i.id.toLowerCase().includes(q) ||
    (i.domain && i.domain.toLowerCase().includes(q))
  )
})

async function loadInstances() {
  loading.value = true
  try {
    instances.value = await instanceApi.list()
  } catch (e) {
    console.error('Failed to load instances:', e)
    toast.error('Failed to load instances')
  } finally {
    loading.value = false
  }
}

async function createInstance() {
  if (!newInstanceName.value.trim()) return
  creating.value = true
  try {
    const inst = await instanceApi.create({
      name: newInstanceName.value.trim(),
      domain: newInstanceDomain.value.trim() || undefined,
    })
    toast.success(`Instance "${inst.name}" created`)
    showCreateDialog.value = false
    newInstanceName.value = ''
    newInstanceDomain.value = ''
    await loadInstances()
  } catch (e: any) {
    toast.error(e?.error || 'Failed to create instance')
  } finally {
    creating.value = false
  }
}

function navigateToInstance(inst: Instance) {
  if (inst.is_root) {
    // Switch back to root
    switchInstance(null)
    router.push('/')
  } else {
    // Switch to sub-instance
    switchInstance(inst.id)
    router.push('/')
  }
}

function selectForEdit(inst: Instance) {
  router.push(`/instances/${inst.id}`)
}

onMounted(loadInstances)
</script>
