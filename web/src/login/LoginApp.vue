<template>
  <component :is="layoutComponent" :branding="branding">
    <!-- Custom font (if specified by branding) -->
    <link v-if="branding?.font_url" rel="stylesheet" :href="branding.font_url" />

    <!-- Custom CSS injection -->
    <component v-if="branding?.custom_css" :is="'style'" v-text="branding.custom_css" />

    <Card class="w-full" :class="cardClass">
      <CardHeader class="text-center">
        <!-- Logo -->
        <div v-if="effectiveLogo" class="flex justify-center mb-2">
          <img :src="effectiveLogo" :alt="branding?.org_name" class="h-8" />
        </div>
        <div v-else class="text-xl font-bold tracking-tight mb-2">{{ branding?.org_name || 'Zitadel' }}</div>
      </CardHeader>

      <CardContent>
        <!-- Global errors from flow engine -->
        <Alert v-if="error" variant="destructive" class="mb-4">
          <AlertCircle class="size-4" />
          <AlertDescription>{{ error }}</AlertDescription>
        </Alert>
        <template v-if="flowStep?.errors?.length">
          <Alert v-for="(err, i) in flowStep.errors" :key="'ge-'+i" variant="destructive" class="mb-4">
            <AlertCircle class="size-4" />
            <AlertDescription>{{ err.message }}</AlertDescription>
          </Alert>
        </template>

        <!-- Global messages from flow engine -->
        <template v-if="flowStep?.messages?.length">
          <Alert v-for="(msg, i) in flowStep.messages" :key="'gm-'+i"
            :class="{ 'mb-4': true, 'border-green-200 bg-green-50 text-green-800': msg.type === 'success', 'border-yellow-200 bg-yellow-50 text-yellow-800': msg.type === 'warning' }">
            <AlertDescription>{{ msg.text }}</AlertDescription>
          </Alert>
        </template>

        <!-- Loading -->
        <div v-if="!flowStep" class="flex justify-center py-8">
          <Spinner class="size-6" />
        </div>

        <!-- Node Renderer -->
        <form v-else @submit.prevent="onSubmit" class="space-y-4">
          <template v-for="(node, i) in flowStep.nodes" :key="i">
            <!-- Heading -->
            <h1 v-if="node.type === 'heading'" class="text-xl font-semibold text-center">{{ node.text }}</h1>

            <!-- Description -->
            <p v-else-if="node.type === 'description'" class="text-sm text-muted-foreground text-center">{{ node.text }}</p>

            <!-- Avatar -->
            <div v-else-if="node.type === 'avatar'" class="flex flex-col items-center gap-1">
              <Avatar class="size-10">
                <AvatarFallback>{{ node.initial }}</AvatarFallback>
              </Avatar>
              <span v-if="node.text" class="text-sm text-muted-foreground">{{ node.text }}</span>
            </div>

            <!-- Icon -->
            <div v-else-if="node.type === 'icon'" class="text-center text-3xl">{{ node.text }}</div>

            <!-- Info -->
            <Alert v-else-if="node.type === 'info'" class="text-sm">
              <AlertDescription>{{ node.text }}</AlertDescription>
            </Alert>

            <!-- Error (inline) -->
            <Alert v-else-if="node.type === 'error'" variant="destructive" class="text-sm">
              <AlertCircle class="size-4" />
              <AlertDescription>{{ node.text }}</AlertDescription>
            </Alert>

            <!-- Spinner -->
            <div v-else-if="node.type === 'spinner'" class="flex justify-center py-4">
              <Spinner class="size-6" />
            </div>

            <!-- Input -->
            <div v-else-if="node.type === 'input'" class="space-y-1.5">
              <div v-if="node.input_type === 'password'" class="flex items-center justify-between">
                <Label :for="node.name">{{ node.label }}</Label>
              </div>
              <Label v-else :for="node.name">{{ node.label }}</Label>
              <Input
                :id="node.name"
                v-model="formData[node.name!]"
                :type="node.input_type || 'text'"
                :placeholder="node.placeholder || ''"
                :autocomplete="node.autocomplete || 'off'"
                :required="node.required"
                :disabled="node.disabled"
                :autofocus="i === firstInputIndex"
                :minlength="node.min_length || undefined"
                :maxlength="node.max_length || undefined"
                :pattern="node.pattern || undefined"
              />
              <!-- Password confirmation (client-side only — not a server node) -->
              <template v-if="node.input_type === 'password' && isRegistrationStep">
                <Label :for="node.name + '_confirm'" class="mt-3">Confirm Password</Label>
                <Input
                  :id="node.name + '_confirm'"
                  v-model="confirmPasswords[node.name!]"
                  type="password"
                  placeholder="Confirm your password"
                  autocomplete="new-password"
                  required
                  class="mt-1.5"
                />
                <p v-if="confirmPasswords[node.name!] && formData[node.name!] !== confirmPasswords[node.name!]"
                   class="text-xs text-destructive">Passwords do not match</p>
              </template>
              <!-- Per-field errors -->
              <p v-for="(fe, j) in (node.errors || [])" :key="j" class="text-xs text-destructive">{{ fe }}</p>
            </div>

            <!-- Field Description (helper text below input) -->
            <p v-else-if="node.type === 'field_description'" class="text-xs text-muted-foreground -mt-2 pl-0.5">{{ node.text }}</p>

            <!-- Password Hint (Forgot password?) -->
            <div v-else-if="node.type === 'password_hint'" class="flex justify-end -mt-2">
              <button
                type="button"
                class="text-xs text-muted-foreground underline-offset-4 hover:underline cursor-pointer"
                @click="submitAction(node.action || 'forgot_password')"
              >{{ node.label }}</button>
            </div>

            <!-- Consent Checkbox -->
            <div v-else-if="node.type === 'consent_checkbox'" class="flex items-start gap-2">
              <input
                :id="node.name"
                type="checkbox"
                :required="node.required"
                v-model="formData[node.name!]"
                class="mt-0.5 accent-[var(--brand-primary,#6366f1)]"
              />
              <label :for="node.name" class="text-xs text-muted-foreground leading-relaxed" v-html="renderConsentLabel(node.label || '')"></label>
            </div>

            <!-- Hidden -->
            <input v-else-if="node.type === 'hidden'" type="hidden" :name="node.name" :value="node.value || ''" />

            <!-- Submit -->
            <Button
              v-else-if="node.type === 'submit'"
              type="submit"
              class="w-full"
              :disabled="loading || node.disabled || !passwordsMatch"
              @click="pendingAction = node.action || ''"
            >
              <Spinner v-if="loading" class="size-4 mr-2" />
              {{ loading ? 'Loading...' : node.label }}
            </Button>

            <!-- Divider -->
            <div v-else-if="node.type === 'divider'" class="relative py-2">
              <Separator />
              <span class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 bg-card px-2 text-xs text-muted-foreground">or</span>
            </div>

            <!-- Social Group (container mode — renders SSO buttons as a group) -->
            <div v-else-if="node.type === 'social_group'" class="space-y-2">
              <Button
                v-for="(child, ci) in (node.children || [])"
                :key="'sg'+ci"
                type="button"
                variant="outline"
                class="w-full gap-2"
                @click="submitAction(child.action || 'sso', { provider_id: child.provider_id || '' })"
              >
                <span class="text-base">{{ ssoIcon(child.template || '') }}</span>
                {{ child.label }}
              </Button>
            </div>

            <!-- Alt Button -->
            <Button
              v-else-if="node.type === 'button'"
              type="button"
              variant="outline"
              class="w-full"
              :disabled="loading || node.disabled"
              @click="submitAction(node.action || '')"
            >
              {{ node.label }}
            </Button>

            <!-- SSO Button -->
            <Button
              v-else-if="node.type === 'sso_button'"
              type="button"
              variant="outline"
              class="w-full gap-2"
              @click="submitAction(node.action || 'sso', { provider_id: node.provider_id || '' })"
            >
              <span class="text-base">{{ ssoIcon(node.template || '') }}</span>
              {{ node.label }}
            </Button>

            <!-- Link / Back -->
            <Button
              v-else-if="node.type === 'link'"
              type="button"
              variant="link"
              class="w-full text-muted-foreground"
              @click="submitAction(node.action || 'back')"
            >
              {{ node.label }}
            </Button>

            <!-- Registration Link -->
            <Button
              v-else-if="node.type === 'registration_link'"
              type="button"
              variant="link"
              class="w-full text-muted-foreground font-medium"
              @click="submitAction(node.action || 'register')"
            >
              {{ node.label }}
            </Button>

            <!-- Terms Footer -->
            <p v-else-if="node.type === 'terms_footer'" class="text-xs text-muted-foreground text-center pt-2">
              By clicking continue, you agree to our
              <a v-if="node.attributes?.terms_url" :href="node.attributes.terms_url" target="_blank" class="underline underline-offset-4 hover:text-foreground">Terms of Service</a>
              <template v-if="node.attributes?.terms_url && node.attributes?.privacy_url"> and </template>
              <a v-if="node.attributes?.privacy_url" :href="node.attributes.privacy_url" target="_blank" class="underline underline-offset-4 hover:text-foreground">Privacy Policy</a>.
            </p>

            <!-- Altcha PoW Captcha -->
            <div v-else-if="node.type === 'captcha_altcha'" class="space-y-2">
              <div ref="altchaCaptchaEl" class="flex items-center gap-2 rounded-md border border-input px-3 py-2 text-sm">
                <Spinner v-if="captchaSolving" class="size-4" />
                <svg v-else-if="captchaSolved" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="size-5 text-green-500"><path fill-rule="evenodd" d="M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z" clip-rule="evenodd" /></svg>
                <div v-else class="size-4 rounded border-2 border-muted-foreground/30 cursor-pointer" @click="solveCaptcha" />
                <span class="text-muted-foreground text-xs">{{ captchaSolving ? 'Verifying...' : captchaSolved ? 'Verified' : 'I am human' }}</span>
              </div>
            </div>

            <!-- Generic Captcha Checkbox (hCaptcha, reCAPTCHA, Turnstile) -->
            <div v-else-if="node.type === 'captcha_checkbox'" class="space-y-2">
              <div class="flex items-center gap-2 rounded-md border border-input px-3 py-2 text-sm">
                <input
                  type="checkbox"
                  class="accent-[var(--brand-primary,#6366f1)]"
                  :checked="!!formData[node.name!]"
                  @click="formData[node.name!] = 'verified'"
                />
                <span class="text-muted-foreground text-xs">I am human ({{ node.attributes?.provider || 'captcha' }})</span>
              </div>
            </div>

            <!-- Fingerprint Collector (invisible) -->
            <div v-else-if="node.type === 'fingerprint_collect'" class="hidden" />

            <!-- Group (container for nested nodes) -->
            <div v-else-if="node.type === 'group'" class="space-y-4">
              <template v-for="(child, ci) in (node.children || [])" :key="'g'+ci">
                <div v-if="child.type === 'input'" class="space-y-1.5">
                  <Label :for="child.name">{{ child.label }}</Label>
                  <Input
                    :id="child.name"
                    v-model="formData[child.name!]"
                    :type="child.input_type || 'text'"
                    :placeholder="child.placeholder || ''"
                    :required="child.required"
                  />
                </div>
              </template>
            </div>
          </template>
        </form>
      </CardContent>
    </Card>

    <!-- Powered by -->
    <template #footer>
      <p v-if="!branding?.hide_zitadel_branding" class="mt-6 text-xs text-muted-foreground text-center">
        Powered by Zitadel
      </p>
    </template>
  </component>
