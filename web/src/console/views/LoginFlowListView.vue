<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">Login Flows</h1>
        <p class="text-muted-foreground text-sm mt-1">
          Manage login experiences. The default flow applies to all users not matched by a specific flow.
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="outline" @click="$router.push('/marketplace?type=login_flow')">
          <Store class="size-4 mr-2" />
          From Marketplace
        </Button>
        <Button @click="showCreateDialog = true">
          <Plus class="size-4 mr-2" />
          Create Flow
        </Button>
      </div>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>

    <!-- Empty state (should not happen since default always exists) -->
    <div v-else-if="flows.length === 0" class="text-center py-12">
      <div class="text-4xl mb-3">🔐</div>
      <h3 class="text-lg font-semibold">No Login Flows</h3>
      <p class="text-muted-foreground text-sm mt-1">Something went wrong — the default flow should always exist.</p>
    </div>

    <div v-else class="space-y-4">
      <!-- Default Flow — always shown first, visually distinct -->
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
              <Badge :variant="stateVariant(defaultFlow.state)" class="text-xs">{{ defaultFlow.state }}</Badge>
            </div>
          </div>
        </CardHeader>
        <CardContent class="text-sm text-muted-foreground space-y-2">
          <!-- Signal badges -->
          <div class="flex flex-wrap gap-1.5">
            <Badge v-if="getCaptcha(defaultFlow)" variant="outline" class="text-xs">
              🛡️ {{ getCaptchaProvider(defaultFlow) }}
            </Badge>
            <Badge v-if="getFingerprint(defaultFlow)" variant="outline" class="text-xs">
              🔍 Fingerprint
            </Badge>
            <Badge v-if="getRateLimit(defaultFlow)" variant="outline" class="text-xs">
              ⏱️ Rate limit
            </Badge>
            <Badge v-if="getTelemetry(defaultFlow)" variant="outline" class="text-xs">
              📊 Telemetry
            </Badge>
          </div>
          <div class="flex items-center justify-between pt-1 text-xs text-muted-foreground/70">
            <span>Priority: {{ defaultFlow.priority }}</span>
            <span>{{ formatDate(defaultFlow.created_at) }}</span>
          </div>
        </CardContent>
      </Card>

      <!-- Divider between default and custom flows -->
      <div v-if="customFlows.length > 0" class="flex items-center gap-3 py-1">
        <div class="flex-1 h-px bg-border" />
        <span class="text-xs text-muted-foreground font-medium">Custom Flows</span>
        <div class="flex-1 h-px bg-border" />
      </div>

      <!-- Custom Flows Grid -->
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
                <Badge :variant="stateVariant(flow.state)" class="text-xs">{{ flow.state }}</Badge>
              </div>
            </div>
          </CardHeader>
          <CardContent class="text-sm text-muted-foreground space-y-2">
            <!-- Audience summary -->
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

            <!-- Signal badges -->
            <div class="flex flex-wrap gap-1.5">
              <Badge v-if="getCaptcha(flow)" variant="outline" class="text-xs">
                🛡️ {{ getCaptchaProvider(flow) }}
              </Badge>
              <Badge v-if="getFingerprint(flow)" variant="outline" class="text-xs">
                🔍 Fingerprint
              </Badge>
              <Badge v-if="getRateLimit(flow)" variant="outline" class="text-xs">
                ⏱️ Rate limit
              </Badge>
            </div>

            <div class="flex items-center justify-between pt-1 text-xs text-muted-foreground/70">
              <span>Priority: {{ flow.priority }}</span>
              <span>{{ formatDate(flow.created_at) }}</span>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>

    <!-- Create Dialog -->
    <Dialog v-model:open="showCreateDialog">
      <DialogContent class="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Create Login Flow</DialogTitle>
          <DialogDescription>
            Create a custom flow to customize login for specific users, schemas, or orgs.
            The default flow handles everyone else.
          </DialogDescription>
        </DialogHeader>
        <form @submit.prevent="createFlow" class="space-y-4">
          <div class="space-y-1.5">
            <Label for="flow-name">Name</Label>
            <Input id="flow-name" v-model="newFlow.name" placeholder="e.g. Passkey-First Login" required />
          </div>
          <div class="space-y-1.5">
            <Label for="flow-state">Start state</Label>
            <select id="flow-state" v-model="newFlow.state" class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
              <option value="draft">Draft (not served)</option>
              <option value="testing">Testing (served to user allowlist only)</option>
              <option value="active">Active (served to all matching users)</option>
            </select>
            <p class="text-xs text-muted-foreground">
              Draft → Testing → Active. You can promote flows from the detail view.
            </p>
          </div>
          <div class="space-y-1.5">
            <Label for="flow-preset">Preset</Label>
            <select id="flow-preset" v-model="newFlow.preset" class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
              <option value="identifier_first">Identifier First</option>
              <option value="passkey_first">Passkey First</option>
              <option value="sso_only">SSO Only</option>
            </select>
          </div>
          <div class="space-y-1.5">
            <Label>Captcha Provider</Label>
            <select v-model="newFlow.captcha" class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
              <option value="altcha">Altcha (PoW, self-hosted)</option>
              <option value="none">None</option>
            </select>
          </div>
          <div class="flex items-center gap-2">
            <input type="checkbox" id="fp-enabled" v-model="newFlow.fingerprint" class="accent-primary" />
            <Label for="fp-enabled">Enable browser fingerprinting</Label>
          </div>
          <div class="flex justify-end gap-2 pt-2">
            <Button type="button" variant="outline" @click="showCreateDialog = false">Cancel</Button>
            <Button type="submit" :disabled="creating">
              <Spinner v-if="creating" class="size-4 mr-2" />
              Create
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
/**
 * LoginFlowListView — Lists all login flows with the default flow prominent.
 * The default flow is always shown first with a distinct visual treatment.
 * Custom flows show audience targeting and state badges.
 */
