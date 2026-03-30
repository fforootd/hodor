<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="icon" @click="$router.push('/login-flows')">
          <ArrowLeft class="size-4" />
        </Button>
        <div>
          <div class="flex items-center gap-2">
            <h1 class="text-2xl font-bold tracking-tight">{{ flow?.name || 'Login Flow' }}</h1>
            <Badge v-if="flow?.is_default" variant="default">Default</Badge>
            <Badge :variant="stateVariant(flow?.state)" class="text-xs">{{ flow?.state || 'draft' }}</Badge>
          </div>
          <p class="text-muted-foreground text-sm mt-0.5">
            <template v-if="flow?.is_default">
              Applies to all users not matched by a specific flow
            </template>
            <template v-else>
              Configure strategy, protections, and layout for this login flow
            </template>
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <Button
          v-if="flow && flow.state !== 'active' && flow.state !== 'archived'"
          variant="outline"
          :disabled="promoting"
          @click="promoteFlow"
        >
          <Spinner v-if="promoting" class="size-4 mr-2" />
          <ArrowUp v-else class="size-4 mr-2" />
          Promote to {{ flow?.state === 'draft' ? 'Testing' : 'Active' }}
        </Button>
        <Button :disabled="saving" @click="saveFlow">
          <Spinner v-if="saving" class="size-4 mr-2" />
          Save
        </Button>
      </div>
    </div>

    <div
      v-if="flow?.is_default"
      class="rounded-lg border border-primary/20 bg-primary/[0.03] p-4 flex items-start gap-3"
    >
      <Shield class="size-5 text-primary mt-0.5 shrink-0" />
      <div>
        <p class="text-sm font-medium">This is your instance default login flow</p>
        <p class="text-xs text-muted-foreground mt-0.5">
          All users who don't match a more specific flow will see this configuration.
          Changes here affect every login that isn't handled by a targeted flow.
        </p>
      </div>
    </div>

    <Card v-if="templateSource">
      <CardContent class="py-4 flex items-center justify-between gap-4">
        <div>
          <p class="text-sm font-medium">Template source</p>
          <p class="text-xs text-muted-foreground mt-0.5">
            This flow was installed from <span class="font-mono">{{ templateSource }}</span>.
          </p>
        </div>
        <Badge variant="secondary">Template-backed</Badge>
      </CardContent>
    </Card>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <div class="space-y-6">
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
              <Label for="priority">Priority</Label>
              <Input id="priority" v-model.number="form.priority" type="number" min="0" max="1000" />
              <p class="text-xs text-muted-foreground">Higher priority flows are evaluated first.</p>
            </div>
            <div class="space-y-1.5">
              <Label for="strategy">Flow Strategy</Label>
              <select
                id="strategy"
                v-model="form.strategy"
                class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="identifier_first">Identifier first</option>
                <option value="passkey_first">Passkey first</option>
                <option value="sso_only">SSO only</option>
                <option value="custom">Custom</option>
              </select>
              <p class="text-xs text-muted-foreground">
                Strategy controls how the flow starts and branches. For complete starting points,
                use Marketplace templates.
              </p>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle class="text-sm font-medium">Branding</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div
              v-for="assetField in brandingAssetFields"
              :key="assetField.key"
              class="rounded-lg border p-3 space-y-3"
            >
              <div class="flex items-start justify-between gap-3">
                <div>
                  <p class="text-sm font-medium">{{ assetField.label }}</p>
                  <p class="text-xs text-muted-foreground">{{ assetField.description }}</p>
                </div>
                <Button
                  v-if="form.branding[assetField.key]"
                  type="button"
                  size="sm"
                  variant="ghost"
                  :disabled="assetBusy[assetField.key]"
                  @click="removeBrandingAsset(assetField.key)"
                >
                  Remove
                </Button>
              </div>

              <div v-if="form.branding[assetField.key]" class="rounded-md border bg-muted/20 p-2">
                <img
                  :src="form.branding[assetField.key]"
                  :alt="assetField.label"
                  class="max-h-16 max-w-full rounded object-contain"
                />
              </div>

              <div class="space-y-1.5">
                <Label :for="`upload-${assetField.key}`">Upload file</Label>
                <input
                  :id="`upload-${assetField.key}`"
                  type="file"
                  accept="image/*,.svg"
                  class="block w-full text-sm"
                  :disabled="assetBusy[assetField.key]"
                  @change="onBrandingFileSelected(assetField.key, $event)"
                />
              </div>

              <div class="grid gap-2 sm:grid-cols-[1fr_auto]">
                <Input
                  v-model="assetImportUrls[assetField.key]"
                  :placeholder="assetField.placeholder"
                  :disabled="assetBusy[assetField.key]"
                />
                <Button
                  type="button"
                  variant="outline"
                  :disabled="assetBusy[assetField.key] || !assetImportUrls[assetField.key]"
                  @click="importBrandingAsset(assetField.key)"
                >
                  {{ assetBusy[assetField.key] ? 'Importing…' : 'Import URL' }}
                </Button>
              </div>

              <p
                v-if="assetField.key === 'cover_image' && !['split', 'card_image'].includes(form.layout)"
                class="text-xs text-muted-foreground"
              >
                Cover images appear in the shared preview when the layout is Split or Card with image.
              </p>
            </div>

            <div class="flex items-center gap-2">
              <input
                id="hide-powered-by"
                v-model="form.branding.hide_zitadel_branding"
                type="checkbox"
                class="accent-primary"
              />
              <Label for="hide-powered-by">Hide “Powered by Zitadel” footer</Label>
            </div>
          </CardContent>
        </Card>

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
              <select
                v-model="form.captcha.provider"
                class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="altcha">Altcha (PoW, self-hosted)</option>
                <option value="hcaptcha">hCaptcha</option>
                <option value="recaptcha">reCAPTCHA</option>
                <option value="turnstile">Cloudflare Turnstile</option>
                <option value="none">None</option>
              </select>
            </div>
            <div class="space-y-1.5">
              <Label>Mode</Label>
              <select
                v-model="form.captcha.mode"
                class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="always">Always show</option>
                <option value="risk_based">Risk-based (show when suspicious)</option>
                <option value="never">Disabled</option>
              </select>
            </div>
            <div v-if="form.captcha.provider === 'altcha'" class="space-y-1.5">
              <Label>Difficulty (1-5)</Label>
              <div class="flex items-center gap-3">
                <input v-model.number="form.captcha.difficulty" type="range" min="1" max="5" class="flex-1" />
                <span class="text-sm font-mono w-6 text-center">{{ form.captcha.difficulty }}</span>
              </div>
              <p class="text-xs text-muted-foreground">Higher = more PoW work required. 3 is recommended.</p>
            </div>
          </CardContent>
        </Card>

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
              <input id="fp-on" v-model="form.fingerprint.enabled" type="checkbox" class="accent-primary" />
              <Label for="fp-on">Enable fingerprinting</Label>
            </div>
            <div v-if="form.fingerprint.enabled" class="space-y-1.5">
              <Label>Provider</Label>
              <select
                v-model="form.fingerprint.provider"
                class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="thumbmarkjs">ThumbmarkJS (recommended)</option>
                <option value="built_in">Built-in (canvas + WebGL)</option>
              </select>
            </div>
            <div v-if="form.fingerprint.enabled" class="flex items-center gap-2">
              <input id="fp-persist" v-model="form.fingerprint.persist" type="checkbox" class="accent-primary" />
              <Label for="fp-persist">Persist across sessions (returning-user detection)</Label>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle class="text-sm font-medium">⏱️ Rate Limiting</CardTitle>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-1.5">
                <Label for="max-attempts">Max attempts</Label>
                <Input id="max-attempts" v-model.number="form.rateLimit.maxAttempts" type="number" min="1" max="100" />
              </div>
              <div class="space-y-1.5">
                <Label for="window">Window (seconds)</Label>
                <Input id="window" v-model.number="form.rateLimit.windowSeconds" type="number" min="60" max="3600" />
              </div>
            </div>
            <div class="space-y-1.5">
              <Label for="lockout">Lockout (seconds)</Label>
              <Input id="lockout" v-model.number="form.rateLimit.lockoutSeconds" type="number" min="0" max="86400" />
            </div>
            <div class="space-y-1.5">
              <Label>Scope</Label>
              <select
                v-model="form.rateLimit.scope"
                class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="ip">Per IP</option>
                <option value="identifier">Per identifier</option>
                <option value="fingerprint">Per fingerprint</option>
              </select>
            </div>
          </CardContent>
        </Card>

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
              <input id="tel-on" v-model="form.telemetry.enabled" type="checkbox" class="accent-primary" />
              <Label for="tel-on">Collect browser telemetry</Label>
            </div>
            <div v-if="form.telemetry.enabled" class="space-y-1.5">
              <Label>Sample rate</Label>
              <div class="flex items-center gap-3">
                <input
                  v-model.number="form.telemetry.sampleRate"
                  type="range"
                  min="0"
                  max="1"
                  step="0.1"
                  class="flex-1"
                />
                <span class="text-sm font-mono w-10 text-right">{{ Math.round(form.telemetry.sampleRate * 100) }}%</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <div class="space-y-4">
        <Card class="sticky top-6">
          <CardHeader class="space-y-4">
            <div>
              <CardTitle class="text-sm font-medium">Shared Preview</CardTitle>
              <p class="text-xs text-muted-foreground mt-1">
                Uses the same Vue renderer and layout shells as the hosted login page and the
                published web component.
              </p>
            </div>

            <div class="space-y-2">
              <Label>Layout</Label>
              <div class="flex items-center gap-2 flex-wrap">
                <button
                  v-for="layout in layouts"
                  :key="layout.id"
                  class="text-xs px-2.5 py-1.5 rounded-md border transition-colors"
                  :class="form.layout === layout.id ? 'bg-primary text-primary-foreground border-primary' : 'bg-background hover:bg-accent border-border'"
                  @click="form.layout = layout.id"
                >
                  {{ layout.label }}
                </button>
              </div>
              <p class="text-xs text-muted-foreground">
                Layout controls the visual shell only. Customers embedding
                <code>&lt;zitadel-login&gt;</code>
                can still override layout with web component attributes.
              </p>
            </div>
          </CardHeader>

          <CardContent class="space-y-4">
            <div class="rounded-xl border bg-muted/20 overflow-hidden">
              <LoginShell :branding="previewBranding" preview>
                <LoginNodeRenderer
                  :flow-step="previewStep"
                  :preview="true"
                  :form-data="previewFormData"
                  :confirm-passwords="previewConfirmPasswords"
                />
              </LoginShell>
            </div>

            <div class="grid grid-cols-1 gap-2 text-xs text-muted-foreground">
              <div v-if="form.telemetry.enabled" class="flex items-center gap-2">
                <div class="size-1.5 rounded-full bg-green-500" />
                Telemetry active ({{ Math.round(form.telemetry.sampleRate * 100) }}% sample rate)
              </div>
              <div v-if="form.fingerprint.enabled" class="flex items-center gap-2">
                <div class="size-1.5 rounded-full bg-blue-500" />
                Fingerprint provider: {{ form.fingerprint.provider }}
              </div>
              <div class="flex items-center gap-2">
                <div class="size-1.5 rounded-full bg-orange-500" />
                Rate limit: {{ form.rateLimit.maxAttempts }} attempts / {{ form.rateLimit.windowSeconds }}s (per {{ form.rateLimit.scope }})
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '@/api/client'
import type { FlowBranding } from '@/api/branding'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { ArrowLeft, ArrowUp, Shield } from 'lucide-vue-next'
import LoginShell from '@/login/components/LoginShell.vue'
import LoginNodeRenderer from '@/login/components/LoginNodeRenderer.vue'
import { buildPreviewFlowStep } from '@/login/preview'

