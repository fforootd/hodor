<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">Login Flows</h1>
        <p class="text-muted-foreground text-sm mt-1">
          Schema-driven login experiences. Each flow defines captcha, fingerprinting, rate limits, and layout behavior.
        </p>
      </div>
      <Button @click="showCreateDialog = true">
        <Plus class="size-4 mr-2" />
        Create Flow
      </Button>
    </div>

    <!-- Flow Cards -->
    <div v-if="loading" class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>

    <div v-else-if="flows.length === 0" class="text-center py-12">
      <div class="text-4xl mb-3">🔐</div>
      <h3 class="text-lg font-semibold">No Login Flows Yet</h3>
      <p class="text-muted-foreground text-sm mt-1 max-w-md mx-auto">
        Create your first login flow to customize the authentication experience.
        Flows define captcha challenges, browser fingerprinting, and layout per org or app.
      </p>
      <Button class="mt-4" @click="showCreateDialog = true">Create Your First Flow</Button>
    </div>

    <div v-else class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
      <Card
        v-for="flow in flows"
        :key="flow.id"
        class="group hover:shadow-md transition-shadow cursor-pointer"
        @click="$router.push(`/login-flows/${flow.id}`)"
      >
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span class="text-xl">🔐</span>
              <CardTitle class="text-base">{{ flow.display_name || flow.identifier }}</CardTitle>
            </div>
            <Badge v-if="isDefault(flow)" variant="secondary" class="text-xs">Default</Badge>
          </div>
        </CardHeader>
        <CardContent class="text-sm text-muted-foreground space-y-2">
          <p v-if="getDescription(flow)" class="line-clamp-2">{{ getDescription(flow) }}</p>

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
            <Badge v-if="getTelemetry(flow)" variant="outline" class="text-xs">
              📊 Telemetry
            </Badge>
          </div>

          <!-- Scope -->
          <div class="flex items-center justify-between pt-1 text-xs text-muted-foreground/70">
            <span>Scope: {{ getScope(flow) }}</span>
            <span>{{ formatDate(flow.created_at) }}</span>
          </div>
        </CardContent>
      </Card>
    </div>

    <!-- Create Dialog -->
    <Dialog v-model:open="showCreateDialog">
      <DialogContent class="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Create Login Flow</DialogTitle>
          <DialogDescription>
            Define a new login flow with captcha, fingerprinting, and rate limiting rules.
          </DialogDescription>
        </DialogHeader>
        <form @submit.prevent="createFlow" class="space-y-4">
          <div class="space-y-1.5">
            <Label for="flow-name">Name</Label>
            <Input id="flow-name" v-model="newFlow.name" placeholder="e.g. Passkey-First Login" required />
          </div>
          <div class="space-y-1.5">
            <Label for="flow-desc">Description</Label>
            <Input id="flow-desc" v-model="newFlow.description" placeholder="What makes this flow unique?" />
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
          <div class="flex items-center gap-2">
            <input type="checkbox" id="tel-enabled" v-model="newFlow.telemetry" class="accent-primary" />
            <Label for="tel-enabled">Enable OTel telemetry</Label>
          </div>
          <div class="flex items-center gap-2">
            <input type="checkbox" id="is-default" v-model="newFlow.isDefault" class="accent-primary" />
            <Label for="is-default">Set as default flow</Label>
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
 * LoginFlowListView — Lists all login flow entities.
 * Login flows are stored as entities (schema_type=login_flow).
 */
import { ref, onMounted } from 'vue'
import { api } from '@/api/client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Plus } from 'lucide-vue-next'

interface FlowEntity {
  id: string
  identifier: string
  display_name: string
  profile: any
  created_at: string
}

const flows = ref<FlowEntity[]>([])
const loading = ref(true)
const showCreateDialog = ref(false)
const creating = ref(false)

const newFlow = ref({
  name: '',
  description: '',
  captcha: 'altcha',
  fingerprint: true,
  telemetry: true,
  isDefault: false,
})

// Helpers to read profile fields safely.
function getProfile(flow: FlowEntity): any {
  return flow.profile || {}
}
function getDescription(flow: FlowEntity): string {
  return getProfile(flow).description || ''
}
function getCaptcha(flow: FlowEntity): any {
  return getProfile(flow).captcha
}
function getCaptchaProvider(flow: FlowEntity): string {
  return getCaptcha(flow)?.provider || 'altcha'
}
function getFingerprint(flow: FlowEntity): any {
  const fp = getProfile(flow).fingerprint
  return fp?.enabled !== false ? fp : null
}
function getRateLimit(flow: FlowEntity): any {
  return getProfile(flow).rate_limit
}
function getTelemetry(flow: FlowEntity): any {
  const tel = getProfile(flow).telemetry
  return tel?.enabled !== false ? tel : null
}
function getScope(flow: FlowEntity): string {
  return getProfile(flow).scope || 'instance'
}
function isDefault(flow: FlowEntity): boolean {
  return getProfile(flow).is_default === true
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
      identifier: newFlow.value.name.toLowerCase().replace(/\s+/g, '_'),
      display_name: newFlow.value.name,
      profile: {
        display_name: newFlow.value.name,
        description: newFlow.value.description,
        is_default: newFlow.value.isDefault,
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
          enabled: newFlow.value.telemetry,
          sample_rate: 1.0,
        },
      },
    })
    showCreateDialog.value = false
    newFlow.value = { name: '', description: '', captcha: 'altcha', fingerprint: true, telemetry: true, isDefault: false }
    await loadFlows()
  } catch (e: any) {
    // TODO: show error toast
    console.error('Failed to create login flow:', e)
  } finally {
    creating.value = false
  }
}

onMounted(loadFlows)
</script>
