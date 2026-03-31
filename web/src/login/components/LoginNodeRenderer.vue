<template>
  <Alert v-if="submitError" variant="destructive" class="mb-4">
    <AlertCircle class="size-4" />
    <AlertDescription>{{ submitError }}</AlertDescription>
  </Alert>

  <template v-if="flowStep?.errors?.length">
    <Alert v-for="(err, i) in flowStep.errors" :key="'ge-' + i" variant="destructive" class="mb-4">
      <AlertCircle class="size-4" />
      <AlertDescription>{{ err.message }}</AlertDescription>
    </Alert>
  </template>

  <template v-if="flowStep?.messages?.length">
    <Alert
      v-for="(msg, i) in flowStep.messages"
      :key="'gm-' + i"
      :class="{
        'mb-4': true,
        'border-green-200 bg-green-50 text-green-800': msg.type === 'success',
        'border-yellow-200 bg-yellow-50 text-yellow-800': msg.type === 'warning',
      }"
    >
      <AlertDescription>{{ msg.text }}</AlertDescription>
    </Alert>
  </template>

  <Transition name="fade" mode="out-in">
    <form v-if="flowStep" :key="flowStep.step" @submit.prevent="emit('submit')" class="space-y-4">
      <template v-for="(node, i) in orderedNodes" :key="i">
        <h1 v-if="node.type === 'heading'" class="text-xl font-semibold text-center">
          {{ node.text }}
        </h1>

        <p
          v-else-if="node.type === 'description'"
          class="text-sm text-muted-foreground text-center"
        >
          {{ node.text }}
        </p>

        <div v-else-if="node.type === 'avatar'" class="flex flex-col items-center gap-1">
          <Avatar class="size-10">
            <AvatarFallback>{{ node.initial }}</AvatarFallback>
          </Avatar>
          <span v-if="node.text" class="text-sm text-muted-foreground">{{ node.text }}</span>
        </div>

        <div v-else-if="node.type === 'icon'" class="text-center text-3xl">
          {{ node.text }}
        </div>

        <Alert v-else-if="node.type === 'info'" class="text-sm">
          <AlertDescription>{{ node.text }}</AlertDescription>
        </Alert>

        <Alert v-else-if="node.type === 'error'" variant="destructive" class="text-sm">
          <AlertCircle class="size-4" />
          <AlertDescription>{{ node.text }}</AlertDescription>
        </Alert>

        <div v-else-if="node.type === 'spinner'" class="flex justify-center py-4">
          <Spinner class="size-6" />
        </div>

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
            :disabled="preview || node.disabled"
            :autofocus="!preview && i === firstInputIndex"
            :minlength="node.min_length || undefined"
            :maxlength="node.max_length || undefined"
            :pattern="node.pattern || undefined"
          />
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
              :disabled="preview"
            />
            <p
              v-if="
                confirmPasswords[node.name!] &&
                formData[node.name!] !== confirmPasswords[node.name!]
              "
              class="text-xs text-destructive"
            >
              Passwords do not match
            </p>
          </template>
          <p v-for="(fe, j) in node.errors || []" :key="j" class="text-xs text-destructive">
            {{ fe }}
          </p>
        </div>

        <p
          v-else-if="node.type === 'field_description'"
          class="text-xs text-muted-foreground -mt-2 pl-0.5"
        >
          {{ node.text }}
        </p>

        <div v-else-if="node.type === 'password_hint'" class="flex justify-end -mt-2">
          <button
            type="button"
            class="text-xs text-muted-foreground underline-offset-4 hover:underline cursor-pointer"
            :disabled="preview"
            @click="emit('action', node.action || 'forgot_password')"
          >
            {{ node.label }}
          </button>
        </div>

        <div v-else-if="node.type === 'consent_checkbox'" class="flex items-start gap-2">
          <input
            :id="node.name"
            v-model="formData[node.name!]"
            type="checkbox"
            :required="node.required"
            class="mt-0.5 accent-[var(--brand-primary,#6366f1)]"
            :disabled="preview"
          />
          <label
            :for="node.name"
            class="text-xs text-muted-foreground leading-relaxed"
            v-html="renderConsentLabel(node.label || '')"
          ></label>
        </div>

        <input
          v-else-if="node.type === 'hidden'"
          type="hidden"
          :name="node.name"
          :value="node.value || ''"
        />

        <Button
          v-else-if="node.type === 'submit'"
          type="submit"
          class="w-full"
          :disabled="
            preview ||
            loading ||
            node.disabled ||
            !passwordsMatch ||
            actionDisabled(node.action || '')
          "
          @click="emit('action', node.action || '')"
        >
          <Spinner v-if="loading" class="size-4 mr-2" />
          {{ loading ? 'Loading...' : node.label }}
        </Button>

        <div v-else-if="node.type === 'divider'" class="relative py-2">
          <Separator />
          <span
            class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 bg-card px-2 text-xs text-muted-foreground"
            >or</span
          >
        </div>

        <div v-else-if="node.type === 'social_group'" class="space-y-2">
          <Button
            v-for="(child, ci) in node.children || []"
            :key="'sg' + ci"
            type="button"
            variant="outline"
            class="w-full gap-2"
            :disabled="preview || actionDisabled(child.action || 'sso')"
            @click="emit('action', child.action || 'sso', { provider_id: child.provider_id || '' })"
          >
            <span class="text-base">{{ ssoIcon(child.template || '') }}</span>
            {{ child.label }}
          </Button>
        </div>

        <Button
          v-else-if="node.type === 'button'"
          type="button"
          variant="outline"
          class="w-full"
          :disabled="preview || loading || node.disabled || actionDisabled(node.action || '')"
          @click="emit('action', node.action || '')"
        >
          {{ node.label }}
        </Button>

        <Button
          v-else-if="node.type === 'sso_button'"
          type="button"
          variant="outline"
          class="w-full gap-2"
          :disabled="preview || actionDisabled(node.action || 'sso')"
          @click="emit('action', node.action || 'sso', { provider_id: node.provider_id || '' })"
        >
          <span class="text-base">{{ ssoIcon(node.template || '') }}</span>
          {{ node.label }}
        </Button>

        <Button
          v-else-if="node.type === 'link'"
          type="button"
          variant="link"
          class="w-full text-muted-foreground"
          :disabled="preview"
          @click="emit('action', node.action || 'back')"
        >
          {{ node.label }}
        </Button>

        <Button
          v-else-if="node.type === 'registration_link'"
          type="button"
          variant="link"
          class="w-full text-muted-foreground font-medium"
          :disabled="preview"
          @click="emit('action', node.action || 'register')"
        >
          {{ node.label }}
        </Button>

        <p
          v-else-if="node.type === 'terms_footer'"
          class="text-xs text-muted-foreground text-center pt-2"
        >
          By clicking continue, you agree to our
          <a
            v-if="node.attributes?.terms_url"
            :href="node.attributes.terms_url"
            target="_blank"
            class="underline underline-offset-4 hover:text-foreground"
            >Terms of Service</a
          >
          <template v-if="node.attributes?.terms_url && node.attributes?.privacy_url">
            and
          </template>
          <a
            v-if="node.attributes?.privacy_url"
            :href="node.attributes.privacy_url"
            target="_blank"
            class="underline underline-offset-4 hover:text-foreground"
            >Privacy Policy</a
          >.
        </p>

        <CaptchaWidget
          v-else-if="node.type === 'captcha_altcha' || node.type === 'captcha_checkbox'"
          :node="node"
          :preview="preview"
          :captcha-solving="props.captchaSolving"
          :verified="props.captchaSolved"
          @solve-altcha="emit('solve-captcha')"
          @token="emit('captcha-token', $event)"
          @reset="emit('captcha-reset')"
          @error="emit('captcha-error', $event)"
        />

        <div v-else-if="node.type === 'fingerprint_collect'" class="hidden" />

        <div v-else-if="node.type === 'group'" class="space-y-4">
          <template v-for="(child, ci) in node.children || []" :key="'g' + ci">
            <div v-if="child.type === 'input'" class="space-y-1.5">
              <Label :for="child.name">{{ child.label }}</Label>
              <Input
                :id="child.name"
                v-model="formData[child.name!]"
                :type="child.input_type || 'text'"
                :placeholder="child.placeholder || ''"
                :required="child.required"
                :disabled="preview"
              />
            </div>
          </template>
        </div>
      </template>
    </form>
  </Transition>