</template>

<script setup lang="ts">
/**
 * LoginApp — Server-driven login UI.
 *
 * Renders UINode[] from the flow API. Works both as:
 * 1. Standalone Vue SPA (mounted in #login-app by main.ts)
 * 2. Inside the <zitadel-login> custom element (via LoginApp.ce.vue)
 *
 * Props are optional — when used standalone, defaults use same-origin API.
 * When wrapped in the CE, the api-base-url prop is passed through.
 *
 * ADR-019: Server-Driven Login UI + Web Components
 * ADR-020: Customizable Login Layouts
 */
import { ref, computed, onMounted, onUnmounted, reactive, watch, type Component } from 'vue'
import { flowApi, type FlowStep, type FlowBranding, type FlowCompleteResponse } from '@/api/branding'
import { initTelemetry, shutdownTelemetry, traceStepTransition, traceFormSubmit, setFlowId } from '@/lib/telemetry'
import { collectFingerprint, submitFingerprint } from '@/lib/fingerprint'

// shadcn components
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Spinner } from '@/components/ui/spinner'
import { AlertCircle } from 'lucide-vue-next'

// Layout components
import CenteredLayout from './layouts/CenteredLayout.vue'
import SplitLayout from './layouts/SplitLayout.vue'
import MutedLayout from './layouts/MutedLayout.vue'
import CardImageLayout from './layouts/CardImageLayout.vue'
import MinimalLayout from './layouts/MinimalLayout.vue'

