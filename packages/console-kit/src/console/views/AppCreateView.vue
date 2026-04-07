<template>
  <WizardSheet
    :open="open"
    title="Create Application"
    description="Register a new application"
    :steps="steps"
    :current-step="currentStep"
    :can-proceed="canProceed"
    :submitting="submitting"
    submit-label="Create Application"
    @update:open="$emit('update:open', $event)"
    @next="onNext"
    @prev="prev"
  >
    <!-- Step 0: Application Type -->
    <template #step-0>
      <div class="space-y-4">
        <div>
          <h3 class="text-base font-medium mb-1">Select Application Type</h3>
          <p class="text-sm text-muted-foreground">Choose the type that best matches your application</p>
        </div>
        <RadioGroup v-model="form.app_type" class="grid gap-3">
          <label
            v-for="t in appTypes"
            :key="t.value"
            class="flex items-start gap-3 rounded-lg border p-4 cursor-pointer transition-colors hover:bg-muted/50"
            :class="form.app_type === t.value ? 'border-primary bg-primary/5' : ''"
          >
            <RadioGroupItem :value="t.value" class="mt-0.5" />
            <component :is="t.icon" class="size-4 text-muted-foreground shrink-0 mt-0.5" />
            <div class="flex-1 min-w-0">
              <div class="text-sm font-medium">{{ t.label }}</div>
              <div class="text-xs text-muted-foreground mt-0.5">{{ t.description }}</div>
              <div v-if="t.tags.length" class="flex flex-wrap gap-1.5 mt-2">
                <Badge v-for="tag in t.tags" :key="tag" variant="secondary" class="text-[10px] h-5">{{ tag }}</Badge>
              </div>
            </div>
          </label>
        </RadioGroup>
      </div>
    </template>

    <!-- Step 1: Application Details -->
    <template #step-1>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Application Details</h3>
          <p class="text-sm text-muted-foreground">Configure your application settings</p>
        </div>
        <div class="space-y-2">
          <Label for="app-name">Application Name</Label>
          <Input id="app-name" v-model="form.name" placeholder="My Web App" />
          <p class="text-xs text-muted-foreground">A human-readable name for this application</p>
        </div>
      </div>
    </template>

    <!-- Step 2: Confirmation -->
    <template #step-2>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Review Application</h3>
        </div>
        <div class="rounded-lg border overflow-hidden text-sm">
          <div class="grid grid-cols-[1fr_auto] p-3 border-b bg-muted/20">
            <span class="text-muted-foreground">Name</span>
            <span class="font-medium text-right">{{ form.name || '—' }}</span>
          </div>
          <div class="grid grid-cols-[1fr_auto] p-3">
            <span class="text-muted-foreground">Type</span>
            <span class="font-medium text-right">{{ selectedTypeLabel }}</span>
          </div>
        </div>
        <div v-if="error" class="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{{ error }}</div>
      </div>
    </template>
  </WizardSheet>
</template>

<script setup lang="ts">
import { reactive, computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { appApi } from '@/api/resources'
import { notifySuccess, notifyError } from '@/lib/notify'
import { useWizardSheet } from '@/console/composables/useWizardSheet'
import { useInstanceRoutes } from '@/console/composables/useInstanceRoutes'
import WizardSheet from '@/console/components/WizardSheet.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Globe, Smartphone, Server, Chrome } from 'lucide-vue-next'

defineProps<{ open: boolean }>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  created: []
}>()

const steps = [
  { title: 'Application Type', description: 'Choose your app type' },
  { title: 'Application Details', description: 'Configure settings' },
  { title: 'Confirmation', description: 'Review and create' },
]

const { currentStep, submitting, next, prev, reset } = useWizardSheet(steps.length)
const router = useRouter()
const { resolveRoute } = useInstanceRoutes()
const error = ref('')

const form = reactive({
  name: '',
  app_type: 'web',
})

const appTypes = [
  { value: 'web', label: 'Web Application', description: 'Single-page app or traditional web application', icon: Globe, tags: ['PKCE', 'Code Flow'] },
  { value: 'native', label: 'Native / Mobile', description: 'iOS, Android, or desktop application', icon: Smartphone, tags: ['PKCE'] },
  { value: 'api', label: 'API / Machine-to-Machine', description: 'Backend service or API client', icon: Server, tags: ['Client Credentials'] },
  { value: 'browser_extension', label: 'Browser Extension', description: 'Chrome, Firefox, or other browser extension', icon: Chrome, tags: ['PKCE'] },
]

const selectedTypeLabel = computed(() => appTypes.find(t => t.value === form.app_type)?.label || '')
const canProceed = computed(() => {
  if (currentStep.value === 0) return !!form.app_type
  if (currentStep.value === 1) return form.name.trim().length > 0
  return true
})

function buildClientId(name: string) {
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').replace(/-+/g, '-')
  return `${slug || 'app'}-${Date.now().toString(36)}`
}

async function onNext() {
  if (currentStep.value < steps.length - 1) {
    next()
    return
  }
  submitting.value = true
  error.value = ''
  try {
    const created = await appApi.create({
      name: form.name.trim(),
      app_type: form.app_type,
      client_id: buildClientId(form.name.trim()),
    })
    notifySuccess('Application created', `${form.name} is ready.`)
    emit('update:open', false)
    emit('created')
    reset()
    router.push(resolveRoute(`/applications/${created.id}`))
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create application'
    notifyError('Failed to create application', e)
  } finally {
    submitting.value = false
  }
}
</script>
