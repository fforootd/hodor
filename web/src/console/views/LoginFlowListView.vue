<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">Login Flows</h1>
        <p class="text-muted-foreground text-sm mt-1">
          Manage login experiences. Templates are the fastest way to start, and the default flow
          applies to users not matched by a more specific flow.
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button @click="$router.push('/marketplace?type=login_flow')">
          <Store class="size-4 mr-2" />
          Use Template
        </Button>
        <Button variant="outline" @click="showCreateDialog = true">
          <Plus class="size-4 mr-2" />
          Start Manually
        </Button>
      </div>
    </div>

    <div v-if="loading" class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>

    <div v-else-if="flows.length === 0" class="text-center py-12">
      <div class="text-4xl mb-3">🔐</div>
      <h3 class="text-lg font-semibold">No Login Flows</h3>
      <p class="text-muted-foreground text-sm mt-1">Something went wrong — the default flow should always exist.</p>
    </div>

    <div v-else class="space-y-4">
      <Card
        v-if="defaultFlow"
        class="border-primary/20 bg-primary/[0.02] hover:shadow-md transition-shadow cursor-pointer"
        @click="$router.push(`/login-flows/${defaultFlow.id}`)"
      >
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <div class="flex items-center justify-center size-8 rounded-md bg-primary/10">
                <Shield class="size-4 text-primary" />
              </div>
              <div>
                <CardTitle class="text-base">{{ defaultFlow.name || 'Default Login' }}</CardTitle>
                <p class="text-xs text-muted-foreground mt-0.5">
                  Applies to all users not matched by a specific flow
                </p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <Badge variant="default" class="text-xs">Default</Badge>
              <Badge v-if="isTemplateBacked(defaultFlow)" variant="secondary" class="text-xs">Template</Badge>
              <Badge :variant="stateVariant(defaultFlow.state)" class="text-xs">{{ defaultFlow.state }}</Badge>
            </div>
          </div>
        </CardHeader>
        <CardContent class="text-sm text-muted-foreground space-y-2">
          <div class="flex flex-wrap gap-1.5">
            <Badge variant="outline" class="text-xs">Strategy: {{ formatStrategy(defaultFlow.strategy) }}</Badge>
            <Badge v-if="getLayout(defaultFlow)" variant="outline" class="text-xs">
              Layout: {{ getLayoutLabel(getLayout(defaultFlow)) }}
            </Badge>
            <Badge v-if="getCaptcha(defaultFlow)" variant="outline" class="text-xs">
              🛡️ {{ getCaptchaProvider(defaultFlow) }}
            </Badge>
            <Badge v-if="getFingerprint(defaultFlow)" variant="outline" class="text-xs">🔍 Fingerprint</Badge>
            <Badge v-if="getRateLimit(defaultFlow)" variant="outline" class="text-xs">⏱️ Rate limit</Badge>
            <Badge v-if="getTelemetry(defaultFlow)" variant="outline" class="text-xs">📊 Telemetry</Badge>
          </div>
          <div class="flex items-center justify-between pt-1 text-xs text-muted-foreground/70">
            <span>Priority: {{ defaultFlow.priority }}</span>
            <span>{{ formatDate(defaultFlow.created_at) }}</span>
          </div>
        </CardContent>
      </Card>

      <div v-if="customFlows.length > 0" class="flex items-center gap-3 py-1">
        <div class="flex-1 h-px bg-border" />
        <span class="text-xs text-muted-foreground font-medium">Custom Flows</span>
        <div class="flex-1 h-px bg-border" />
      </div>

      <div v-if="customFlows.length > 0" class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        <Card
          v-for="flow in customFlows"
          :key="flow.id"
          class="group hover:shadow-md transition-shadow cursor-pointer"
          @click="$router.push(`/login-flows/${flow.id}`)"
        >
          <CardHeader class="pb-3">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <span class="text-xl">🔐</span>
                <CardTitle class="text-base">{{ flow.name }}</CardTitle>
              </div>
              <div class="flex items-center gap-1.5">
                <Badge v-if="isTemplateBacked(flow)" variant="secondary" class="text-xs">Template</Badge>
                <Badge :variant="stateVariant(flow.state)" class="text-xs">{{ flow.state }}</Badge>
              </div>
            </div>
          </CardHeader>
          <CardContent class="text-sm text-muted-foreground space-y-2">
            <div v-if="hasAudience(flow)" class="flex flex-wrap gap-1.5">
              <Badge v-if="audienceSchemaCount(flow)" variant="secondary" class="text-xs">
                {{ audienceSchemaCount(flow) }} schema{{ audienceSchemaCount(flow) > 1 ? 's' : '' }}
              </Badge>
              <Badge v-if="audienceUserCount(flow)" variant="secondary" class="text-xs">
                {{ audienceUserCount(flow) }} user{{ audienceUserCount(flow) > 1 ? 's' : '' }}
              </Badge>
              <Badge v-if="audienceOrgCount(flow)" variant="secondary" class="text-xs">
                {{ audienceOrgCount(flow) }} org{{ audienceOrgCount(flow) > 1 ? 's' : '' }}
              </Badge>
            </div>
            <div v-else class="text-xs text-muted-foreground/60 italic">No audience targeting</div>

            <div class="flex flex-wrap gap-1.5">
              <Badge variant="outline" class="text-xs">Strategy: {{ formatStrategy(flow.strategy) }}</Badge>
              <Badge v-if="getLayout(flow)" variant="outline" class="text-xs">
                Layout: {{ getLayoutLabel(getLayout(flow)) }}
              </Badge>
              <Badge v-if="getCaptcha(flow)" variant="outline" class="text-xs">
                🛡️ {{ getCaptchaProvider(flow) }}
              </Badge>
              <Badge v-if="getFingerprint(flow)" variant="outline" class="text-xs">🔍 Fingerprint</Badge>
              <Badge v-if="getRateLimit(flow)" variant="outline" class="text-xs">⏱️ Rate limit</Badge>
            </div>

            <div class="flex items-center justify-between pt-1 text-xs text-muted-foreground/70">
              <span>Priority: {{ flow.priority }}</span>
              <span>{{ formatDate(flow.created_at) }}</span>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>

    <Dialog v-model:open="showCreateDialog">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Start Login Flow Manually</DialogTitle>
          <DialogDescription>
            Templates are recommended for complete starting points. Manual setup creates a blank
            flow with the defaults you can refine in the editor.
          </DialogDescription>
        </DialogHeader>
        <form class="space-y-4" @submit.prevent="createFlow">
          <div class="space-y-1.5">
            <Label for="flow-name">Name</Label>
            <Input id="flow-name" v-model="newFlow.name" placeholder="e.g. B2B Beta Login" required />
          </div>
          <div class="space-y-1.5">
            <Label for="flow-state">Start state</Label>
            <select
              id="flow-state"
              v-model="newFlow.state"
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="draft">Draft (not served)</option>
              <option value="testing">Testing (served to user allowlist only)</option>
              <option value="active">Active (served to all matching users)</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <Label for="flow-strategy">Flow Strategy</Label>
            <select
              id="flow-strategy"
              v-model="newFlow.strategy"
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="identifier_first">Identifier first</option>
              <option value="passkey_first">Passkey first</option>
              <option value="sso_only">SSO only</option>
              <option value="custom">Custom</option>
            </select>
            <p class="text-xs text-muted-foreground">
              Layout defaults to centered. You can edit layout and protections after creation.
            </p>
          </div>
          <div class="flex justify-end gap-2 pt-2">
            <Button type="button" variant="outline" @click="showCreateDialog = false">Cancel</Button>
            <Button type="submit" :disabled="creating">
              <Spinner v-if="creating" class="size-4 mr-2" />
              Create flow
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { api } from '@/api/client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Plus, Shield, Store } from 'lucide-vue-next'

