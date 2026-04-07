<template>
  <Sheet :open="open" @update:open="$emit('update:open', $event)">
    <SheetContent side="right" class="sm:max-w-lg w-full p-0 flex flex-col">
      <!-- Header -->
      <div class="px-6 py-4 border-b shrink-0">
        <h2 class="text-lg font-semibold tracking-tight">{{ title }}</h2>
        <p v-if="description" class="text-sm text-muted-foreground mt-0.5">{{ description }}</p>
      </div>

      <!-- Body: stepper sidebar + content -->
      <div class="flex flex-1 overflow-hidden">
        <!-- Left Sidebar: Vertical Stepper -->
        <div class="w-52 bg-muted/30 border-r p-6 flex flex-col shrink-0 overflow-y-auto">
          <div class="relative">
            <!-- Connecting line -->
            <div
              class="absolute left-[11px] top-[14px] w-0.5 bg-border pointer-events-none"
              :style="{ height: `${(steps.length - 1) * 56}px` }"
            />

            <div
              v-for="(step, index) in steps"
              :key="index"
              class="relative flex items-start mb-6 last:mb-0"
            >
              <!-- Step circle -->
              <div
                class="relative z-10 size-6 rounded-full flex items-center justify-center text-xs font-medium shrink-0 transition-all duration-200"
                :class="stepCircleClass(index)"
              >
                <Check v-if="currentStep > index" class="size-3.5" />
                <ChevronRight v-else-if="currentStep === index" class="size-3.5" />
                <span v-else>{{ index + 1 }}</span>
              </div>

              <!-- Step label -->
              <div class="ml-3 min-w-0">
                <div
                  class="text-sm font-medium leading-tight"
                  :class="currentStep >= index ? 'text-foreground' : 'text-muted-foreground'"
                >
                  {{ step.title }}
                </div>
                <div
                  v-if="currentStep === index && step.description"
                  class="text-xs text-muted-foreground mt-0.5 truncate"
                >
                  {{ step.description }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Right Content Area -->
        <div class="flex-1 flex flex-col overflow-hidden">
          <div class="flex-1 overflow-y-auto p-6">
            <div
              v-for="(step, index) in steps"
              :key="index"
            >
              <div
                v-if="currentStep === index"
                class="animate-in fade-in slide-in-from-right-4 duration-200"
              >
                <slot :name="`step-${index}`" />
              </div>
            </div>
          </div>

          <!-- Bottom Actions -->
          <div class="p-4 border-t bg-muted/20 flex items-center justify-between shrink-0">
            <Button
              variant="outline"
              size="sm"
              :disabled="currentStep === 0 || submitting"
              @click="$emit('prev')"
            >
              Back
            </Button>
            <Button
              size="sm"
              :disabled="!canProceed || submitting"
              @click="$emit('next')"
            >
              <Loader2 v-if="submitting" class="size-4 mr-2 animate-spin" />
              {{ currentStep === steps.length - 1 ? (submitting ? 'Creating...' : submitLabel) : 'Continue' }}
            </Button>
          </div>
        </div>
      </div>
    </SheetContent>
  </Sheet>
</template>

<script setup lang="ts">
import { Sheet, SheetContent } from '@/components/ui/sheet'
import { Button } from '@/components/ui/button'
import { Check, ChevronRight, Loader2 } from 'lucide-vue-next'
import type { WizardStep } from '@/console/composables/useWizardSheet'

const props = defineProps<{
  open: boolean
  title: string
  description?: string
  steps: WizardStep[]
  currentStep: number
  canProceed: boolean
  submitting: boolean
  submitLabel: string
}>()

defineEmits<{
  'update:open': [value: boolean]
  next: []
  prev: []
}>()

function stepCircleClass(index: number) {
  if (props.currentStep > index) return 'bg-primary text-primary-foreground'
  if (props.currentStep === index) return 'bg-primary text-primary-foreground ring-4 ring-primary/20'
  return 'bg-muted text-muted-foreground border border-border'
}
</script>
