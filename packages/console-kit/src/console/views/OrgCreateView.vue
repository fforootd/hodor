<template>
  <WizardSheet
    :open="open"
    title="Create Organization"
    description="Set up a new organization"
    :steps="steps"
    :current-step="currentStep"
    :can-proceed="canProceed"
    :submitting="submitting"
    submit-label="Create Organization"
    @update:open="$emit('update:open', $event)"
    @next="onNext"
    @prev="prev"
  >
    <!-- Step 0: Organization Details -->
    <template #step-0>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Organization Information</h3>
          <p class="text-sm text-muted-foreground">Enter basic details about the organization</p>
        </div>
        <div class="space-y-2">
          <Label for="org-name">Organization Name</Label>
          <Input id="org-name" v-model="form.name" placeholder="Acme Corporation" />
          <p class="text-xs text-muted-foreground">This will be displayed across the platform</p>
        </div>
        <div class="space-y-2">
          <Label for="org-description">Description <span class="text-muted-foreground font-normal">(Optional)</span></Label>
          <Input id="org-description" v-model="form.description" placeholder="A brief description of your organization..." />
        </div>
        <div class="space-y-3">
          <Label>Organization Type</Label>
          <RadioGroup v-model="form.org_type" class="grid gap-2">
            <label
              v-for="t in orgTypes"
              :key="t.value"
              class="flex items-center gap-3 rounded-lg border p-3.5 cursor-pointer transition-colors hover:bg-muted/50"
              :class="form.org_type === t.value ? 'border-primary bg-primary/5' : ''"
            >
              <RadioGroupItem :value="t.value" />
              <component :is="t.icon" class="size-4 text-muted-foreground shrink-0" />
              <div>
                <div class="text-sm font-medium">{{ t.label }}</div>
                <div class="text-xs text-muted-foreground">{{ t.description }}</div>
              </div>
            </label>
          </RadioGroup>
        </div>
      </div>
    </template>

    <!-- Step 1: Confirmation -->
    <template #step-1>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Review Organization</h3>
        </div>
        <div class="rounded-lg border overflow-hidden text-sm">
          <div class="grid grid-cols-[1fr_auto] p-3 border-b bg-muted/20">
            <span class="text-muted-foreground">Name</span>
            <span class="font-medium text-right">{{ form.name || '—' }}</span>
          </div>
          <div v-if="form.description" class="grid grid-cols-[1fr_auto] p-3 border-b">
            <span class="text-muted-foreground">Description</span>
            <span class="font-medium text-right max-w-[200px] truncate">{{ form.description }}</span>
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
import { orgApi } from '@/api/resources'
import { notifySuccess, notifyError } from '@/lib/notify'
import { useWizardSheet } from '@/console/composables/useWizardSheet'
import { useInstanceRoutes } from '@/console/composables/useInstanceRoutes'
import WizardSheet from '@/console/components/WizardSheet.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Building2, UsersRound, User } from 'lucide-vue-next'

defineProps<{ open: boolean }>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  created: []
}>()

const steps = [
  { title: 'Organization Details', description: 'Basic information' },
  { title: 'Confirmation', description: 'Review and create' },
]

const { currentStep, submitting, next, prev, reset } = useWizardSheet(steps.length)
const router = useRouter()
const { resolveRoute } = useInstanceRoutes()
const error = ref('')

const form = reactive({
  name: '',
  description: '',
  org_type: 'company',
})

const orgTypes = [
  { value: 'company', label: 'Company', description: 'For business organizations', icon: Building2 },
  { value: 'team', label: 'Team', description: 'For departments or teams', icon: UsersRound },
  { value: 'personal', label: 'Personal', description: 'For individual use', icon: User },
]

const selectedTypeLabel = computed(() => orgTypes.find(t => t.value === form.org_type)?.label || '')
const canProceed = computed(() => {
  if (currentStep.value === 0) return form.name.trim().length > 0
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
    const created = await orgApi.create({ name: form.name.trim() })
    notifySuccess('Organization created', `${form.name} is ready.`)
    emit('update:open', false)
    emit('created')
    reset()
    router.push(resolveRoute(`/orgs/${created.id}`))
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create organization'
    notifyError('Failed to create organization', e)
  } finally {
    submitting.value = false
  }
}
</script>
