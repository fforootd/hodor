<template>
  <Dialog :open="open" @update:open="$emit('update:open', $event)">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Download class="size-5 text-primary" />
          Install {{ detail?.template?.name || templateId }}
        </DialogTitle>
        <DialogDescription v-if="detail?.template?.description">
          {{ detail.template.description }}
        </DialogDescription>
      </DialogHeader>

      <!-- Loading -->
      <div v-if="loading" class="flex h-32 items-center justify-center">
        <Spinner class="mr-2" /> Loading template…
      </div>

      <!-- Error -->
      <div v-else-if="error" class="flex h-32 items-center justify-center text-destructive text-sm">
        {{ error }}
      </div>

      <!-- Main form -->
      <div v-else-if="detail" class="space-y-4">
        <!-- Template meta -->
        <div class="flex items-center gap-2 flex-wrap">
          <Badge variant="secondary" class="text-xs">{{ detail.template.type }}</Badge>
          <Badge variant="outline" class="text-xs font-mono">v{{ detail.template.version }}</Badge>
          <Badge v-for="tag in detail.template.tags" :key="tag" variant="outline" class="text-xs">
            {{ tag }}
          </Badge>
        </div>

        <!-- Variable inputs -->
        <div v-if="variableKeys.length > 0" class="space-y-3">
          <h4 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider">Configuration</h4>
          <div v-for="key in variableKeys" :key="key" class="space-y-1.5">
            <Label :for="`var-${key}`" class="text-sm font-medium">
              {{ formatLabel(key) }}
              <span v-if="detail.variables[key].description" class="font-normal text-muted-foreground ml-1">
                — {{ detail.variables[key].description }}
              </span>
            </Label>

            <!-- Boolean -->
            <div v-if="detail.variables[key].type === 'boolean'" class="flex items-center gap-2">
              <Switch
                :id="`var-${key}`"
                :checked="formValues[key] ?? detail.variables[key].default ?? false"
                @update:checked="formValues[key] = $event"
              />
              <span class="text-sm text-muted-foreground">{{ formValues[key] ? 'Enabled' : 'Disabled' }}</span>
            </div>

            <!-- Integer -->
            <Input
              v-else-if="detail.variables[key].type === 'integer'"
              :id="`var-${key}`"
              type="number"
              :model-value="formValues[key] ?? detail.variables[key].default ?? 0"
              @update:model-value="formValues[key] = Number($event)"
              class="h-9"
            />

            <!-- String (default) -->
            <Input
              v-else
              :id="`var-${key}`"
              :model-value="formValues[key] ?? detail.variables[key].default ?? ''"
              @update:model-value="formValues[key] = $event"
              :placeholder="String(detail.variables[key].default || '')"
              class="h-9"
            />
          </div>
        </div>

        <!-- No variables -->
        <p v-else class="text-sm text-muted-foreground py-2">
          This template requires no configuration.
        </p>

        <!-- Preview toggle -->
        <Collapsible>
          <CollapsibleTrigger class="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer hover:text-foreground transition-colors">
            <ChevronRight class="size-3 transition-transform data-[state=open]:rotate-90" />
            Preview resolved payload
          </CollapsibleTrigger>
          <CollapsibleContent>
            <pre class="mt-2 rounded-md border bg-muted/30 p-3 text-xs font-mono overflow-auto max-h-48">{{ JSON.stringify(detail.payload, null, 2) }}</pre>
          </CollapsibleContent>
        </Collapsible>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="$emit('update:open', false)" :disabled="installing">
          Cancel
        </Button>
        <Button @click="install" :disabled="installing || loading || !!error">
          <Spinner v-if="installing" class="mr-1.5 size-3.5" />
          {{ installing ? 'Installing…' : 'Install' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { toast } from 'vue-sonner'
import { catalogApi, type CatalogTemplateDetail } from '@/api/resources'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Spinner } from '@/components/ui/spinner'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Download, ChevronRight } from 'lucide-vue-next'

const props = defineProps<{
  open: boolean
  templateId: string
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  installed: [result: { id: string; template_id: string; type: string }]
}>()

const detail = ref<CatalogTemplateDetail | null>(null)
const loading = ref(false)
const error = ref('')
const installing = ref(false)
const formValues = ref<Record<string, any>>({})

const variableKeys = computed(() =>
  detail.value?.variables ? Object.keys(detail.value.variables) : []
)

function formatLabel(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

// Fetch template details when opened
watch(() => props.open, async (isOpen) => {
  if (!isOpen || !props.templateId) return
  loading.value = true
  error.value = ''
  formValues.value = {}

  try {
    detail.value = await catalogApi.get(props.templateId)
    // Pre-fill defaults
    if (detail.value.variables) {
      for (const [key, v] of Object.entries(detail.value.variables)) {
        if (v.default !== undefined) {
          formValues.value[key] = v.default
        }
      }
    }
  } catch (e: any) {
    error.value = e.message || 'Failed to load template'
  } finally {
    loading.value = false
  }
}, { immediate: true })

async function install() {
  if (!props.templateId) return
  installing.value = true
  try {
    const result = await catalogApi.install(props.templateId, formValues.value)
    const templateType = detail.value?.template?.type || ''
    toast.success(`Installed "${detail.value?.template?.name || props.templateId}"`, {
      description: `Entity ${result.id} created`,
    })
    emit('installed', { ...result, type: templateType })
    emit('update:open', false)
  } catch (e: any) {
    toast.error('Install failed', { description: e.message })
  } finally {
    installing.value = false
  }
}
</script>
