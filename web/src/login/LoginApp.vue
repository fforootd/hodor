<template>
  <div class="flex min-h-screen flex-col items-center justify-center p-4" :style="shellStyle">
    <!-- Custom font (if specified by branding) -->
    <link v-if="branding?.font_url" rel="stylesheet" :href="branding.font_url" />

    <Card class="w-full max-w-sm">
      <CardHeader class="text-center">
        <!-- Logo -->
        <div v-if="branding?.logo_url" class="flex justify-center mb-2">
          <img :src="branding.logo_url" :alt="branding.org_name" class="h-8" />
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
              <Label :for="node.name">{{ node.label }}</Label>
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
              <!-- Per-field errors -->
              <p v-for="(fe, j) in (node.errors || [])" :key="j" class="text-xs text-destructive">{{ fe }}</p>
            </div>

            <!-- Hidden -->
            <input v-else-if="node.type === 'hidden'" type="hidden" :name="node.name" :value="node.value || ''" />

            <!-- Submit -->
            <Button
              v-else-if="node.type === 'submit'"
              type="submit"
              class="w-full"
              :disabled="loading || node.disabled"
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

            <!-- Group (container for nested nodes) -->
            <div v-else-if="node.type === 'group'" class="space-y-4">
              <template v-for="(child, ci) in (node.children || [])" :key="'g'+ci">
                <!-- Recursive render would be cleaner, but for POC we support one level -->
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

    <p v-if="!branding?.hide_zitadel_branding" class="mt-6 text-xs text-muted-foreground">
      Powered by Zitadel
    </p>
  </div>
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
 */
import { ref, computed, onMounted, reactive } from 'vue'
import { flowApi, type FlowStep, type FlowBranding, type FlowCompleteResponse } from '@/api/branding'

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

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  redirectUri?: string
  state?: string
}>(), {
  apiBaseUrl: '',
  redirectUri: '',
  state: '',
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
const pendingAction = ref('')

// Dynamic branding styles (CSS custom properties from x-branding schema)
const shellStyle = computed(() => {
  const c = branding.value?.colors || {}
  return {
    '--brand-primary': c.primary || '#6366f1',
    '--brand-background': c.background || '#f0f2ff',
    '--brand-surface': c.surface || '#ffffff',
    '--brand-text': c.text || '#1a1a2e',
    '--brand-error': c.error || '#ef4444',
    background: `linear-gradient(135deg, ${c.background || '#f0f2ff'} 0%, #fafbff 50%, #f5f3ff 100%)`,
    fontFamily: branding.value?.font_family || 'Inter, system-ui, sans-serif',
  }
})

const firstInputIndex = computed(() => {
  if (!flowStep.value) return -1
  return flowStep.value.nodes.findIndex(n => n.type === 'input')
})

const ssoIcons: Record<string, string> = {
  google: '🔵', entraid: '🟦', gitlab: '🦊', apple: '🍎', github: '🐙', custom: '🔑',
}
function ssoIcon(template: string) { return ssoIcons[template] || '🔑' }

onMounted(async () => {
  try {
    const step = await flowApi.create()
    flowStep.value = step
    branding.value = step.branding

    // Pre-fill formData from nodes with initial values.
    for (const node of step.nodes) {
      if (node.name && node.value) {
        formData[node.name] = node.value
      }
    }
  } catch {
    error.value = 'Failed to initialize login flow'
    emit('login-error', { code: 'init_failed', message: 'Failed to initialize login flow' })
  }
})

async function onSubmit() {
  const action = pendingAction.value || 'identifier'
  await submitAction(action)
}

async function submitAction(action: string, extra?: Record<string, string>) {
  if (!flowStep.value) return
  loading.value = true
  error.value = ''

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

    // Clear password field on step change. Pre-fill new values.
    if (formData.password) formData.password = ''
    for (const node of step.nodes) {
      if (node.name && node.value && !formData[node.name]) {
        formData[node.name] = node.value
      }
    }
  } catch (e: any) {
    const msg = e.message || 'Something went wrong'
    error.value = msg
    emit('login-error', { code: 'submit_failed', message: msg })
  } finally {
    loading.value = false
    pendingAction.value = ''
  }
}
</script>
