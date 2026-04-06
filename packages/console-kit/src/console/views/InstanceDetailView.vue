<template>
  <div class="space-y-8">
    <!-- Back link -->
    <div>
      <router-link to="/instances" class="text-sm text-muted-foreground hover:text-foreground inline-flex items-center gap-1">
        <ArrowLeft class="size-4" />
        Back to Instances
      </router-link>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex h-48 items-center justify-center">
      <Spinner class="size-6 text-muted-foreground" />
    </div>

    <!-- Not found -->
    <div v-else-if="!instance" class="text-center py-16 text-muted-foreground">
      Instance not found.
    </div>

    <!-- Content -->
    <template v-else>
      <!-- Header -->
      <div class="flex items-start justify-between">
        <div>
          <h1 class="text-2xl font-semibold tracking-tight">{{ instance.primary_domain || instance.instance_id }}</h1>
          <p class="text-sm text-muted-foreground mt-1">{{ instance.instance_id }}</p>
        </div>
        <div class="flex items-center gap-2">
          <StateBadge :state="instance.state" :label="stateLabel" />
          <Badge variant="outline">{{ instance.kind }}</Badge>
        </div>
      </div>

      <!-- Tabs -->
      <Tabs v-model="activeTab" class="w-full">
        <TabsList>
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="domains">Domains</TabsTrigger>
          <TabsTrigger value="features">Features</TabsTrigger>
          <TabsTrigger value="settings">Settings</TabsTrigger>
        </TabsList>

        <!-- Overview -->
        <TabsContent value="overview" class="space-y-6 pt-4">
          <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <Card>
              <CardContent class="pt-4">
                <p class="text-xs text-muted-foreground">Status</p>
                <p class="text-sm font-medium mt-1">{{ stateLabel }}</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent class="pt-4">
                <p class="text-xs text-muted-foreground">Region</p>
                <p class="text-sm font-medium mt-1">{{ instance.region_key || 'Global' }}</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent class="pt-4">
                <p class="text-xs text-muted-foreground">Type</p>
                <p class="text-sm font-medium mt-1 capitalize">{{ instance.kind }}</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent class="pt-4">
                <p class="text-xs text-muted-foreground">Created</p>
                <p class="text-sm font-medium mt-1">{{ formatDate(instance.created_at) }}</p>
              </CardContent>
            </Card>
          </div>

          <!-- Quick actions -->
          <Card v-if="instance.state === 'active' || instance.state === 'suspended'">
            <CardContent class="pt-4 flex items-center justify-between">
              <div>
                <p class="text-sm font-medium">
                  {{ instance.state === 'active' ? 'Suspend Instance' : 'Reactivate Instance' }}
                </p>
                <p class="text-xs text-muted-foreground mt-0.5">
                  {{ instance.state === 'active' ? 'Temporarily disable this instance.' : 'Bring this instance back online.' }}
                </p>
              </div>
              <Button
                :variant="instance.state === 'active' ? 'outline' : 'default'"
                size="sm"
                :disabled="saving"
                @click="toggleState"
              >
                {{ instance.state === 'active' ? 'Suspend' : 'Activate' }}
              </Button>
            </CardContent>
          </Card>
        </TabsContent>

        <!-- Domains -->
        <TabsContent value="domains" class="space-y-4 pt-4">
          <div class="flex items-center justify-between">
            <p class="text-sm text-muted-foreground">Manage domains for this instance.</p>
            <Button size="sm" @click="showAddDomain = true">
              <Plus class="mr-1 size-4" /> Add Domain
            </Button>
          </div>

          <!-- Add domain inline form -->
          <Card v-if="showAddDomain">
            <CardContent class="pt-4 flex items-end gap-3">
              <div class="flex-1 space-y-1">
                <Label for="new-domain">Domain</Label>
                <Input id="new-domain" v-model="newDomain" placeholder="custom.example.com" />
              </div>
              <Button size="sm" :disabled="!newDomain.trim() || addingDomain" @click="addDomain">
                <Spinner v-if="addingDomain" class="mr-1 size-4" />
                Add
              </Button>
              <Button size="sm" variant="ghost" @click="showAddDomain = false; newDomain = ''">Cancel</Button>
            </CardContent>
          </Card>

          <!-- Domain list -->
          <Card>
            <div class="divide-y">
              <div
                v-for="d in domains"
                :key="d.domain"
                class="flex items-center justify-between px-4 py-3"
              >
                <div class="flex items-center gap-2">
                  <Globe class="size-4 text-muted-foreground" />
                  <span class="text-sm font-medium">{{ d.domain }}</span>
                  <Badge v-if="d.is_primary" variant="secondary" class="text-xs">Primary</Badge>
                  <Badge v-if="d.verified" variant="outline" class="text-xs">Verified</Badge>
                </div>
                <Button
                  v-if="!d.is_primary"
                  variant="ghost"
                  size="sm"
                  class="text-destructive hover:text-destructive"
                  :disabled="removingDomain === d.domain"
                  @click="removeDomain(d.domain)"
                >
                  <Trash2 class="size-4" />
                </Button>
              </div>
              <div v-if="domains.length === 0" class="px-4 py-6 text-center text-sm text-muted-foreground">
                No domains configured.
              </div>
            </div>
          </Card>
        </TabsContent>

        <!-- Features -->
        <TabsContent value="features" class="space-y-4 pt-4">
          <p class="text-sm text-muted-foreground">Feature flags for this instance. Toggle to override defaults.</p>
          <Card>
            <div class="divide-y">
              <div
                v-for="feature in featureList"
                :key="feature.key"
                class="flex items-center justify-between px-4 py-3"
              >
                <div>
                  <p class="text-sm font-medium">{{ feature.label }}</p>
                  <p class="text-xs text-muted-foreground">{{ feature.description }}</p>
                </div>
                <Switch
                  :checked="featureOverrides[feature.key] === true"
                  @update:checked="(val: boolean) => setFeature(feature.key, val)"
                />
              </div>
            </div>
          </Card>
        </TabsContent>

        <!-- Settings -->
        <TabsContent value="settings" class="space-y-6 pt-4">
          <!-- Placement -->
          <Card>
            <CardContent class="pt-4 space-y-4">
              <div>
                <p class="text-sm font-medium">Placement</p>
                <p class="text-xs text-muted-foreground">Where this instance's data is stored.</p>
              </div>
              <div class="grid grid-cols-2 gap-3">
                <div class="rounded-lg border p-3">
                  <p class="text-xs text-muted-foreground">Mode</p>
                  <p class="text-sm font-medium capitalize">{{ instance.placement_mode }}</p>
                </div>
                <div class="rounded-lg border p-3">
                  <p class="text-xs text-muted-foreground">Region</p>
                  <p class="text-sm font-medium">{{ instance.region_key || 'Global' }}</p>
                </div>
              </div>
            </CardContent>
          </Card>

          <!-- Danger zone -->
          <Card class="border-destructive/50">
            <CardContent class="pt-4 flex items-center justify-between">
              <div>
                <p class="text-sm font-medium text-destructive">Delete Instance</p>
                <p class="text-xs text-muted-foreground">This action cannot be undone. All data will be permanently removed.</p>
              </div>
              <Button variant="destructive" size="sm" :disabled="deleting" @click="confirmDelete">
                <Spinner v-if="deleting" class="mr-1 size-4" />
                Delete
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </template>

    <!-- Delete confirmation dialog -->
    <Dialog v-model:open="showDeleteDialog">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Delete Instance</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ instance?.primary_domain || instance?.instance_id }}</strong>?
            This will permanently remove all data associated with this instance.
          </DialogDescription>
        </DialogHeader>
        <div class="space-y-2">
          <Label>Type the instance ID to confirm:</Label>
          <Input v-model="deleteConfirmation" :placeholder="instance?.instance_id" />
        </div>
        <div class="flex justify-end gap-2 mt-4">
          <Button variant="outline" @click="showDeleteDialog = false">Cancel</Button>
          <Button
            variant="destructive"
            :disabled="deleteConfirmation !== instance?.instance_id || deleting"
            @click="doDelete"
          >
            <Spinner v-if="deleting" class="mr-1 size-4" />
            Delete permanently
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { instanceApi, type Instance, type InstanceDomain } from '@/api/resources'
import { notifySuccess, notifyError } from '@/lib/notify'
import { formatDate } from '@/console/utils/format'
import { Card, CardContent } from '@/components/ui/card'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Spinner } from '@/components/ui/spinner'
import { StateBadge } from '@/components/ui/state-badge'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { ArrowLeft, Globe, Plus, Trash2 } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()
const instanceId = computed(() => route.params.id as string)

