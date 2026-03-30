<template>
  <LoginShell
    :branding="branding"
    :layout-override="props.layoutOverride"
    :dark-mode-override="props.darkModeOverride"
    :cover-image-override="props.coverImageOverride"
    :primary-color-override="props.primaryColorOverride"
  >
    <LoginNodeRenderer
      v-if="initState === 'ready' && flowStep"
      :flow-step="flowStep"
      :submit-error="submitError"
      :loading="loading"
      :form-data="formData"
      :confirm-passwords="confirmPasswords"
      :captcha-solving="captchaSolving"
      :captcha-solved="captchaSolved"
      @submit="onSubmit"
      @action="handleRendererAction"
      @solve-captcha="solveCaptcha"
    />

    <div
      v-else-if="initState === 'initializing'"
      class="flex flex-col items-center gap-3 py-8 text-center"
    >
      <Spinner class="size-6" />
      <div class="space-y-1">
        <p class="text-sm font-medium">Initializing login</p>
        <p class="text-xs text-muted-foreground">Loading your sign-in flow…</p>
      </div>
    </div>

    <div
      v-else-if="initState === 'waiting_for_server'"
      class="flex flex-col items-center gap-3 py-8 text-center"
    >
      <Spinner class="size-6" />
      <div class="space-y-1">
        <p class="text-sm font-medium">Starting Zitadel</p>
        <p class="text-xs text-muted-foreground">
          {{ initError?.message || 'Zitadel is still starting. Try again in a moment.' }}
        </p>
        <p v-if="retryDelayMs" class="text-xs text-muted-foreground/80">Retrying soon…</p>
      </div>
    </div>

    <div
      v-else-if="initState === 'fatal'"
      class="flex flex-col items-center gap-4 py-8 text-center"
    >
      <AlertCircle class="size-8 text-destructive" />
      <div class="space-y-1">
        <p class="text-sm font-medium">Login is unavailable</p>
        <p class="text-xs text-muted-foreground">
          {{ initError?.message || 'Login is temporarily unavailable.' }}
        </p>
        <p v-if="initError?.kind === 'configuration'" class="text-xs text-muted-foreground/80">
          Check the login flow and schema bootstrap data, then retry.
        </p>
      </div>
      <Button type="button" class="w-full" @click="retryInitialize">Retry</Button>
    </div>
  </LoginShell>
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
  import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
  import {
    flowApi,
    type FlowStep,
    type FlowBranding,
    type FlowCompleteResponse,
  } from '@/api/branding'
  import {
    initTelemetry,
    shutdownTelemetry,
    traceStepTransition,
    traceFormSubmit,
    setFlowId,
  } from '@/lib/telemetry'
  import { collectFingerprint, submitFingerprint } from '@/lib/fingerprint'
  import {
    nextLoginInitRetryDelay,
    shouldRetryLoginInit,
    toLoginErrorDetail,
    type LoginErrorDetail,
    type LoginInitState,
  } from './init-state'
  import { Button } from '@/components/ui/button'
  import { Spinner } from '@/components/ui/spinner'
  import { AlertCircle } from 'lucide-vue-next'
  import LoginShell from './components/LoginShell.vue'
  import LoginNodeRenderer from './components/LoginNodeRenderer.vue'

  const props = withDefaults(
    defineProps<{
      apiBaseUrl?: string
      redirectUri?: string
      state?: string
      layoutOverride?: string
      darkModeOverride?: string
      coverImageOverride?: string
      primaryColorOverride?: string
    }>(),
    {
      apiBaseUrl: '',
      redirectUri: '',
      state: '',
      layoutOverride: '',
      darkModeOverride: '',
      coverImageOverride: '',
      primaryColorOverride: '',
    },
  )

  const emit = defineEmits<{
    'login-complete': [detail: { session_id: string; redirect_uri: string }]
    'login-error': [detail: LoginErrorDetail]
    'login-redirect': [detail: { redirect_url: string }]
  }>()

  const flowStep = ref<FlowStep | null>(null)
  const branding = ref<FlowBranding | null>(null)
  const submitError = ref('')
  const loading = ref(false)
  const formData = reactive<Record<string, string>>({})
  const confirmPasswords = reactive<Record<string, string>>({})
  const pendingAction = ref('')
  const initState = ref<LoginInitState>('initializing')
  const initError = ref<LoginErrorDetail | null>(null)
  const retryDelayMs = ref(0)
  const captchaSolving = ref(false)
  const captchaSolved = ref(false)
  const fingerprintCollected = ref(false)
  let disposed = false

  const effectiveDarkMode = computed(() => {
    if (props.darkModeOverride) return props.darkModeOverride
    return branding.value?.dark_mode || 'light'
  })

  watch(
    effectiveDarkMode,
    (mode) => {
      const root = document.documentElement
      if (mode === 'dark') {
        root.classList.add('dark')
      } else if (mode === 'auto') {
        const mq = window.matchMedia('(prefers-color-scheme: dark)')
        root.classList.toggle('dark', mq.matches)
      } else {
        root.classList.remove('dark')
      }
    },
    { immediate: true },
  )

  function resetFormState() {
    flowStep.value = null
    submitError.value = ''
    Object.keys(formData).forEach((key) => {
      delete formData[key]
    })
    Object.keys(confirmPasswords).forEach((key) => {
      delete confirmPasswords[key]
    })
  }

  function applyInitializedFlow(step: FlowStep) {
    flowStep.value = step
    branding.value = step.branding
    initState.value = 'ready'
    initError.value = null
    retryDelayMs.value = 0
    setFlowId(step.flow_id)

    for (const node of step.nodes) {
      if (node.name && node.value) {
        formData[node.name] = node.value
      }
    }

    maybeCollectFingerprint(step)
  }

  function sleep(ms: number) {
    return new Promise((resolve) => window.setTimeout(resolve, ms))
  }

  async function waitForReadiness(delayMs: number) {
    const startedAt = Date.now()
    while (!disposed && Date.now() - startedAt < delayMs) {
      const ready = await flowApi.ready(props.apiBaseUrl || '').catch(() => false)
      if (ready) return
      await sleep(Math.min(250, delayMs))
    }
  }

  async function initializeFlow() {
    resetFormState()
    let attempt = 0

    while (!disposed) {
      initState.value = attempt === 0 ? 'initializing' : 'waiting_for_server'
      retryDelayMs.value = 0

      try {
        const step = await flowApi.create(props.apiBaseUrl || '', props.redirectUri, props.state)
        applyInitializedFlow(step)
        return
      } catch (err) {
        const detail = toLoginErrorDetail(err)
        initError.value = detail

        if (!shouldRetryLoginInit(detail, attempt)) {
          initState.value = 'fatal'
          emit('login-error', detail)
          return
        }

        const delay = nextLoginInitRetryDelay(attempt)
        if (delay == null) {
          initState.value = 'fatal'
          emit('login-error', detail)
          return
        }

        initState.value = 'waiting_for_server'
        retryDelayMs.value = delay
        await waitForReadiness(delay)
        attempt += 1
      }
    }
  }

  async function retryInitialize() {
    initError.value = null
    initState.value = 'initializing'
    await initializeFlow()
  }

  onMounted(async () => {
    initTelemetry({ baseUrl: props.apiBaseUrl || '', enabled: true })
    await initializeFlow()
  })

  onUnmounted(() => {
    disposed = true
    shutdownTelemetry()
  })

  function handleRendererAction(action: string, extra?: Record<string, string>) {
    if (!action) return
    if (action === 'identifier' || action === 'password' || action === 'register_submit') {
      pendingAction.value = action
      return
    }
    void submitAction(action, extra)
  }

  async function onSubmit() {
    const action = pendingAction.value || 'identifier'
    await submitAction(action)
  }

  async function submitAction(action: string, extra?: Record<string, string>) {
    if (!flowStep.value) return
    loading.value = true
    submitError.value = ''

    const span = traceFormSubmit(action, flowStep.value.flow_id)
    const previousStep = flowStep.value.step

    try {
      const payload: Record<string, string> = { action, ...formData, ...extra }
      const resp = await flowApi.submit(
        props.apiBaseUrl || '',
        flowStep.value.flow_id,
        action,
        payload,
      )

      if ('redirect_url' in resp && (resp as any).redirect_url) {
        emit('login-redirect', { redirect_url: (resp as any).redirect_url })
        window.location.href = (resp as any).redirect_url
        return
      }

      if ('redirect_uri' in resp && (resp as FlowCompleteResponse).redirect_uri) {
        const complete = resp as FlowCompleteResponse
        emit('login-complete', {
          session_id: String(complete.session_id),
          redirect_uri: complete.redirect_uri,
        })
        window.location.href = complete.redirect_uri
        return
      }

      const step = resp as FlowStep
      flowStep.value = step
      if (step.branding) branding.value = step.branding

      if (step.step !== previousStep) {
        traceStepTransition(previousStep || 'unknown', step.step, step.flow_id)
      }

      if (formData.password) formData.password = ''
      Object.keys(confirmPasswords).forEach((k) => {
        confirmPasswords[k] = ''
      })

      for (const node of step.nodes) {
        if (node.name && node.value && !formData[node.name]) {
          formData[node.name] = node.value
        }
      }

      maybeCollectFingerprint(step)
      captchaSolved.value = false
      captchaSolving.value = false
    } catch (err) {
      const detail = toLoginErrorDetail(err)
      submitError.value = detail.message
      emit('login-error', detail)
    } finally {
      loading.value = false
      pendingAction.value = ''
      if (span) span.end()
    }
  }

  async function solveCaptcha() {
    if (!flowStep.value || captchaSolving.value || captchaSolved.value) return
    captchaSolving.value = true

    try {
      const baseUrl = props.apiBaseUrl || ''
      const challengeResp = await fetch(`${baseUrl}/v1/captcha/challenge`, {
        credentials: 'include',
      })
      const challenge = await challengeResp.json()

      const startTime = performance.now()
      let solution = -1
      for (let i = 0; i <= challenge.maxnumber; i++) {
        const input = challenge.salt + String(i)
        const hashBuf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(input))
        const hashHex = Array.from(new Uint8Array(hashBuf))
          .map((b) => b.toString(16).padStart(2, '0'))
          .join('')
        if (hashHex === challenge.challenge) {
          solution = i
          break
        }
      }
      const took = Math.round(performance.now() - startTime)

      if (solution === -1) {
        captchaSolving.value = false
        submitError.value = 'Captcha challenge could not be solved'
        return
      }

      const payload = JSON.stringify({
        algorithm: challenge.algorithm,
        challenge: challenge.challenge,
        number: solution,
        salt: challenge.salt,
        signature: challenge.signature,
        took,
      })

      await submitAction('captcha_submit', { altcha_payload: payload })
      captchaSolved.value = true
    } catch {
      submitError.value = 'Captcha verification failed'
    } finally {
      captchaSolving.value = false
    }
  }

  async function maybeCollectFingerprint(step: FlowStep) {
    if (fingerprintCollected.value) return
    const hasNode = step.nodes.some((n) => n.type === 'fingerprint_collect')
    if (!hasNode) return

    try {
      const fp = await collectFingerprint()
      await submitFingerprint(props.apiBaseUrl || '', step.flow_id, fp)
      fingerprintCollected.value = true
    } catch {
      // Fingerprint collection should never block login.
    }
  }
</script>