const route = useRoute()
const router = useRouter()

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

type BrandingAssetField = 'logo_url' | 'logo_dark' | 'cover_image' | 'favicon'

const flow = ref<LoginFlow | null>(null)
const currentConfig = ref<Record<string, any>>({})
const saving = ref(false)
const promoting = ref(false)
const previewFormData = reactive<Record<string, any>>({})
const previewConfirmPasswords = reactive<Record<string, string>>({})
const assetImportUrls = reactive<Record<string, string>>({
  logo_url: '',
  logo_dark: '',
  cover_image: '',
  favicon: '',
})
const assetBusy = reactive<Record<string, boolean>>({
  logo_url: false,
  logo_dark: false,
  cover_image: false,
  favicon: false,
})

const brandingAssetFields = [
  {
    key: 'logo_url',
    label: 'Logo',
    description: 'Used in the login card header on light backgrounds.',
    placeholder: 'https://example.com/logo.svg',
  },
  {
    key: 'logo_dark',
    label: 'Dark Logo',
    description: 'Used when dark mode is active and a dark-specific logo is available.',
    placeholder: 'https://example.com/logo-dark.svg',
  },
  {
    key: 'cover_image',
    label: 'Cover Image',
    description: 'Used by Split and Card with image layouts.',
    placeholder: 'https://example.com/cover.jpg',
  },
  {
    key: 'favicon',
    label: 'Favicon',
    description: 'Shown in the browser tab for the hosted login page.',
    placeholder: 'https://example.com/favicon.png',
  },
] as const

