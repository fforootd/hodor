<template>
  <Sheet :open="open" @update:open="$emit('update:open', $event)">
    <SheetContent class="sm:max-w-2xl overflow-y-auto" side="right">
      <SheetHeader>
        <SheetTitle>{{ title }}</SheetTitle>
        <SheetDescription v-if="description">{{ description }}</SheetDescription>
      </SheetHeader>

      <div class="flex-1 px-4 space-y-4">
        <SchemaTabsEditor
          v-if="schemaContext.schema"
          v-model="formData"
          :schema="schemaContext.schema"
          :curl-snippets="curlSnippets"
          :form-title="formTitle || `${resourceLabel} Fields`"
          @update:json-valid="(v) => jsonValid = v"
        />

        <div v-else class="flex h-32 items-center justify-center text-sm text-muted-foreground">
          Loading schema…
        </div>

        <div
          v-if="error"
          role="alert"
          class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        >
          {{ error }}
        </div>
      </div>

      <SheetFooter class="flex-row justify-end px-4">
        <Button variant="outline" @click="$emit('update:open', false)" :disabled="submitting">
          Cancel
        </Button>
        <Button :disabled="submitting || !jsonValid" @click="submit">
          {{ submitting ? 'Creating…' : `Create ${resourceLabel}` }}
        </Button>
      </SheetFooter>

      <p
        v-if="!jsonValid && schemaContext.schema"
        class="px-4 pb-4 text-xs text-muted-foreground"
      >
        Fix validation errors above to continue.
      </p>
    </SheetContent>
  </Sheet>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
  type SchemaResourceType,
} from '@/console/utils/schema-resource'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'

const props = withDefaults(defineProps<{
  open: boolean
  title: string
  description?: string
  schemaType: SchemaResourceType
  apiPath: string
  resourceLabel: string
  formTitle?: string
  defaultFormData?: Record<string, any>
  includeOrgHeader?: boolean
  createFn: (payload: any) => Promise<{ id: string }>
}>(), {
  includeOrgHeader: false,
  defaultFormData: () => ({}),
})

const emit = defineEmits<{
  'update:open': [value: boolean]
  created: [id: string]
}>()

const { currentOrgId } = useOrgContext()

const submitting = ref(false)
const error = ref('')
const jsonValid = ref(true)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: props.schemaType,
  versions: [],
})

const payload = computed(() =>
  buildResourceWriteBody(props.schemaType, schemaContext.value.schemaId, normalizeResourceData(formData.value)),
)
const curlSnippets = computed(() =>
  buildCurlSnippets({
    path: props.apiPath,
    body: payload.value,
    includeOrgHeader: props.includeOrgHeader,
    orgId: currentOrgId.value,
    methods: ['POST'],
  }),
)

// Load schema and reset form when sheet opens
watch(() => props.open, async (isOpen) => {
  if (!isOpen) return
  error.value = ''
  jsonValid.value = true
  formData.value = { ...props.defaultFormData }
  schemaContext.value = {
    display: {},
    schema: null,
    schemaId: '',
    schemaType: props.schemaType,
    versions: [],
  }
  schemaContext.value = await loadResourceSchemaContext(props.schemaType)
})

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const created = await props.createFn(payload.value)
    emit('created', created.id)
    emit('update:open', false)
  } catch (err: any) {
    error.value = err?.message || `Failed to create ${props.resourceLabel.toLowerCase()}`
  } finally {
    submitting.value = false
  }
}
</script>
