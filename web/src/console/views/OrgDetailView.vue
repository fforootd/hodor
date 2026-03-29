<template>
  <div class="space-y-6 max-w-3xl">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="icon" as-child>
          <router-link to="/orgs"><ArrowLeft class="size-4" /></router-link>
        </Button>
        <div v-if="org">
          <h1 class="text-2xl font-semibold tracking-tight">{{ org.name }}</h1>
          <div class="flex items-center gap-2 text-sm text-muted-foreground">
            <Badge :variant="org.state === 'active' ? 'default' : 'destructive'" class="capitalize text-xs">{{ org.state || 'active' }}</Badge>
            <span>·</span>
            <span class="text-xs">Created {{ formatDate(org.created_at) }}</span>
          </div>
        </div>
      </div>
      <Button v-if="org" variant="destructive" size="sm" @click="deleteOrg">Delete</Button>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="space-y-3">
      <div class="h-12 rounded-lg bg-muted animate-pulse" />
      <div class="h-8 rounded-lg bg-muted animate-pulse w-3/4" />
    </div>

    <!-- Error -->
    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">{{ error }}</div>

    <!-- Details -->
    <Card v-if="org">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Details</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="space-y-2">
          <Label for="org-name">Name</Label>
          <Input id="org-name" v-model="editName" />
        </div>
        <div class="space-y-2">
          <Label for="org-state">State</Label>
          <select id="org-state" v-model="editState" class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring">
            <option value="active">Active</option>
            <option value="inactive">Inactive</option>
          </select>
        </div>
      </CardContent>
    </Card>

    <!-- System -->
    <Card v-if="org">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">System Information</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="grid grid-cols-2 gap-y-2 text-sm">
          <span class="text-muted-foreground">ID</span>
          <span class="font-mono text-xs break-all">{{ org.id }}</span>
          <span class="text-muted-foreground">Instance</span>
          <span class="font-mono text-xs">{{ org.instance_id || '—' }}</span>
          <span class="text-muted-foreground">Created</span>
          <span>{{ formatDate(org.created_at) }}</span>
          <span class="text-muted-foreground">Updated</span>
          <span>{{ formatDate(org.updated_at) }}</span>
        </div>
      </CardContent>
    </Card>

    <!-- Save -->
    <div v-if="hasChanges" class="flex justify-end gap-3">
      <Button variant="outline" @click="resetEdits">Discard</Button>
      <Button :disabled="saving" @click="save">{{ saving ? 'Saving…' : 'Save Changes' }}</Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { orgApi, type Org } from '@/api/resources'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ArrowLeft } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

const org = ref<Org | null>(null)
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const editName = ref('')
const editState = ref('active')

const orgId = computed(() => route.params.id as string)

const hasChanges = computed(() => {
  if (!org.value) return false
  return editName.value !== (org.value.name || '') || editState.value !== (org.value.state || 'active')
})

function formatDate(ts?: string): string {
  if (!ts) return '—'
  try {
    return new Date(ts).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
  } catch { return ts }
}

function resetEdits() {
  if (org.value) {
    editName.value = org.value.name || ''
    editState.value = org.value.state || 'active'
  }
}

async function loadOrg() {
  loading.value = true
  error.value = ''
  try {
    org.value = await orgApi.get(orgId.value)
    resetEdits()
  } catch (e: any) {
    error.value = e?.message || 'Failed to load organization'
  } finally {
    loading.value = false
  }
}

async function save() {
  saving.value = true
  error.value = ''
  try {
    const changes: Record<string, any> = {}
    if (editName.value !== (org.value?.name || '')) changes.name = editName.value
    if (editState.value !== (org.value?.state || 'active')) changes.state = editState.value
    await orgApi.update(orgId.value, changes)
    await loadOrg()
  } catch (e: any) {
    error.value = e?.message || 'Failed to save'
  } finally {
    saving.value = false
  }
}

async function deleteOrg() {
  if (!confirm('Delete this organization? This cannot be undone.')) return
  try {
    await orgApi.delete(orgId.value)
    router.push('/orgs')
  } catch (e: any) {
    error.value = e?.message || 'Failed to delete'
  }
}

onMounted(loadOrg)
watch(orgId, loadOrg)
</script>