interface LoginFlow {
  id: string
  name: string
  strategy: string
  is_default: boolean
  enabled: boolean
  state: string
  priority: number
  audience: any
  auth_methods: any
  config: any
  metadata?: any
  created_at: string
  updated_at: string
}

const flows = ref<LoginFlow[]>([])
const loading = ref(true)
const showCreateDialog = ref(false)
const creating = ref(false)

const defaultFlow = computed(() => flows.value.find((f) => f.is_default))
const customFlows = computed(() => flows.value.filter((f) => !f.is_default))

const newFlow = ref({
  name: '',
  state: 'draft',
  strategy: 'identifier_first',
})

function getConfig(flow: LoginFlow): any {
  if (!flow.config) return {}
  if (typeof flow.config === 'string') {
    try { return JSON.parse(flow.config) } catch { return {} }
  }
  return flow.config
}

function getCaptcha(flow: LoginFlow) {
  return getConfig(flow).captcha
}

function getCaptchaProvider(flow: LoginFlow) {
  return getCaptcha(flow)?.provider || 'altcha'
}

function getFingerprint(flow: LoginFlow) {
  const fp = getConfig(flow).fingerprint
  return fp?.enabled === false ? null : fp
}

function getRateLimit(flow: LoginFlow) {
  return getConfig(flow).rate_limit
}

