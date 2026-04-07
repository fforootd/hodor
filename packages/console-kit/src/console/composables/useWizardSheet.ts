import { ref, computed, type Ref, toValue, type MaybeRefOrGetter } from 'vue'

export interface WizardStep {
  title: string
  description?: string
}

export function useWizardSheet(stepCount: MaybeRefOrGetter<number>) {
  const currentStep = ref(0)
  const submitting = ref(false)

  const isFirstStep = computed(() => currentStep.value === 0)
  const isLastStep = computed(() => currentStep.value === toValue(stepCount) - 1)

  function next() {
    if (!isLastStep.value) currentStep.value++
  }

  function prev() {
    if (!isFirstStep.value) currentStep.value--
  }

  function reset() {
    currentStep.value = 0
    submitting.value = false
  }

  return { currentStep, submitting, isFirstStep, isLastStep, next, prev, reset }
}
