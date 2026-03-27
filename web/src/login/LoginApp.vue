<template>
  <div class="flex min-h-screen flex-col items-center justify-center p-4" :style="shellStyle">
    <Card class="w-full max-w-sm">
      <CardHeader class="text-center">
        <!-- Logo -->
        <div v-if="branding?.logo_url" class="flex justify-center mb-2">
          <img :src="branding.logo_url" :alt="branding.org_name" class="h-8" />
        </div>
        <div v-else class="text-xl font-bold tracking-tight mb-2">{{ branding?.org_name || 'Zitadel' }}</div>
      </CardHeader>

      <CardContent>
        <!-- Error -->
        <Alert v-if="error" variant="destructive" class="mb-4">
          <AlertCircle class="size-4" />
          <AlertDescription>{{ error }}</AlertDescription>
        </Alert>

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
            <div v-else-if="node.type === 'avatar'" class="flex justify-center">
              <Avatar class="size-10">
                <AvatarFallback>{{ node.initial }}</AvatarFallback>
              </Avatar>
            </div>

            <!-- Icon -->
            <div v-else-if="node.type === 'icon'" class="text-center text-3xl">{{ node.text }}</div>

            <!-- Info -->
            <Alert v-else-if="node.type === 'info'" class="text-sm">
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
                :autofocus="i === firstInputIndex"
              />
            </div>

            <!-- Submit -->
            <Button
              v-else-if="node.type === 'submit'"
              type="submit"
              class="w-full"
              :disabled="loading"
              @click="pendingAction = node.action || ''"
            >
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
              :disabled="loading"
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

            <!-- Link -->
            <Button
              v-else-if="node.type === 'link'"
              type="button"
              variant="link"
              class="w-full text-muted-foreground"
              @click="submitAction(node.action || 'back')"
            >
              {{ node.label }}
            </Button>
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
  google: '🔵', entraid: '🟦', gitlab: '🦊', apple: '🍎', custom: '🔑',
}
function ssoIcon(template: string) { return ssoIcons[template] || '🔑' }

onMounted(async () => {
  try {
    const step = await flowApi.create()
    flowStep.value = step
    branding.value = step.branding
  } catch {
    error.value = 'Failed to initialize login flow'
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

    if ('redirect_uri' in resp && (resp as FlowCompleteResponse).redirect_uri) {
      window.location.href = (resp as FlowCompleteResponse).redirect_uri
      return
    }
    if ('redirect_url' in resp) {
      window.location.href = (resp as any).redirect_url
      return
    }

    const step = resp as FlowStep
    flowStep.value = step
    if (step.branding) branding.value = step.branding
    if (formData.password) formData.password = ''
  } catch (e: any) {
    const msg = e.message || 'Something went wrong'
    if (msg.includes('invalid_password')) {
      error.value = 'Invalid password. Please try again.'
    } else if (msg.includes('not found')) {
      error.value = 'Account not found.'
    } else {
      error.value = msg
    }
  } finally {
    loading.value = false
    pendingAction.value = ''
  }
}
</script>