function getTelemetry(flow: LoginFlow) {
  const telemetry = getConfig(flow).telemetry
  return telemetry?.enabled === false ? null : telemetry
}

function getLayout(flow: LoginFlow) {
  return getConfig(flow).branding?.layout || ''
}

function getLayoutLabel(layout: string) {
  switch (layout) {
    case 'card_image': return 'Card with image'
    default:
      return layout
        .replace(/_/g, ' ')
        .replace(/\b\w/g, (char) => char.toUpperCase())
  }
}

function formatStrategy(strategy: string) {
  switch (strategy) {
    case 'identifier_first': return 'Identifier first'
    case 'passkey_first': return 'Passkey first'
    case 'sso_only': return 'SSO only'
    case 'custom': return 'Custom'
    default: return strategy
  }
}

function isTemplateBacked(flow: LoginFlow) {
  return Boolean(flow.metadata?._catalog?.template_id)
}

function hasAudience(flow: LoginFlow) {
  if (!flow.audience) return false
  const audience = typeof flow.audience === 'string' ? safeJSON(flow.audience) : flow.audience
  return audience.schema_ids?.length > 0 || audience.user_ids?.length > 0 || audience.org_ids?.length > 0
}

function audienceSchemaCount(flow: LoginFlow) {
  return (typeof flow.audience === 'string' ? safeJSON(flow.audience) : flow.audience || {}).schema_ids?.length || 0
}

function audienceUserCount(flow: LoginFlow) {
  return (typeof flow.audience === 'string' ? safeJSON(flow.audience) : flow.audience || {}).user_ids?.length || 0
}

function audienceOrgCount(flow: LoginFlow) {
  return (typeof flow.audience === 'string' ? safeJSON(flow.audience) : flow.audience || {}).org_ids?.length || 0
}

function safeJSON(value: string) {
  try { return JSON.parse(value) } catch { return {} }
}

function stateVariant(state?: string): 'default' | 'secondary' | 'outline' | 'destructive' {
  switch (state) {
    case 'active': return 'default'
    case 'testing': return 'secondary'
    case 'archived': return 'destructive'
    default: return 'outline'
  }
}

function formatDate(value?: string) {
  return value ? new Date(value).toLocaleDateString() : ''
}

async function loadFlows() {
  loading.value = true
  try {
    flows.value = (await api.get<{ items: LoginFlow[] }>('/v1/login-flows')).items || []
  } catch {
    flows.value = []
  } finally {
    loading.value = false
  }
}

async function createFlow() {
  creating.value = true
  try {
    await api.post('/v1/login-flows', {
      name: newFlow.value.name,
      strategy: newFlow.value.strategy,
      state: newFlow.value.state,
      config: {
        captcha: {
          provider: 'altcha',
          mode: 'risk_based',
          difficulty: 3,
          steps: ['identifier', 'password'],
        },
        fingerprint: {
          enabled: true,
          provider: 'thumbmarkjs',
          persist: true,
          steps: ['identifier'],
        },
        rate_limit: {
          max_attempts: 5,
          window_seconds: 300,
          lockout_seconds: 900,
          scope: 'ip',
        },
        telemetry: {
          enabled: true,
          sample_rate: 1.0,
        },
        branding: {
          layout: 'centered',
        },
      },
    })
    showCreateDialog.value = false
    newFlow.value = {
      name: '',
      state: 'draft',
      strategy: 'identifier_first',
    }
    await loadFlows()
  } catch (e: any) {
    console.error('Failed to create login flow:', e)
  } finally {
    creating.value = false
  }
}

onMounted(loadFlows)
</script>
