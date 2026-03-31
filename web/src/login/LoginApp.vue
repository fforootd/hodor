<template>
  <AppBootstrapScreen
    v-if="initState !== 'ready' || (!flowStep && !isExitMode)"
    app-name="login"
    :state="initState"
    :error="initError"
    :retry-delay-ms="retryDelayMs"
    configuration-hint="Check the login flow and schema bootstrap data, then retry."
    @retry="retryInitialize"
  />

  <LoginShell
    v-else
    :branding="branding"
    :layout-override="props.layoutOverride"
    :dark-mode-override="props.darkModeOverride"
    :cover-image-override="props.coverImageOverride"
    :primary-color-override="props.primaryColorOverride"
  >
    <div v-if="isExitMode" data-testid="login-exit-state" class="space-y-4 text-center">
      <div
        class="mx-auto flex size-12 items-center justify-center rounded-full bg-primary/10 text-primary"
      >
        <CircleCheckBig class="size-6" />
      </div>
      <div class="space-y-2">
        <h1 data-testid="login-exit-title" class="text-xl font-semibold">
          {{ exitCopy.title }}
        </h1>
        <p class="text-sm text-muted-foreground">
          {{ exitCopy.description }}
        </p>
      </div>

      <Button
        v-if="sanitizedContinueTo"
        data-testid="login-exit-continue"
        type="button"
        class="w-full"
        @click="goToContinueTarget"
      >
        Continue
      </Button>
    </div>

    <LoginNodeRenderer
      v-else
      :flow-step="flowStep"
      :submit-error="submitError"
      :loading="loading"
      :form-data="formData"
      :confirm-passwords="confirmPasswords"
      :captcha-solving="captchaSolving"
      :captcha-solved="captchaSolved"
      :captcha-required="flowStep?.captcha_required === true"
      @update:form-data="handleFormDataUpdate"
      @update:confirm-passwords="handleConfirmPasswordsUpdate"
      @submit="onSubmit"
      @action="handleRendererAction"
      @solve-captcha="solveCaptcha"
      @captcha-token="submitCaptchaToken"
      @captcha-reset="resetCaptchaState"
      @captcha-error="setCaptchaError"
    />
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
  import { collectFingerprint } from '@/lib/fingerprint'
  import { createReadyzWaiter, useAppBootstrap } from '@/bootstrap/app-bootstrap'
  import { toLoginErrorDetail, type LoginErrorDetail } from './init-state'
  import AppBootstrapScreen from '@/components/AppBootstrapScreen.vue'
  import { Button } from '@/components/ui/button'
  import LoginShell from './components/LoginShell.vue'
  import LoginNodeRenderer from './components/LoginNodeRenderer.vue'
  import { CircleCheckBig } from 'lucide-vue-next'

  const props = withDefaults(
    defineProps<{
      apiBaseUrl?: string
      redirectUri?: string
      state?: string
      authRequestId?: string
      layoutOverride?: string
      darkModeOverride?: string
      coverImageOverride?: string
      primaryColorOverride?: string
    }>(),
    {
      apiBaseUrl: '',
      redirectUri: '',
      state: '',
      authRequestId: '',
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
  const captchaSolving = ref(false)
  const fingerprintCollected = ref(false)
  const currentUrlParams = computed(() => new URLSearchParams(window.location.search))
  const effectiveRedirectUri = computed(
    () => props.redirectUri || currentUrlParams.value.get('redirect_uri') || '',
  )
  const effectiveState = computed(() => props.state || currentUrlParams.value.get('state') || '')
  const effectiveAuthRequestId = computed(
    () => props.authRequestId || currentUrlParams.value.get('auth_request_id') || '',
  )
  const exitState = computed(() => currentUrlParams.value.get('exit') || '')
  const continueTo = computed(() => currentUrlParams.value.get('continue_to') || '')
  const isExitMode = computed(() => exitState.value !== '')
  const sanitizedContinueTo = computed(() => sanitizeContinueTo(continueTo.value))
  const exitCopy = computed(() => {
    switch (exitState.value) {
      case 'device_complete':
        return {
          title: 'Device sign-in complete',
          description: 'You can return to the device that requested access and continue there.',
        }
      case 'sso_success':
      default:
        return {
          title: 'Sign-in complete',
          description:
            'Your identity provider sign-in succeeded. You can continue or close this page.',
        }
    }
  })

  const {
    state: initState,
    error: initError,
    retryDelayMs,
    run: runInitialize,
    retry: retryInitialize,
    dispose: disposeBootstrap,
  } = useAppBootstrap(
    async () => {
      resetFormState()
      if (isExitMode.value) {
        return
      }
      const resp = await flowApi.create(
        props.apiBaseUrl || '',
        effectiveRedirectUri.value,
        effectiveState.value,
        effectiveAuthRequestId.value,
      )
      if (handleCompleteResponse(resp)) return
      applyInitializedFlow(resp as FlowStep)
    },
    {
      waitForReady: createReadyzWaiter(props.apiBaseUrl || ''),
      onFatal: (detail) => emit('login-error', detail),
    },
  )

  let disposed = false

  const captchaSolved = computed(() => flowStep.value?.captcha_verified === true)

  const protectedCaptchaActions = new Set([
    'identifier',
    'password',
    'magic_link',
    'sso',
    'register_submit',
    'send_reset',
  ])

  const effectiveDarkMode = computed(() => {
    if (props.darkModeOverride) return props.darkModeOverride
    return branding.value?.dark_mode || 'light'
  })

  function syncFavicon(href: string) {
    if (typeof document === 'undefined') return
    let link = document.querySelector(
      "link[data-zitadel-login-favicon='true']",
    ) as HTMLLinkElement | null
    if (!href) {
      link?.remove()
      return
    }
    if (!link) {
      link = document.createElement('link')
      link.rel = 'icon'
      link.setAttribute('data-zitadel-login-favicon', 'true')
      document.head.appendChild(link)
    }
    link.href = href
  }

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

  watch(
    () => branding.value?.favicon || '',
    (favicon) => {
      syncFavicon(favicon)
    },
    { immediate: true },
  )

  function resetFormState() {
    flowStep.value = null
    submitError.value = ''
    fingerprintCollected.value = false
    Object.keys(formData).forEach((key) => {
      delete formData[key]
    })
    Object.keys(confirmPasswords).forEach((key) => {
      delete confirmPasswords[key]
    })
    captchaSolving.value = false
  }

  function replaceRecord(target: Record<string, any>, next: Record<string, any>) {
    Object.keys(target).forEach((key) => {
      if (!(key in next)) {
        delete target[key]
      }
    })
    Object.assign(target, next)
  }

  function handleFormDataUpdate(nextValue: Record<string, any>) {
    replaceRecord(formData, nextValue)
  }

  function handleConfirmPasswordsUpdate(nextValue: Record<string, string>) {
    replaceRecord(confirmPasswords, nextValue)
  }

  function applyFlowStepState(step: FlowStep) {
    flowStep.value = step
    branding.value = step.branding
    setFlowId(step.flow_id)

    for (const node of step.nodes) {
      if (node.name && node.value) {
        formData[node.name] = node.value
      }
    }
  }

  function applyInitializedFlow(step: FlowStep) {
    applyFlowStepState(step)
    maybeCollectFingerprint(step)
  }

  function handleCompleteResponse(resp: FlowStep | FlowCompleteResponse): boolean {
    if (!('redirect_uri' in resp) || !resp.redirect_uri) {
      return false
    }

    const complete = resp as FlowCompleteResponse
    emit('login-complete', {
      session_id: String(complete.session_id || ''),
      redirect_uri: complete.redirect_uri,
    })
    window.location.href = complete.redirect_uri
    return true
  }

  function sanitizeContinueTo(value: string) {
    if (!value || !value.startsWith('/') || value.startsWith('//')) {
      return ''
    }
    return value
  }

  function goToContinueTarget() {
    if (!sanitizedContinueTo.value) return
    window.location.href = sanitizedContinueTo.value
  }

  onMounted(async () => {
    initTelemetry({ baseUrl: props.apiBaseUrl || '', enabled: true })
    await runInitialize()
  })

  onUnmounted(() => {
    disposed = true
    disposeBootstrap()
    syncFavicon('')
    shutdownTelemetry()
  })

  function handleRendererAction(action: string, extra?: Record<string, string>) {
    if (!action) return
    if (requiresCaptchaVerification(action)) {
      submitError.value = 'Complete captcha verification to continue.'
      return
    }
    if (action === 'identifier' || action === 'password' || action === 'register_submit') {
      pendingAction.value = action
      return
    }
    void submitAction(action, extra)
  }

  async function onSubmit() {
    const action = pendingAction.value || 'identifier'
    if (requiresCaptchaVerification(action)) {
      submitError.value = 'Complete captcha verification to continue.'
      return
    }
    await submitAction(action)
  }

  function requiresCaptchaVerification(action: string): boolean {
    return (
      !!action &&
      protectedCaptchaActions.has(action) &&
      flowStep.value?.captcha_required === true &&
      flowStep.value?.captcha_verified !== true
    )
  }

  function normalizeAltchaDigest(algorithm: string): AlgorithmIdentifier {
    switch (algorithm.toUpperCase()) {
      case 'SHA-384':
        return 'SHA-384'
      case 'SHA-512':
        return 'SHA-512'
      default:
        return 'SHA-256'
    }
  }

  function resetCaptchaState() {
    captchaSolving.value = false
    if (
      submitError.value === 'Complete captcha verification to continue.' ||
      submitError.value === 'Captcha verification failed'
    ) {
      submitError.value = ''
    }
  }

  function setCaptchaError(message: string) {
    captchaSolving.value = false
    submitError.value = message
  }

  async function submitAction(action: string, extra?: Record<string, string>) {
    if (!flowStep.value) return
    loading.value = true
    if (action !== 'captcha_submit') {
      submitError.value = ''
    }

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

      if (handleCompleteResponse(resp)) {
        return
      }

      const step = resp as FlowStep
      applyFlowStepState(step)

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
      captchaSolving.value = false
    } catch (err) {
      const detail = toLoginErrorDetail(err)
      submitError.value = detail.message
      captchaSolving.value = false
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
    submitError.value = ''

    try {
      const challenge = await flowApi.captchaChallenge(
        props.apiBaseUrl || '',
        flowStep.value.flow_id,
      )
      const digestAlgorithm = normalizeAltchaDigest(String(challenge.algorithm || 'SHA-256'))

      const startTime = performance.now()
      let solution = -1
      const maxNumber = Number(challenge.maxnumber ?? 0)
      for (let i = 0; i <= maxNumber; i++) {
        const input = String(challenge.salt || '') + String(i)
        const hashBuf = await crypto.subtle.digest(digestAlgorithm, new TextEncoder().encode(input))
        const hashHex = Array.from(new Uint8Array(hashBuf))
          .map((b) => b.toString(16).padStart(2, '0'))
          .join('')
        if (hashHex === String(challenge.challenge || '')) {
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
        algorithm: String(challenge.algorithm || ''),
        challenge: String(challenge.challenge || ''),
        number: solution,
        salt: String(challenge.salt || ''),
        signature: String(challenge.signature || ''),
        took,
      })

      await submitAction('captcha_submit', { altcha_payload: payload })
    } catch (err) {
      const detail = toLoginErrorDetail(err)
      if (
        detail.status === 400 &&
        detail.message.toLowerCase().includes('not active for this step') &&
        flowStep.value
      ) {
        try {
          const refreshedStep = await flowApi.get(props.apiBaseUrl || '', flowStep.value.flow_id)
          applyFlowStepState(refreshedStep)
          submitError.value = ''
          return
        } catch {
          // Fall through to the generic captcha error if the refresh also fails.
        }
      }
      submitError.value = 'Captcha verification failed'
      captchaSolving.value = false
    } finally {
      captchaSolving.value = false
    }
  }

  async function submitCaptchaToken(token: string) {
    if (!flowStep.value || !token || captchaSolving.value) return
    captchaSolving.value = true
    submitError.value = ''
    await submitAction('captcha_submit', { captcha_token: token })
  }

  async function maybeCollectFingerprint(step: FlowStep) {
    if (fingerprintCollected.value) return
    const hasNode = step.nodes.some((n) => n.type === 'fingerprint_collect')
    if (!hasNode) return

    try {
      const fp = await collectFingerprint()
      const refreshedStep = (await flowApi.submit(
        props.apiBaseUrl || '',
        step.flow_id,
        'fingerprint_submit',
        {
          visitor_id: fp.visitorId,
          fingerprint_hash: fp.visitorId,
        },
      )) as FlowStep
      applyFlowStepState(refreshedStep)
      fingerprintCollected.value = true
    } catch {
      // Fingerprint collection should never block login.
    }
  }
</script>
