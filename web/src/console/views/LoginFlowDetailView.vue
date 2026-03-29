<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="icon" @click="$router.push('/login-flows')">
          <ArrowLeft class="size-4" />
        </Button>
        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ flow?.display_name || 'Login Flow' }}</h1>
          <p class="text-muted-foreground text-sm mt-0.5">{{ getProfile().description || 'Configure login experience' }}</p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <Badge v-if="getProfile().is_default" variant="secondary">Default</Badge>
        <Badge variant="outline">v{{ flow?.profile?.version || 1 }}</Badge>
        <Button @click="saveFlow" :disabled="saving">
          <Spinner v-if="saving" class="size-4 mr-2" />
          Save
        </Button>
      </div>
    </div>

    <!-- Two-column layout: Editor + Preview -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Left: Configuration Editor -->
      <div class="space-y-6">
        <!-- General -->
        <Card>
          <CardHeader>
            <CardTitle class="text-sm font-medium">General</CardTitle>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="space-y-1.5">
              <Label for="name">Name</Label>
              <Input id="name" v-model="form.name" />
            </div>
            <div class="space-y-1.5">
              <Label for="desc">Description</Label>
              <Input id="desc" v-model="form.description" />
            </div>
            <div class="flex items-center gap-2">
              <input type="checkbox" id="default" v-model="form.isDefault" class="accent-primary" />
              <Label for="default">Default flow for this scope</Label>
            </div>
          </CardContent>
        </Card>

        <!-- Captcha -->
        <Card>
          <CardHeader>
            <div class="flex items-center justify-between">
              <CardTitle class="text-sm font-medium">🛡️ Captcha</CardTitle>
              <Badge :variant="form.captcha.mode !== 'never' ? 'default' : 'outline'" class="text-xs">
                {{ form.captcha.mode !== 'never' ? 'Active' : 'Disabled' }}
              </Badge>
            </div>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="space-y-1.5">
              <Label>Provider</Label>
              <select v-model="form.captcha.provider" class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                <option value="altcha">Altcha (PoW, self-hosted)</option>
                <option value="hcaptcha">hCaptcha</option>
                <option value="recaptcha">reCAPTCHA</option>
                <option value="turnstile">Cloudflare Turnstile</option>
                <option value="none">None</option>
              </select>
            </div>
            <div class="space-y-1.5">
              <Label>Mode</Label>
              <select v-model="form.captcha.mode" class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                <option value="always">Always show</option>
                <option value="risk_based">Risk-based (show when suspicious)</option>
                <option value="never">Disabled</option>
              </select>
            </div>
            <div v-if="form.captcha.provider === 'altcha'" class="space-y-1.5">
              <Label>Difficulty (1-5)</Label>
              <div class="flex items-center gap-3">
                <input type="range" v-model.number="form.captcha.difficulty" min="1" max="5" class="flex-1" />
                <span class="text-sm font-mono w-6 text-center">{{ form.captcha.difficulty }}</span>
              </div>
              <p class="text-xs text-muted-foreground">Higher = more PoW work required. 3 is recommended.</p>
            </div>
          </CardContent>
        </Card>

        <!-- Fingerprint -->
        <Card>
          <CardHeader>
            <div class="flex items-center justify-between">
              <CardTitle class="text-sm font-medium">🔍 Browser Fingerprinting</CardTitle>
              <Badge :variant="form.fingerprint.enabled ? 'default' : 'outline'" class="text-xs">
                {{ form.fingerprint.enabled ? 'Active' : 'Disabled' }}
              </Badge>
            </div>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="flex items-center gap-2">
              <input type="checkbox" id="fp-on" v-model="form.fingerprint.enabled" class="accent-primary" />
              <Label for="fp-on">Enable fingerprinting</Label>
            </div>
            <div v-if="form.fingerprint.enabled" class="space-y-1.5">
              <Label>Provider</Label>
              <select v-model="form.fingerprint.provider" class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                <option value="thumbmarkjs">ThumbmarkJS (recommended)</option>
                <option value="built_in">Built-in (canvas + WebGL)</option>
              </select>
            </div>
            <div v-if="form.fingerprint.enabled" class="flex items-center gap-2">
              <input type="checkbox" id="fp-persist" v-model="form.fingerprint.persist" class="accent-primary" />
              <Label for="fp-persist">Persist across sessions (returning-user detection)</Label>
            </div>
          </CardContent>
        </Card>

        <!-- Rate Limiting -->
        <Card>
          <CardHeader>
            <CardTitle class="text-sm font-medium">⏱️ Rate Limiting</CardTitle>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-1.5">
                <Label for="max-attempts">Max attempts</Label>
                <Input id="max-attempts" type="number" v-model.number="form.rateLimit.maxAttempts" min="1" max="100" />
              </div>
              <div class="space-y-1.5">
                <Label for="window">Window (seconds)</Label>
                <Input id="window" type="number" v-model.number="form.rateLimit.windowSeconds" min="60" max="3600" />
              </div>
            </div>
            <div class="space-y-1.5">
              <Label for="lockout">Lockout (seconds)</Label>
              <Input id="lockout" type="number" v-model.number="form.rateLimit.lockoutSeconds" min="0" max="86400" />
            </div>
            <div class="space-y-1.5">
              <Label>Scope</Label>
              <select v-model="form.rateLimit.scope" class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                <option value="ip">Per IP</option>
                <option value="identifier">Per identifier</option>
                <option value="fingerprint">Per fingerprint</option>
              </select>
            </div>
          </CardContent>
        </Card>

        <!-- Telemetry -->
        <Card>
          <CardHeader>
            <div class="flex items-center justify-between">
              <CardTitle class="text-sm font-medium">📊 Telemetry</CardTitle>
              <Badge :variant="form.telemetry.enabled ? 'default' : 'outline'" class="text-xs">
                {{ form.telemetry.enabled ? 'Active' : 'Disabled' }}
              </Badge>
            </div>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="flex items-center gap-2">
              <input type="checkbox" id="tel-on" v-model="form.telemetry.enabled" class="accent-primary" />
              <Label for="tel-on">Collect browser telemetry</Label>
            </div>
            <div v-if="form.telemetry.enabled" class="space-y-1.5">
              <Label>Sample rate</Label>
              <div class="flex items-center gap-3">
                <input type="range" v-model.number="form.telemetry.sampleRate" min="0" max="1" step="0.1" class="flex-1" />
                <span class="text-sm font-mono w-10 text-right">{{ Math.round(form.telemetry.sampleRate * 100) }}%</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <!-- Right: Live Preview -->
      <div class="space-y-4">
        <Card class="sticky top-6">
          <CardHeader>
            <div class="flex items-center justify-between">
              <CardTitle class="text-sm font-medium">Live Preview</CardTitle>
              <div class="flex items-center gap-2">
                <button
                  v-for="layout in layouts"
                  :key="layout.id"
                  class="text-xs px-2 py-1 rounded transition-colors"
                  :class="previewLayout === layout.id ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'"
                  @click="previewLayout = layout.id"
                >{{ layout.label }}</button>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <!-- Simulated login form preview -->
            <div class="rounded-lg border bg-background p-6 space-y-4 min-h-[400px]">
              <div class="text-center space-y-1">
                <div class="text-xl font-bold">🔐 Acme Corp</div>
                <p class="text-sm text-muted-foreground">Sign in to your account</p>
              </div>

              <div class="space-y-3">
                <div class="space-y-1.5">
                  <Label class="text-xs">Email</Label>
                  <Input placeholder="name@example.com" disabled class="bg-muted/30" />
                </div>

                <div class="space-y-1.5">
                  <Label class="text-xs">Password</Label>
                  <Input type="password" placeholder="••••••••" disabled class="bg-muted/30" />
                </div>

                <!-- Captcha preview -->
                <div v-if="form.captcha.mode !== 'never'" class="rounded-md border border-input px-3 py-2 flex items-center gap-2">
                  <div class="size-4 rounded border-2 border-muted-foreground/30" />
                  <span class="text-xs text-muted-foreground">
                    I am human
                    <span class="text-muted-foreground/50">({{ form.captcha.provider }})</span>
                  </span>
                </div>

                <Button class="w-full" disabled>Sign in</Button>

                <!-- Telemetry indicator -->
                <div v-if="form.telemetry.enabled" class="flex items-center gap-1.5 text-xs text-muted-foreground/50">
                  <div class="size-1.5 rounded-full bg-green-400 animate-pulse" />
                  Telemetry active ({{ Math.round(form.telemetry.sampleRate * 100) }}% sample rate)
                </div>

                <!-- Fingerprint indicator -->
                <div v-if="form.fingerprint.enabled" class="flex items-center gap-1.5 text-xs text-muted-foreground/50">
                  <div class="size-1.5 rounded-full bg-blue-400" />
                  Fingerprint: {{ form.fingerprint.provider }}
                </div>

                <!-- Rate limit indicator -->
                <div class="flex items-center gap-1.5 text-xs text-muted-foreground/50">
                  <div class="size-1.5 rounded-full bg-orange-400" />
                  Rate limit: {{ form.rateLimit.maxAttempts }} attempts / {{ form.rateLimit.windowSeconds }}s (per {{ form.rateLimit.scope }})
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * LoginFlowDetailView — Edit a login flow entity with a live preview.
 */
