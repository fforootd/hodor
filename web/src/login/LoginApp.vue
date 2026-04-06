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
    type FlowRedirectResponse,
  } from '@/api/branding'
  import {
    initTelemetry,
    shutdownTelemetry,
    traceStepTransition,
    traceFormSubmit,
    setFlowId,
  } from '@/lib/telemetry'
  import { collectFingerprint, uploadFingerprintContext } from '@/lib/fingerprint'
  import { createReadyzWaiter, useAppBootstrap } from '@/bootstrap/app-bootstrap'
  import { toLoginErrorDetail, type LoginErrorDetail } from './init-state'
  import AppBootstrapScreen from '@/components/AppBootstrapScreen.vue'
  import { Button } from '@/components/ui/button'
  import LoginShell from '@/login/components/LoginShell.vue'
  import LoginNodeRenderer from '@/login/components/LoginNodeRenderer.vue'
  import { CircleCheckBig } from 'lucide-vue-next'
  import { PROTECTED_CAPTCHA_ACTIONS } from '@/login/constants'

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
  const currentUrlParams = new URLSearchParams(window.location.search)
  const effectiveRedirectUri = computed(
    () => props.redirectUri || currentUrlParams.get('redirect_uri') || '',
  )
  const effectiveState = computed(() => props.state || currentUrlParams.get('state') || '')
  const effectiveAuthRequestId = computed(
    () => props.authRequestId || currentUrlParams.get('auth_request_id') || '',
  )
  const exitState = computed(() => currentUrlParams.get('exit') || '')
  const continueTo = computed(() => currentUrlParams.get('continue_to') || '')
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

  const protectedCaptchaActions = PROTECTED_CAPTCHA_ACTIONS

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

  watch(
    () => flowStep.value?.step,
    (newStep, oldStep) => {
      if (!newStep || newStep === oldStep) return
      // Wait for the out-in Transition to complete (0.2s leave + 0.2s enter + buffer).
      setTimeout(() => {
        const form = document.querySelector('form')
        if (!form) return
        const target =
          form.querySelector<HTMLElement>('input:not([type="hidden"]):not([disabled])') ||
          form.querySelector<HTMLElement>('h1')
        target?.focus({ preventScroll: false })
      }, 450)
    },
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
    maybeAutoSolvePow(step)
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
    if (
      action === 'identifier' ||
      action === 'password' ||
      action === 'register_submit' ||
      action === 'use_session'
    ) {
      pendingAction.value = action
      return
    }
    void submitAction(action, extra)
  }

  function defaultSubmitAction() {
    const submitNode = flowStep.value?.nodes.find(
      (node) => node.type === 'submit' && node.action && node.action !== 'back',
    )
    return submitNode?.action || pendingAction.value || 'identifier'
  }

  async function onSubmit(submittedAction?: string) {
    const action = submittedAction || pendingAction.value || defaultSubmitAction()
    if (requiresCaptchaVerification(action)) {
      // POW challenge is auto-solving in the background.
      // Wait for it to complete, then retry the submission.
      if (captchaSolving.value) {
        submitError.value = 'Verifying... please wait.'
        return
      }
      // Trigger a solve if it hasn't started yet.
      await solveCaptcha()
      // After solving, retry the action.
      if (captchaSolved.value) {
        await submitAction(action)
      }
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

  async function submitAction(action: string, extra?: Record<string, unknown>) {
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

      if ('redirect_url' in resp) {
        const redirectUrl = (resp as FlowRedirectResponse).redirect_url
        if (redirectUrl) {
          emit('login-redirect', { redirect_url: redirectUrl })
          window.location.href = redirectUrl
          return
        }
      }

      const flowOrComplete = resp as FlowStep | FlowCompleteResponse
      if (handleCompleteResponse(flowOrComplete)) {
        return
      }

      const step = flowOrComplete as FlowStep
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
      maybeAutoSolvePow(step)
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
      // Look for an embedded captcha_challenge node in the current flow step.
      const challengeNode = flowStep.value.nodes.find(
        (n: any) => n.type === 'captcha_challenge',
      ) as any | undefined

      let challenge: any
      if (challengeNode) {
        // Use the challenge embedded in the flow response (from risk scoring).
        challenge = challengeNode
      } else {
        // Fallback: fetch from the legacy challenge endpoint.
        challenge = await flowApi.captchaChallenge(
          props.apiBaseUrl || '',
          flowStep.value.flow_id,
        )
      }

      // Solve the POW challenge using the imported solver.
      const { solveChallenge } = await import('@/lib/pow-solver')
      const result = await solveChallenge({
        algorithm: String(challenge.algorithm || 'SHA-256'),
        salt: String(challenge.salt || ''),
        challenge: String(challenge.challenge || ''),
        maxnumber: Number(challenge.maxnumber ?? 0),
        signature: String(challenge.signature || ''),
      })

      // Submit the solution as altcha_payload.
      await submitAction('captcha_submit', {
        altcha_payload: {
          salt: result.salt,
          nonce: result.nonce,
          signature: result.signature,
        },
      })
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

  /** Auto-solve POW challenge if the step contains a captcha_challenge node. */
  function maybeAutoSolvePow(step: FlowStep) {
    if (!step.captcha_required) return
    if (step.captcha_verified) return
    const hasChallenge = step.nodes.some((n: any) => n.type === 'captcha_challenge')
    if (!hasChallenge) return
    // Trigger the solver asynchronously — it runs in the background and submits the solution.
    solveCaptcha()
  }

  async function maybeCollectFingerprint(step: FlowStep) {
    const isAutomatedBrowser =
      typeof navigator !== 'undefined' &&
      (navigator.webdriver || /HeadlessChrome|Playwright/i.test(navigator.userAgent))
    if (isAutomatedBrowser) return
    if (fingerprintCollected.value) return
    const hasNode = step.nodes.some((n) => n.type === 'fingerprint_collect')
    if (!hasNode) return

    try {
      const requestFlowId = step.flow_id
      const requestStep = step.step
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
      fingerprintCollected.value = true

      // Ignore late fingerprint responses once the flow has advanced.
      if (
        flowStep.value?.flow_id !== requestFlowId ||
        flowStep.value?.step !== requestStep
      ) {
        return
      }

      applyFlowStepState(refreshedStep)

      // Upload full fingerprint context to telemetry (fire-and-forget).
      uploadFingerprintContext(props.apiBaseUrl || '', fp)
    } catch {
      // Fingerprint collection should never block login.
    }
  }
</script>
