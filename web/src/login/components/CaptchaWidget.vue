<template>
  <div v-if="provider === 'altcha'" class="space-y-2">
    <div class="flex items-center gap-2 rounded-md border border-input px-3 py-2 text-sm">
      <Spinner v-if="captchaSolving" class="size-4" />
      <svg
        v-else-if="verified"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 20 20"
        fill="currentColor"
        class="size-5 text-green-500"
      >
        <path
          fill-rule="evenodd"
          d="M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z"
          clip-rule="evenodd"
        />
      </svg>
      <div
        v-else
        class="size-4 rounded border-2 border-muted-foreground/30"
        :class="preview ? 'cursor-default' : 'cursor-pointer'"
        @click="!preview && emit('solve-altcha')"
      />
      <span class="text-muted-foreground text-xs">
        {{ captchaSolving ? 'Verifying...' : verified ? 'Verified' : 'I am human' }}
      </span>
    </div>
  </div>

  <div v-else class="space-y-2">
    <div
      v-if="preview"
      class="rounded-md border border-input px-3 py-4 text-xs text-muted-foreground text-center"
    >
      {{ providerLabel }}
      widget
    </div>

    <div
      v-else-if="!siteKey"
      class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-3 text-xs text-destructive"
    >
      {{ providerLabel }} requires a site key.
    </div>

    <div
      v-else-if="verified"
      class="flex items-center gap-2 rounded-md border border-input px-3 py-3 text-sm"
    >
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 20 20"
        fill="currentColor"
        class="size-5 text-green-500"
      >
        <path
          fill-rule="evenodd"
          d="M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z"
          clip-rule="evenodd"
        />
      </svg>
      <span class="text-muted-foreground text-xs">{{ providerLabel }} verified</span>
    </div>

    <div v-else>
      <div ref="container" class="min-h-[78px]" />
      <p v-if="widgetError" class="text-xs text-destructive">{{ widgetError }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
  import type { UINode } from '@/api/branding'
  import { Spinner } from '@/components/ui/spinner'

  declare global {
    interface Window {
      hcaptcha?: any
      grecaptcha?: any
      turnstile?: any
      __zitadelCaptchaScripts?: Record<string, Promise<void>>
    }
  }

  const props = withDefaults(
    defineProps<{
      node: UINode
      preview?: boolean
      captchaSolving?: boolean
      verified?: boolean
    }>(),
    {
      preview: false,
      captchaSolving: false,
      verified: false,
    },
  )

  const emit = defineEmits<{
    'solve-altcha': []
    token: [token: string]
    error: [message: string]
    reset: []
  }>()

  const container = ref<HTMLElement | null>(null)
  const widgetError = ref('')
  let widgetId: string | number | null = null

  const provider = computed(() => props.node.attributes?.provider || 'altcha')
  const siteKey = computed(() => props.node.attributes?.['site-key'] || '')
  const providerLabel = computed(() => {
    switch (provider.value) {
      case 'hcaptcha':
        return 'hCaptcha'
      case 'recaptcha':
        return 'reCAPTCHA'
      case 'turnstile':
        return 'Cloudflare Turnstile'
      default:
        return 'Captcha'
    }
  })

  function loadScript(src: string, globalName: 'hcaptcha' | 'grecaptcha' | 'turnstile') {
    if (typeof window === 'undefined') return Promise.resolve()
    if ((window as any)[globalName]) return Promise.resolve()
    if (!window.__zitadelCaptchaScripts) {
      window.__zitadelCaptchaScripts = {}
    }
    const existing = window.__zitadelCaptchaScripts[src]
    if (existing) return existing

    const promise = new Promise<void>((resolve, reject) => {
      const script = document.createElement('script')
      script.src = src
      script.async = true
      script.defer = true
      script.onload = () => resolve()
      script.onerror = () => reject(new Error(`Failed to load ${providerLabel.value}`))
      document.head.appendChild(script)
    })
    window.__zitadelCaptchaScripts[src] = promise
    return promise
  }

  function clearWidget() {
    try {
      if (widgetId != null) {
        if (provider.value === 'hcaptcha' && window.hcaptcha?.remove) {
          window.hcaptcha.remove(widgetId)
        } else if (provider.value === 'turnstile' && window.turnstile?.remove) {
          window.turnstile.remove(widgetId)
        } else if (provider.value === 'recaptcha' && window.grecaptcha?.reset) {
          window.grecaptcha.reset(widgetId)
        }
      }
    } catch {
      // Best effort cleanup; avoid throwing during step transitions.
    }
    widgetId = null
    if (container.value) {
      container.value.innerHTML = ''
    }
  }

  function widgetTheme() {
    if (typeof document === 'undefined') return 'light'
    return document.documentElement.classList.contains('dark') ? 'dark' : 'light'
  }

  async function renderThirdPartyWidget() {
    clearWidget()
    widgetError.value = ''

    if (
      props.preview ||
      props.verified ||
      provider.value === 'altcha' ||
      !siteKey.value ||
      !container.value
    ) {
      return
    }

    try {
      await nextTick()
      if (!container.value) return

      if (provider.value === 'hcaptcha') {
        await loadScript('https://js.hcaptcha.com/1/api.js?render=explicit', 'hcaptcha')
        widgetId = window.hcaptcha.render(container.value, {
          sitekey: siteKey.value,
          theme: widgetTheme(),
          callback: (token: string) => {
            widgetError.value = ''
            emit('token', token)
          },
          'expired-callback': () => emit('reset'),
          'error-callback': () => {
            widgetError.value = 'Captcha verification failed. Please try again.'
            emit('reset')
            emit('error', widgetError.value)
          },
        })
        return
      }

      if (provider.value === 'recaptcha') {
        await loadScript('https://www.google.com/recaptcha/api.js?render=explicit', 'grecaptcha')
        const grecaptcha = window.grecaptcha
        grecaptcha.ready(() => {
          if (!container.value || props.verified) return
          widgetId = grecaptcha.render(container.value, {
            sitekey: siteKey.value,
            theme: widgetTheme(),
            callback: (token: string) => {
              widgetError.value = ''
              emit('token', token)
            },
            'expired-callback': () => emit('reset'),
            'error-callback': () => {
              widgetError.value = 'Captcha verification failed. Please try again.'
              emit('reset')
              emit('error', widgetError.value)
            },
          })
        })
        return
      }

      if (provider.value === 'turnstile') {
        await loadScript(
          'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit',
          'turnstile',
        )
        widgetId = window.turnstile.render(container.value, {
          sitekey: siteKey.value,
          theme: widgetTheme(),
          callback: (token: string) => {
            widgetError.value = ''
            emit('token', token)
          },
          'expired-callback': () => emit('reset'),
          'error-callback': () => {
            widgetError.value = 'Captcha verification failed. Please try again.'
            emit('reset')
            emit('error', widgetError.value)
          },
        })
      }
    } catch (err) {
      widgetError.value = err instanceof Error ? err.message : 'Captcha could not be loaded.'
      emit('error', widgetError.value)
    }
  }

  watch(
    () => [provider.value, siteKey.value, props.preview, props.verified] as const,
    () => {
      void renderThirdPartyWidget()
    },
    { immediate: true },
  )

  onMounted(() => {
    void renderThirdPartyWidget()
  })

  onBeforeUnmount(() => {
    clearWidget()
  })
</script>
