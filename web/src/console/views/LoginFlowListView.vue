<template>
  <TooltipProvider>
    <div class="space-y-6">
      <div class="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div class="space-y-2">
          <div class="flex items-center gap-2">
            <h1 class="text-2xl font-bold tracking-tight">Login Flows</h1>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="icon" class="size-8 text-muted-foreground">
                  <Info class="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent class="max-w-72">
                The default flow is the fallback experience. Targeted flows override it when a user
                matches a specific audience.
              </TooltipContent>
            </Tooltip>
          </div>
          <p class="text-sm text-muted-foreground">
            {{ defaultFlow ? `Default fallback plus ${customFlows.length} targeted flow${customFlows.length === 1 ? '' : 's'}.` : 'Manage your login flow set.' }}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <Button @click="$router.push('/marketplace?type=login_flow')">
            <Store class="mr-2 size-4" />
            Use Template
          </Button>
          <Button variant="outline" @click="showCreateDialog = true">
            <Plus class="mr-2 size-4" />
            Start Manually
          </Button>
        </div>
      </div>

      <div v-if="loading" class="flex justify-center py-12">
        <Spinner class="size-6" />
      </div>

      <div v-else-if="flows.length === 0" class="py-12 text-center">
        <div class="mb-3 text-4xl">🔐</div>
        <h3 class="text-lg font-semibold">No Login Flows</h3>
        <p class="mt-1 text-sm text-muted-foreground">Something went wrong — the default flow should always exist.</p>
      </div>

      <div v-else class="space-y-4">
        <Card
          v-if="defaultFlow"
          class="cursor-pointer border-primary/20 bg-primary/[0.02] transition-shadow hover:shadow-md"
          @click="$router.push(`/login-flows/${defaultFlow.id}`)"
        >
          <CardHeader class="pb-3">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2.5">
                <div class="flex size-8 items-center justify-center rounded-md bg-primary/10">
                  <Shield class="size-4 text-primary" />
                </div>
                <div class="space-y-1">
                  <CardTitle class="text-base">{{ defaultFlow.name || 'Default Login' }}</CardTitle>
                  <Badge variant="outline" class="w-fit text-[11px]">Fallback coverage</Badge>
                </div>
              </div>
              <div class="flex items-center gap-2">
                <Badge variant="default" class="text-xs">Default</Badge>
                <Badge v-if="isTemplateBacked(defaultFlow)" variant="secondary" class="text-xs">Template</Badge>
                <Badge :variant="stateVariant(defaultFlow.state)" class="text-xs">{{ defaultFlow.state }}</Badge>
              </div>
            </div>
          </CardHeader>
          <CardContent class="space-y-2 text-sm text-muted-foreground">
            <div class="flex flex-wrap gap-1.5">
              <Badge variant="outline" class="text-xs">{{ formatStrategy(defaultFlow.strategy) }}</Badge>
              <Badge v-if="getLayout(defaultFlow)" variant="outline" class="text-xs">
                {{ getLayoutLabel(getLayout(defaultFlow)) }}
              </Badge>
              <Badge v-if="getCaptcha(defaultFlow)" variant="outline" class="text-xs">
                {{ getCaptchaProvider(defaultFlow) }}
              </Badge>
              <Badge v-if="getFingerprint(defaultFlow)" variant="outline" class="text-xs">Fingerprint</Badge>
              <Badge v-if="getRateLimit(defaultFlow)" variant="outline" class="text-xs">Rate limit</Badge>
              <Badge v-if="getTelemetry(defaultFlow)" variant="outline" class="text-xs">Telemetry</Badge>
            </div>
            <div class="flex items-center justify-between pt-1 text-xs text-muted-foreground/70">
              <span>Priority {{ defaultFlow.priority }}</span>
              <span>{{ formatDate(defaultFlow.created_at) }}</span>
            </div>
          </CardContent>
        </Card>

        <div v-if="customFlows.length > 0" class="flex items-center gap-3 py-1">
          <div class="h-px flex-1 bg-border" />
          <span class="text-xs font-medium text-muted-foreground">Custom</span>
          <div class="h-px flex-1 bg-border" />
        </div>

        <div v-if="customFlows.length > 0" class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          <Card
            v-for="flow in customFlows"
            :key="flow.id"
            class="group cursor-pointer transition-shadow hover:shadow-md"
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
            <CardContent class="space-y-2 text-sm text-muted-foreground">
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

              <div class="flex flex-wrap gap-1.5">
                <Badge variant="outline" class="text-xs">{{ formatStrategy(flow.strategy) }}</Badge>
                <Badge v-if="getLayout(flow)" variant="outline" class="text-xs">
                  {{ getLayoutLabel(getLayout(flow)) }}
                </Badge>
                <Badge v-if="getCaptcha(flow)" variant="outline" class="text-xs">
                  {{ getCaptchaProvider(flow) }}
                </Badge>
                <Badge v-if="getFingerprint(flow)" variant="outline" class="text-xs">Fingerprint</Badge>
                <Badge v-if="getRateLimit(flow)" variant="outline" class="text-xs">Rate limit</Badge>
              </div>

              <div class="flex items-center justify-between pt-1 text-xs text-muted-foreground/70">
                <span>Priority {{ flow.priority }}</span>
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
            <DialogDescription>Creates a blank flow you can refine in the editor.</DialogDescription>
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
              <Label for="flow-strategy">Flow strategy</Label>
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
            </div>
            <div class="flex justify-end gap-2 pt-2">
              <Button type="button" variant="outline" @click="showCreateDialog = false">Cancel</Button>
              <Button type="submit" :disabled="creating">
                <Spinner v-if="creating" class="mr-2 size-4" />
                Create flow
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  </TooltipProvider>
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
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Info, Plus, Shield, Store } from 'lucide-vue-next'

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
