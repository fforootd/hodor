<template>
  <WizardSheet
    :open="open"
    title="Create Instance"
    description="Create your deployment instance"
    :steps="steps"
    :current-step="currentStep"
    :can-proceed="canProceed"
    :submitting="submitting"
    submit-label="Create Instance"
    @update:open="$emit('update:open', $event)"
    @next="onNext"
    @prev="prev"
  >
    <!-- Step 0: Configuration -->
    <template #step-0>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Instance Details</h3>
          <p class="text-sm text-muted-foreground">Configure your instance name and region.</p>
        </div>
        <div class="space-y-2">
          <Label for="instance-name">Instance Name</Label>
          <Input
            id="instance-name"
            :model-value="form.instance_id"
            placeholder="my-project-prod"
            @update:model-value="onIdInput"
          />
          <p class="text-xs text-muted-foreground">A friendly name to identify your instance</p>
        </div>
        <div class="space-y-2">
          <Label for="subdomain">Subdomain</Label>
          <div class="flex">
            <Input
              id="subdomain"
              v-model="form.domain_prefix"
              placeholder="my-company"
              class="rounded-r-none"
            />
            <div class="flex items-center rounded-r-md border border-l-0 bg-muted px-3 text-sm text-muted-foreground">
              .zitadel.cloud
            </div>
          </div>
          <p class="text-xs text-muted-foreground">Your instance will be available at {{ form.domain_prefix || 'my-company' }}.zitadel.cloud</p>
        </div>
        <div class="space-y-2">
          <Label>Region</Label>
          <p class="text-xs text-muted-foreground mb-2">Select the region closest to your users</p>
          <RadioGroup v-model="form.region_key" class="grid grid-cols-2 gap-2">
            <label
              v-for="region in regions"
              :key="region.key"
              class="flex items-center gap-2 rounded-lg border px-3 py-2.5 cursor-pointer text-sm transition-colors hover:bg-muted/50"
              :class="form.region_key === region.key ? 'border-primary bg-primary/5' : ''"
            >
              <RadioGroupItem :value="region.key" />
              <span class="font-medium">{{ region.label }}</span>
            </label>
          </RadioGroup>
        </div>
      </div>
    </template>

    <!-- Step 1: Confirmation -->
    <template #step-1>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Review Configuration</h3>
        </div>
        <div class="rounded-lg border overflow-hidden text-sm">
          <div class="grid grid-cols-[1fr_auto] p-3 border-b bg-muted/20">
            <span class="text-muted-foreground">Instance Name</span>
            <span class="font-medium text-right">{{ form.instance_id || '—' }}</span>
          </div>
          <div class="grid grid-cols-[1fr_auto] p-3 border-b">
            <span class="text-muted-foreground">Domain</span>
            <span class="font-medium text-right">{{ fullDomain }}</span>
          </div>
          <div class="grid grid-cols-[1fr_auto] p-3">
            <span class="text-muted-foreground">Region</span>
            <span class="font-medium text-right">{{ selectedRegionLabel }}</span>
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
import { instanceApi } from '@/api/resources'
import { notifySuccess, notifyError } from '@/lib/notify'
import { useWizardSheet } from '@/console/composables/useWizardSheet'
import WizardSheet from '@/console/components/WizardSheet.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

const props = withDefaults(defineProps<{ open?: boolean }>(), { open: true })
const emit = defineEmits<{
  'update:open': [value: boolean]
  created: []
}>()

const steps = [
  { title: 'Configuration', description: 'Set up your instance' },
  { title: 'Confirmation', description: 'Review and create' },
]

const { currentStep, submitting, next, prev, reset } = useWizardSheet(steps.length)
const router = useRouter()
const error = ref('')

const form = reactive({
  instance_id: '',
  domain_prefix: '',
  region_key: 'eu-frankfurt',
})

const regions = [
  { key: 'eu-frankfurt', label: 'EU (Frankfurt)' },
  { key: 'us-virginia', label: 'US (Virginia)' },
  { key: 'us-oregon', label: 'US (Oregon)' },
  { key: 'asia-singapore', label: 'Asia (Singapore)' },
  { key: 'asia-tokyo', label: 'Asia (Tokyo)' },
  { key: 'au-sydney', label: 'Australia (Sydney)' },
]

function onIdInput(value: string | number) {
  const slugged = String(value)
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, '')
    .replace(/^-+/, '')
    .replace(/-+/g, '-')
  form.instance_id = slugged
  if (!form.domain_prefix || form.domain_prefix === slugged.slice(0, -1)) {
    form.domain_prefix = slugged
  }
}

const fullDomain = computed(() => form.domain_prefix ? `${form.domain_prefix}.zitadel.cloud` : '')
const selectedRegionLabel = computed(() => regions.find(r => r.key === form.region_key)?.label || '')

const canProceed = computed(() => {
  if (currentStep.value === 0) return form.instance_id.trim().length >= 1 && form.domain_prefix.trim().length > 0
  return true
})

async function onNext() {
  if (currentStep.value < steps.length - 1) {
    next()
    return
  }
  submitting.value = true
  error.value = ''
  try {
    await instanceApi.create({
      instance_id: form.instance_id,
      domain: fullDomain.value,
      placement_mode: form.region_key ? 'regional' : 'global',
      region_key: form.region_key || undefined,
      kind: 'managed',
    })
    notifySuccess('Instance created', `${fullDomain.value} is ready.`)
    emit('update:open', false)
    emit('created')
    reset()
    router.push('/instances')
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create instance'
    notifyError('Failed to create instance', e)
  } finally {
    submitting.value = false
  }
}
</script>