</template>

<script setup lang="ts">
  import { computed, reactive } from 'vue'
  import type { FlowStep } from '@/api/branding'
  import { Button } from '@/components/ui/button'
  import { Input } from '@/components/ui/input'
  import { Label } from '@/components/ui/label'
  import { Separator } from '@/components/ui/separator'
  import { Avatar, AvatarFallback } from '@/components/ui/avatar'
  import { Alert, AlertDescription } from '@/components/ui/alert'
  import { Spinner } from '@/components/ui/spinner'
  import { AlertCircle } from 'lucide-vue-next'
  import CaptchaWidget from './CaptchaWidget.vue'

  const props = withDefaults(
    defineProps<{
      flowStep: FlowStep | null
      submitError?: string
      loading?: boolean
      preview?: boolean
      captchaSolving?: boolean
      captchaSolved?: boolean
      captchaRequired?: boolean
      formData?: Record<string, any>
      confirmPasswords?: Record<string, string>
    }>(),
    {
      submitError: '',
      loading: false,
      preview: false,
      captchaSolving: false,
      captchaSolved: false,
      captchaRequired: false,
      formData: () => ({}),
      confirmPasswords: () => ({}),
    },
  )

  const emit = defineEmits<{
    submit: []
    action: [action: string, extra?: Record<string, string>]
    'solve-captcha': []
    'captcha-token': [token: string]
    'captcha-reset': []
    'captcha-error': [message: string]
  }>()

  const formData = reactive(props.formData)
  const confirmPasswords = reactive(props.confirmPasswords)
  const orderedNodes = computed(() => {
    const nodes = props.flowStep?.nodes || []
    const captchaNodes = nodes.filter(
      (node) => node.type === 'captcha_altcha' || node.type === 'captcha_checkbox',
    )
    if (captchaNodes.length === 0) return nodes

    const withoutCaptcha = nodes.filter(
      (node) => node.type !== 'captcha_altcha' && node.type !== 'captcha_checkbox',
    )
    const submitIndex = withoutCaptcha.findIndex((node) => node.type === 'submit')
    if (submitIndex === -1) return withoutCaptcha.concat(captchaNodes)

    withoutCaptcha.splice(submitIndex + 1, 0, ...captchaNodes)
    return withoutCaptcha
  })

  const isRegistrationStep = computed(() => props.flowStep?.step === 'register')
  const firstInputIndex = computed(() => {
    if (!props.flowStep) return -1
    return orderedNodes.value.findIndex((n) => n.type === 'input')
  })
  const passwordsMatch = computed(() => {
    if (!isRegistrationStep.value) return true
    for (const key of Object.keys(confirmPasswords)) {
      if (confirmPasswords[key] && formData[key] !== confirmPasswords[key]) {
        return false
      }
    }
    return true
  })
  const ssoIcons: Record<string, string> = {
    google: '🔵',
    entraid: '🟦',
    gitlab: '🦊',
    apple: '🍎',
    github: '🐙',
    custom: '🔑',
  }

  function ssoIcon(template: string) {
    return ssoIcons[template] || '🔑'
  }

  function renderConsentLabel(label: string): string {
    return label.replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" target="_blank" class="underline underline-offset-4 hover:text-foreground">$1</a>',
    )
  }

  const protectedCaptchaActions = new Set([
    'identifier',
    'password',
    'magic_link',
    'sso',
    'register_submit',
    'send_reset',
  ])

  function actionDisabled(action: string): boolean {
    return (
      !!action &&
      !!props.captchaRequired &&
      !props.captchaSolved &&
      protectedCaptchaActions.has(action)
    )
  }
</script>

<style scoped>
  .fade-enter-active,
  .fade-leave-active {
    transition:
      opacity 0.2s ease,
      transform 0.2s ease;
  }
  .fade-enter-from {
    opacity: 0;
    transform: translateY(8px);
  }
  .fade-leave-to {
    opacity: 0;
    transform: translateY(-8px);
  }
</style>
