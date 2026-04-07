<template>
  <WizardSheet
    :open="open"
    title="Create Project"
    description="Set up a new project"
    :steps="steps"
    :current-step="currentStep"
    :can-proceed="canProceed"
    :submitting="submitting"
    submit-label="Create Project"
    @update:open="$emit('update:open', $event)"
    @next="onNext"
    @prev="prev"
  >
    <!-- Step 0: Project Details -->
    <template #step-0>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Project Information</h3>
          <p class="text-sm text-muted-foreground">Enter basic details about your project</p>
        </div>
        <div class="space-y-2">
          <Label for="project-name">Project Name</Label>
          <Input id="project-name" v-model="form.name" placeholder="My Project" />
          <p class="text-xs text-muted-foreground">Used to identify this project across the platform</p>
        </div>
        <div class="space-y-2">
          <Label for="project-description">Description <span class="text-muted-foreground font-normal">(Optional)</span></Label>
          <Input id="project-description" v-model="form.description" placeholder="A brief description of your project..." />
        </div>
        <div v-if="currentOrgName" class="rounded-lg border p-3.5">
          <div class="text-sm font-medium">Organization</div>
          <p class="text-xs text-muted-foreground mt-0.5">Project will be created in <strong>{{ currentOrgName }}</strong></p>
        </div>
      </div>
    </template>

    <!-- Step 1: Confirmation -->
    <template #step-1>
      <div class="space-y-5">
        <div>
          <h3 class="text-base font-medium mb-1">Review Project</h3>
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
          <div v-if="currentOrgName" class="grid grid-cols-[1fr_auto] p-3">
            <span class="text-muted-foreground">Organization</span>
            <span class="font-medium text-right">{{ currentOrgName }}</span>
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
import { projectApi } from '@/api/resources'
import { notifySuccess, notifyError } from '@/lib/notify'
import { useWizardSheet } from '@/console/composables/useWizardSheet'
import { useInstanceRoutes } from '@/console/composables/useInstanceRoutes'
import { useOrgContext } from '@/console/composables/useOrgContext'
import WizardSheet from '@/console/components/WizardSheet.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const props = withDefaults(defineProps<{ open?: boolean }>(), { open: true })
const emit = defineEmits<{
  'update:open': [value: boolean]
  created: []
}>()

const steps = [
  { title: 'Project Details', description: 'Basic information' },
  { title: 'Confirmation', description: 'Review and create' },
]

const { currentStep, submitting, next, prev, reset } = useWizardSheet(steps.length)
const router = useRouter()
const { resolveRoute } = useInstanceRoutes()
const { currentOrgId } = useOrgContext()
const error = ref('')

const form = reactive({
  name: '',
  description: '',
})

const currentOrgName = computed(() => currentOrgId.value || '')
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
    const created = await projectApi.create({ name: form.name.trim() })
    notifySuccess('Project created', `${form.name} is ready.`)
    emit('update:open', false)
    emit('created')
    reset()
    router.push(resolveRoute(`/projects/${created.id}`))
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create project'
    notifyError('Failed to create project', e)
  } finally {
    submitting.value = false
  }
}
</script>