const loading = ref(true)
const saving = ref(false)
const deleting = ref(false)
const instance = ref<Instance | null>(null)
const domains = ref<InstanceDomain[]>([])
const activeTab = ref('overview')

// Domain management
const showAddDomain = ref(false)
const newDomain = ref('')
const addingDomain = ref(false)
const removingDomain = ref('')

// Delete confirmation
const showDeleteDialog = ref(false)
const deleteConfirmation = ref('')

// Feature overrides
const featureOverrides = computed(() => instance.value?.feature_overrides || {})

const featureList = [
  { key: 'instance_management', label: 'Instance Management', description: 'Allow creating child instances from this instance.' },
  { key: 'billing', label: 'Billing', description: 'Enable billing and subscription management.' },
  { key: 'federation', label: 'Federation', description: 'Allow federated trust links with other instances.' },
  { key: 'custom_domains', label: 'Custom Domains', description: 'Allow adding custom domains with verification.' },
]

const stateLabels: Record<string, string> = {
  active: 'Active',
  provisioning: 'Setting up',
  deprovisioning: 'Removing',
  suspended: 'Suspended',
}
const stateLabel = computed(() => stateLabels[instance.value?.state || ''] || instance.value?.state || '')

async function loadInstance() {
  loading.value = true
  try {
    instance.value = await instanceApi.get(instanceId.value)
  } catch (e: any) {
    notifyError('Failed to load instance', e)
    instance.value = null
    domains.value = []
    return
  }

  try {
    domains.value = await instanceApi.listDomains(instanceId.value)
  } catch (e: any) {
    domains.value = []
    notifyError('Failed to load instance domains', e)
  } finally {
    loading.value = false
  }
}