const layouts = [
  { id: 'centered', label: 'Centered' },
  { id: 'split', label: 'Split' },
  { id: 'muted', label: 'Muted' },
  { id: 'card_image', label: 'Card with image' },
  { id: 'minimal', label: 'Minimal' },
]

const form = reactive({
  name: '',
  priority: 0,
  strategy: 'identifier_first',
  layout: 'centered',
  branding: {
    logo_url: '',
    logo_dark: '',
    cover_image: '',
    favicon: '',
    hide_zitadel_branding: false,
  },
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

const templateSource = computed(() => {
  const catalog = flow.value?.metadata?._catalog || flow.value?.metadata?.catalog || null
  return catalog?.template_id || null
})

const previewBranding = computed<FlowBranding>(() => {
  const branding = currentConfig.value.branding || {}
  return {
    heading: branding.heading || 'Welcome back',
    description: branding.description || 'Sign in to your account',
    logo_url: form.branding.logo_url || '',
    org_name: branding.org_name || 'Acme Corp',
    colors: {
      primary: '#6366f1',
      primary_foreground: '#ffffff',
      background: '#f0f2ff',
      surface: '#ffffff',
      text: '#1a1a2e',
      muted: '#f4f4f5',
      accent: '#6366f1',
      border: '#e4e4e7',
      error: '#ef4444',
      ...(branding.colors || {}),
    },
    font_family: branding.font_family || 'Inter, system-ui, sans-serif',
    font_url: branding.font_url || '',
    texts: branding.texts || {},
    custom_css: branding.custom_css || '',
    hide_zitadel_branding: form.branding.hide_zitadel_branding,
    layout: form.layout,
    dark_mode: branding.dark_mode || 'light',
    cover_image: form.branding.cover_image || '',
    logo_dark: form.branding.logo_dark || '',
    favicon: form.branding.favicon || '',
    border_radius: branding.border_radius || 'md',
    terms_url: branding.terms_url || '',
    privacy_url: branding.privacy_url || '',
    social_position: branding.social_position || 'bottom',
    consent: branding.consent || [],
  }
})

const previewStep = computed(() =>
  buildPreviewFlowStep({
    strategy: form.strategy,
    branding: previewBranding.value,
    captchaEnabled: form.captcha.mode !== 'never' && form.captcha.provider !== 'none',
    captchaProvider: form.captcha.provider,
  }),
)

function stateVariant(state?: string): 'default' | 'secondary' | 'outline' | 'destructive' {
  switch (state) {
    case 'active': return 'default'
    case 'testing': return 'secondary'
    case 'archived': return 'destructive'
    default: return 'outline'
  }
}

function safeJSON(value: unknown): Record<string, any> {
  if (!value) return {}
  if (typeof value === 'string') {
    try { return JSON.parse(value) } catch { return {} }
  }
  return typeof value === 'object' ? value as Record<string, any> : {}
}

function populateForm(f: LoginFlow) {
  form.name = f.name || ''
  form.priority = f.priority || 0
  form.strategy = f.strategy || 'identifier_first'

  const config = safeJSON(f.config)
  currentConfig.value = config

  form.layout = config.branding?.layout || 'centered'
  form.branding.logo_url = config.branding?.logo_url || ''
  form.branding.logo_dark = config.branding?.logo_dark || ''
  form.branding.cover_image = config.branding?.cover_image || ''
  form.branding.favicon = config.branding?.favicon || ''
  form.branding.hide_zitadel_branding = config.branding?.hide_zitadel_branding ?? false

  if (config.captcha) {
    form.captcha.provider = config.captcha.provider || 'altcha'
    form.captcha.mode = config.captcha.mode || 'risk_based'
    form.captcha.difficulty = config.captcha.difficulty || 3
  }
  if (config.fingerprint) {
    form.fingerprint.enabled = config.fingerprint.enabled !== false
    form.fingerprint.provider = config.fingerprint.provider || 'thumbmarkjs'
    form.fingerprint.persist = config.fingerprint.persist !== false
  }
  if (config.rate_limit) {
    form.rateLimit.maxAttempts = config.rate_limit.max_attempts || 5
    form.rateLimit.windowSeconds = config.rate_limit.window_seconds || 300
    form.rateLimit.lockoutSeconds = config.rate_limit.lockout_seconds || 900
    form.rateLimit.scope = config.rate_limit.scope || 'ip'
  }
  if (config.telemetry) {
    form.telemetry.enabled = config.telemetry.enabled !== false
    form.telemetry.sampleRate = config.telemetry.sample_rate ?? 1.0
  }
}

async function loadFlow() {
  const id = route.params.id as string
  try {
    const resp = await api.get<LoginFlow>(`/v1/login-flows/${id}`)
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
    const nextConfig = {
      ...currentConfig.value,
      captcha: {
        ...(currentConfig.value.captcha || {}),
        provider: form.captcha.provider,
        mode: form.captcha.mode,
        difficulty: form.captcha.difficulty,
        on: ['login'],
      },
      fingerprint: {
        ...(currentConfig.value.fingerprint || {}),
        enabled: form.fingerprint.enabled,
        provider: form.fingerprint.provider,
        persist: form.fingerprint.persist,
        on: ['login'],
      },
      rate_limit: {
        ...(currentConfig.value.rate_limit || {}),
        max_attempts: form.rateLimit.maxAttempts,
        window_seconds: form.rateLimit.windowSeconds,
        lockout_seconds: form.rateLimit.lockoutSeconds,
        scope: form.rateLimit.scope,
      },
      telemetry: {
        ...(currentConfig.value.telemetry || {}),
        enabled: form.telemetry.enabled,
        sample_rate: form.telemetry.sampleRate,
      },
      branding: {
        ...(currentConfig.value.branding || {}),
        layout: form.layout,
        logo_url: form.branding.logo_url,
        logo_dark: form.branding.logo_dark,
        cover_image: form.branding.cover_image,
        favicon: form.branding.favicon,
        hide_zitadel_branding: form.branding.hide_zitadel_branding,
      },
    }

    await api.patch(`/v1/login-flows/${flow.value.id}`, {
      name: form.name,
      strategy: form.strategy,
      priority: form.priority,
      is_default: flow.value.is_default,
      config: nextConfig,
    })
    await loadFlow()
  } catch (e: any) {
    console.error('Failed to save login flow:', e)
  } finally {
    saving.value = false
  }
}

function extractAssetId(url: string): string {
  if (!url) return ''
  try {
    const parsed = new URL(url, window.location.origin)
    const match = parsed.pathname.match(/\/assets\/login\/([^/]+)$/)
    return match?.[1] || ''
  } catch {
    return ''
  }
}

async function onBrandingFileSelected(field: BrandingAssetField, event: Event) {
  if (!flow.value) return
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return

  const body = new FormData()
  body.append('slot', field)
  body.append('file', file)

  assetBusy[field] = true
  try {
    const resp = await api.postForm<{ url: string }>(`/v1/login-flows/${flow.value.id}/assets`, body)
    form.branding[field] = resp.url
  } catch (e) {
    console.error(`Failed to upload ${field}:`, e)
  } finally {
    assetBusy[field] = false
    input.value = ''
  }
}

async function importBrandingAsset(field: BrandingAssetField) {
  if (!flow.value || !assetImportUrls[field]) return
  assetBusy[field] = true
  try {
    const resp = await api.post<{ url: string }>(`/v1/login-flows/${flow.value.id}/assets/import`, {
      slot: field,
      url: assetImportUrls[field],
    })
    form.branding[field] = resp.url
    assetImportUrls[field] = ''
  } catch (e) {
    console.error(`Failed to import ${field}:`, e)
  } finally {
    assetBusy[field] = false
  }
}

async function removeBrandingAsset(field: BrandingAssetField) {
  if (!flow.value) return
  const assetID = extractAssetId(form.branding[field])
  assetBusy[field] = true
  try {
    if (assetID) {
      await api.delete(`/v1/login-flows/${flow.value.id}/assets/${assetID}`)
    }
    form.branding[field] = ''
  } catch (e) {
    console.error(`Failed to remove ${field}:`, e)
  } finally {
    assetBusy[field] = false
  }
}

async function promoteFlow() {
  if (!flow.value) return
  promoting.value = true
  try {
    await api.post(`/v1/login-flows/${flow.value.id}/promote`, {})
    await loadFlow()
  } catch (e: any) {
    console.error('Failed to promote flow:', e)
  } finally {
    promoting.value = false
  }
}

onMounted(loadFlow)
</script>