const layoutMap: Record<string, Component> = {
  centered: CenteredLayout,
  split: SplitLayout,
  muted: MutedLayout,
  card_image: CardImageLayout,
  minimal: MinimalLayout,
}

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  redirectUri?: string
  state?: string
  // Layout & branding overrides (from WC props)
  layoutOverride?: string
  darkModeOverride?: string
  coverImageOverride?: string
  primaryColorOverride?: string
}>(), {
  apiBaseUrl: '',
  redirectUri: '',
  state: '',
  layoutOverride: '',
  darkModeOverride: '',
  coverImageOverride: '',
  primaryColorOverride: '',
})

const emit = defineEmits<{
  'login-complete': [detail: { session_id: string; redirect_uri: string }]
  'login-error': [detail: { code: string; message: string }]
  'login-redirect': [detail: { redirect_url: string }]
}>()

const flowStep = ref<FlowStep | null>(null)
const branding = ref<FlowBranding | null>(null)
const error = ref('')
const loading = ref(false)
const formData = reactive<Record<string, string>>({})
const confirmPasswords = reactive<Record<string, string>>({})
const pendingAction = ref('')

// ─── Captcha state ──────────────────────────────────────────
const captchaSolving = ref(false)
const captchaSolved = ref(false)
const altchaCaptchaEl = ref<HTMLElement | null>(null)