async function toggleState() {
  if (!instance.value) return
  saving.value = true
  const newState = instance.value.state === 'active' ? 'suspended' : 'active'
  try {
    instance.value = await instanceApi.update(instanceId.value, { state: newState })
    notifySuccess('Instance updated', `Instance is now ${stateLabels[newState] || newState}.`)
  } catch (e: any) {
    notifyError('Failed to update instance', e)
  } finally {
    saving.value = false
  }
}

async function addDomain() {
  if (!newDomain.value.trim()) return
  addingDomain.value = true
  try {
    await instanceApi.addDomain(instanceId.value, newDomain.value.trim())
    notifySuccess('Domain added', newDomain.value)
    newDomain.value = ''
    showAddDomain.value = false
    domains.value = await instanceApi.listDomains(instanceId.value)
  } catch (e: any) {
    notifyError('Failed to add domain', e)
  } finally {
    addingDomain.value = false
  }
}

async function removeDomain(domain: string) {
  removingDomain.value = domain
  try {
    await instanceApi.removeDomain(instanceId.value, domain)
    notifySuccess('Domain removed', domain)
    domains.value = await instanceApi.listDomains(instanceId.value)
  } catch (e: any) {
    notifyError('Failed to remove domain', e)
  } finally {
    removingDomain.value = ''
  }
}

async function setFeature(key: string, value: boolean) {
  if (!instance.value) return
  const updated = { ...instance.value.feature_overrides, [key]: value }
  try {
    instance.value = await instanceApi.update(instanceId.value, { feature_overrides: updated })
    notifySuccess('Feature updated', `${key} is now ${value ? 'enabled' : 'disabled'}.`)
  } catch (e: any) {
    notifyError('Failed to update feature', e)
  }
}

function confirmDelete() {
  deleteConfirmation.value = ''
  showDeleteDialog.value = true
}

async function doDelete() {
  deleting.value = true
  try {
    await instanceApi.delete(instanceId.value)
    notifySuccess('Instance deleted', 'The instance is being removed.')
    showDeleteDialog.value = false
    router.push('/instances')
  } catch (e: any) {
    notifyError('Failed to delete instance', e)
  } finally {
    deleting.value = false
  }
}

onMounted(loadInstance)
</script>