import { ref, computed, onMounted } from 'vue'
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
  preset: string
  is_default: boolean
  enabled: boolean
  state: string
  priority: number
  audience: any
  auth_methods: any
  config: any
  created_at: string
  updated_at: string
}

const flows = ref<LoginFlow[]>([])
const loading = ref(true)
const showCreateDialog = ref(false)
const creating = ref(false)

const defaultFlow = computed(() => flows.value.find(f => f.is_default))
const customFlows = computed(() => flows.value.filter(f => !f.is_default))

const newFlow = ref({
  name: '',
  state: 'draft',
  preset: 'identifier_first',
  captcha: 'altcha',
  fingerprint: true,
})

// Config helpers — read from the config JSON blob.
function getConfig(flow: LoginFlow): any {
  if (!flow.config) return {}
  if (typeof flow.config === 'string') {
    try { return JSON.parse(flow.config) } catch { return {} }
  }
  return flow.config
}
function getCaptcha(flow: LoginFlow): any {
  return getConfig(flow).captcha
}
function getCaptchaProvider(flow: LoginFlow): string {
  return getCaptcha(flow)?.provider || 'altcha'
}
function getFingerprint(flow: LoginFlow): any {
  const fp = getConfig(flow).fingerprint
  return fp?.enabled !== false ? fp : null
}
function getRateLimit(flow: LoginFlow): any {
  return getConfig(flow).rate_limit
}
function getTelemetry(flow: LoginFlow): any {
  const tel = getConfig(flow).telemetry
  return tel?.enabled !== false ? tel : null
}

// Audience helpers.
function hasAudience(flow: LoginFlow): boolean {
  if (!flow.audience) return false
  const a = typeof flow.audience === 'string' ? safeJSON(flow.audience) : flow.audience
  return (a.schema_ids?.length > 0) || (a.user_ids?.length > 0) || (a.org_ids?.length > 0)
}
function audienceSchemaCount(flow: LoginFlow): number {
  const a = typeof flow.audience === 'string' ? safeJSON(flow.audience) : (flow.audience || {})
  return a.schema_ids?.length || 0
}
function audienceUserCount(flow: LoginFlow): number {
  const a = typeof flow.audience === 'string' ? safeJSON(flow.audience) : (flow.audience || {})
  return a.user_ids?.length || 0
}
function audienceOrgCount(flow: LoginFlow): number {
  const a = typeof flow.audience === 'string' ? safeJSON(flow.audience) : (flow.audience || {})
  return a.org_ids?.length || 0
}
function safeJSON(s: string): any {
  try { return JSON.parse(s) } catch { return {} }
}

function stateVariant(state: string): 'default' | 'secondary' | 'outline' | 'destructive' {
  switch (state) {
    case 'active': return 'default'
    case 'testing': return 'secondary'
    case 'archived': return 'destructive'
    default: return 'outline'
  }
}

function formatDate(iso: string): string {
  if (!iso) return ''
  return new Date(iso).toLocaleDateString()
}

async function loadFlows() {
  loading.value = true
  try {
    const resp = await api.get<any>('/v1/login-flows')
    flows.value = resp.items || []
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
      preset: newFlow.value.preset,
      state: newFlow.value.state,
      config: {
        captcha: {
          provider: newFlow.value.captcha,
          mode: newFlow.value.captcha !== 'none' ? 'risk_based' : 'never',
          difficulty: 3,
          steps: ['identifier', 'password'],
        },
        fingerprint: {
          enabled: newFlow.value.fingerprint,
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
      },
    })
    showCreateDialog.value = false
    newFlow.value = { name: '', state: 'draft', preset: 'identifier_first', captcha: 'altcha', fingerprint: true }
    await loadFlows()
  } catch (e: any) {
    console.error('Failed to create login flow:', e)
  } finally {
    creating.value = false
  }
}

onMounted(loadFlows)
</script>