// ─── Fingerprint state ──────────────────────────────────────
const fingerprintCollected = ref(false)

// ─── Layout resolution ──────────────────────────────────────
const effectiveLayout = computed(() => {
  const override = props.layoutOverride
  if (override && layoutMap[override]) return override
  return branding.value?.layout || 'centered'
})

const layoutComponent = computed(() => layoutMap[effectiveLayout.value] || CenteredLayout)

// ─── Dark mode ──────────────────────────────────────────────
const effectiveDarkMode = computed(() => {
  if (props.darkModeOverride) return props.darkModeOverride
  return branding.value?.dark_mode || 'light'
})

watch(effectiveDarkMode, (mode) => {
  const root = document.documentElement
  if (mode === 'dark') {
    root.classList.add('dark')
  } else if (mode === 'auto') {
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    root.classList.toggle('dark', mq.matches)
  } else {
    root.classList.remove('dark')
  }
}, { immediate: true })

// ─── Logo (dark mode aware) ─────────────────────────────────
const effectiveLogo = computed(() => {
  if (effectiveDarkMode.value === 'dark' && branding.value?.logo_dark) {
    return branding.value.logo_dark
  }
  return branding.value?.logo_url || ''
})

// ─── Card styling (border-radius) ───────────────────────────
const radiusMap: Record<string, string> = { sm: '0.25rem', md: '0.5rem', lg: '0.75rem', xl: '1rem', full: '9999px' }

const cardClass = computed(() => {
  // For split and card_image layouts, no max-width (layout handles it)
  if (effectiveLayout.value === 'split' || effectiveLayout.value === 'card_image') {
    return ''
  }
  return 'max-w-sm'
})

// ─── Registration step detection (for password confirmation) ─
const isRegistrationStep = computed(() => flowStep.value?.step === 'register')

// ─── Password match validation ──────────────────────────────
const passwordsMatch = computed(() => {
  if (!isRegistrationStep.value) return true
  for (const key of Object.keys(confirmPasswords)) {
    if (confirmPasswords[key] && formData[key] !== confirmPasswords[key]) {
      return false
    }
  }
  return true
})

const firstInputIndex = computed(() => {
  if (!flowStep.value) return -1
  return flowStep.value.nodes.findIndex(n => n.type === 'input')
})

const ssoIcons: Record<string, string> = {
  google: '🔵', entraid: '🟦', gitlab: '🦊', apple: '🍎', github: '🐙', custom: '🔑',
}
function ssoIcon(template: string) { return ssoIcons[template] || '🔑' }

// ─── Consent label renderer (markdown links → inline HTML) ──
function renderConsentLabel(label: string): string {
  // Convert [text](url) to <a> tags
  return label.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" class="underline underline-offset-4 hover:text-foreground">$1</a>')
}

onMounted(async () => {
  // Initialize telemetry (collects page load timing automatically).
  const baseUrl = props.apiBaseUrl || ''
  initTelemetry({ baseUrl, enabled: true })

  try {
    const step = await flowApi.create()
    flowStep.value = step
    branding.value = step.branding

    // Link telemetry to the flow.
    setFlowId(step.flow_id)

    // Apply primary color override.
    if (props.primaryColorOverride && branding.value) {
      branding.value.colors = { ...branding.value.colors, primary: props.primaryColorOverride }
    }
    // Apply cover image override.
    if (props.coverImageOverride && branding.value) {
      branding.value.cover_image = props.coverImageOverride
    }

    // Pre-fill formData from nodes with initial values.
    for (const node of step.nodes) {
      if (node.name && node.value) {
        formData[node.name] = node.value
      }
    }

    // Auto-collect fingerprint if a fingerprint_collect node is present.
    maybeCollectFingerprint(step)
  } catch {
    error.value = 'Failed to initialize login flow'
    emit('login-error', { code: 'init_failed', message: 'Failed to initialize login flow' })
  }
})