import { ref, reactive, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '@/api/client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { ArrowLeft } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

interface FlowEntity {
  id: string
  identifier: string
  display_name: string
  profile: any
  created_at: string
  updated_at: string
}

const flow = ref<FlowEntity | null>(null)
const saving = ref(false)
const previewLayout = ref('centered')

const layouts = [
  { id: 'centered', label: 'Centered' },
  { id: 'split', label: 'Split' },
  { id: 'card_image', label: 'Card' },
  { id: 'minimal', label: 'Minimal' },
]

const form = reactive({
  name: '',
  description: '',
  isDefault: false,
  captcha: {
    provider: 'altcha',
    mode: 'risk_based',
    difficulty: 3,
  },
  fingerprint: {
    enabled: true,
    provider: 'thumbmarkjs',
    persist: true,
  },
  rateLimit: {
    maxAttempts: 5,
    windowSeconds: 300,
    lockoutSeconds: 900,
    scope: 'ip',
  },
  telemetry: {
    enabled: true,
    sampleRate: 1.0,
  },
})

function getProfile(): any {
  return flow.value?.profile || {}
}

function populateForm(entity: FlowEntity) {
  const p = entity.profile || {}
  form.name = entity.display_name || p.display_name || ''
  form.description = p.description || ''
  form.isDefault = p.is_default || false

  if (p.captcha) {
    form.captcha.provider = p.captcha.provider || 'altcha'
    form.captcha.mode = p.captcha.mode || 'risk_based'
    form.captcha.difficulty = p.captcha.difficulty || 3
  }
  if (p.fingerprint) {
    form.fingerprint.enabled = p.fingerprint.enabled !== false
    form.fingerprint.provider = p.fingerprint.provider || 'thumbmarkjs'
    form.fingerprint.persist = p.fingerprint.persist !== false
  }
  if (p.rate_limit) {
    form.rateLimit.maxAttempts = p.rate_limit.max_attempts || 5
    form.rateLimit.windowSeconds = p.rate_limit.window_seconds || 300
    form.rateLimit.lockoutSeconds = p.rate_limit.lockout_seconds || 900
    form.rateLimit.scope = p.rate_limit.scope || 'ip'
  }
  if (p.telemetry) {
    form.telemetry.enabled = p.telemetry.enabled !== false
    form.telemetry.sampleRate = p.telemetry.sample_rate ?? 1.0
  }
}

async function loadFlow() {
  const id = route.params.id as string
  try {
    const resp = await api.get<FlowEntity>(`/v1/login-flows/${id}`)
    flow.value = resp
    populateForm(resp)
  } catch {
    router.push('/login-flows')
  }
}

async function saveFlow() {
  if (!flow.value) return
  saving.value = true
  try {
    await api.patch(`/v1/login-flows/${flow.value.id}`, {
      display_name: form.name,
      profile: {
        display_name: form.name,
        description: form.description,
        is_default: form.isDefault,
        captcha: {
          provider: form.captcha.provider,
          mode: form.captcha.mode,
          difficulty: form.captcha.difficulty,
          steps: ['identifier', 'password'],
        },
        fingerprint: {
          enabled: form.fingerprint.enabled,
          provider: form.fingerprint.provider,
          persist: form.fingerprint.persist,
          steps: ['identifier'],
        },
        rate_limit: {
          max_attempts: form.rateLimit.maxAttempts,
          window_seconds: form.rateLimit.windowSeconds,
          lockout_seconds: form.rateLimit.lockoutSeconds,
          scope: form.rateLimit.scope,
        },
        telemetry: {
          enabled: form.telemetry.enabled,
          sample_rate: form.telemetry.sampleRate,
        },
        branding: {
          layout: previewLayout.value,
        },
      },
    })
    await loadFlow()
  } catch (e: any) {
    console.error('Failed to save login flow:', e)
  } finally {
    saving.value = false
  }
}

onMounted(loadFlow)
</script>