onUnmounted(() => {
  shutdownTelemetry()
})

async function onSubmit() {
  const action = pendingAction.value || 'identifier'
  await submitAction(action)
}

async function submitAction(action: string, extra?: Record<string, string>) {
  if (!flowStep.value) return
  loading.value = true
  error.value = ''

  // Trace the form submission.
  const span = traceFormSubmit(action, flowStep.value.flow_id)
  const previousStep = flowStep.value.step

  try {
    const payload: Record<string, string> = { action, ...formData, ...extra }
    const resp = await flowApi.submit(flowStep.value.flow_id, action, payload)

    // Handle SSO redirect.
    if ('redirect_url' in resp && (resp as any).redirect_url) {
      emit('login-redirect', { redirect_url: (resp as any).redirect_url })
      window.location.href = (resp as any).redirect_url
      return
    }

    // Handle completion.
    if ('redirect_uri' in resp && (resp as FlowCompleteResponse).redirect_uri) {
      const complete = resp as FlowCompleteResponse
      emit('login-complete', {
        session_id: String(complete.session_id),
        redirect_uri: complete.redirect_uri,
      })
      window.location.href = complete.redirect_uri
      return
    }

    // Normal step transition.
    const step = resp as FlowStep
    flowStep.value = step
    if (step.branding) branding.value = step.branding

    // Trace step transition.
    if (step.step !== previousStep) {
      traceStepTransition(previousStep || 'unknown', step.step, step.flow_id)
    }

    // Clear password field on step change. Pre-fill new values.
    if (formData.password) formData.password = ''
    // Clear confirm passwords on step change.
    Object.keys(confirmPasswords).forEach(k => { confirmPasswords[k] = '' })

    for (const node of step.nodes) {
      if (node.name && node.value && !formData[node.name]) {
        formData[node.name] = node.value
      }
    }

    // Auto-collect fingerprint if a fingerprint_collect node is present.
    maybeCollectFingerprint(step)

    // Reset captcha state on step change.
    captchaSolved.value = false
    captchaSolving.value = false
  } catch (e: any) {
    const msg = e.message || 'Something went wrong'
    error.value = msg
    emit('login-error', { code: 'submit_failed', message: msg })
  } finally {
    loading.value = false
    pendingAction.value = ''
    if (span) span.end()
  }
}

// ─── Captcha solving ────────────────────────────────────────
async function solveCaptcha() {
  if (!flowStep.value || captchaSolving.value || captchaSolved.value) return
  captchaSolving.value = true

  try {
    const baseUrl = props.apiBaseUrl || ''
    // 1. Get challenge from server.
    const challengeResp = await fetch(`${baseUrl}/v1/captcha/challenge`, {
      credentials: 'include',
    })
    const challenge = await challengeResp.json()

    // 2. Solve PoW in a microtask (SHA-256 brute force).
    const startTime = performance.now()
    let solution = -1
    for (let i = 0; i <= challenge.maxnumber; i++) {
      const input = challenge.salt + String(i)
      const hashBuf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(input))
      const hashHex = Array.from(new Uint8Array(hashBuf)).map(b => b.toString(16).padStart(2, '0')).join('')
      if (hashHex === challenge.challenge) {
        solution = i
        break
      }
    }
    const took = Math.round(performance.now() - startTime)

    if (solution === -1) {
      captchaSolving.value = false
      error.value = 'Captcha challenge could not be solved'
      return
    }

    // 3. Build solution payload (compatible with Altcha widget protocol).
    const payload = JSON.stringify({
      algorithm: challenge.algorithm,
      challenge: challenge.challenge,
      number: solution,
      salt: challenge.salt,
      signature: challenge.signature,
      took,
    })

    // 4. Submit to flow engine.
    await submitAction('captcha_submit', { altcha_payload: payload })
    captchaSolved.value = true
  } catch {
    error.value = 'Captcha verification failed'
  } finally {
    captchaSolving.value = false
  }
}

// ─── Fingerprint auto-collection ────────────────────────────
async function maybeCollectFingerprint(step: FlowStep) {
  if (fingerprintCollected.value) return
  const hasNode = step.nodes.some(n => n.type === 'fingerprint_collect')
  if (!hasNode) return

  try {
    const fp = await collectFingerprint()
    const baseUrl = props.apiBaseUrl || ''
    await submitFingerprint(baseUrl, step.flow_id, fp)
    fingerprintCollected.value = true
  } catch {
    // Silent fail — fingerprint should never block login.
  }
}
</script>
