<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold tracking-tight">Create {{ label }}</h2>
        <button
          class="rounded-sm opacity-70 hover:opacity-100 transition-opacity text-[var(--color-muted-foreground)]"
          @click="onCancel"
        >✕</button>
      </div>

      <!-- Step Indicator -->
      <div class="flex items-center gap-2 text-xs text-[var(--color-muted-foreground)]">
        <span
          v-for="(step, i) in steps"
          :key="i"
          :class="[
            'inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border transition-colors',
            i === currentStep
              ? 'border-[var(--color-primary)] bg-[var(--color-primary)] text-[var(--color-primary-foreground)]'
              : i < currentStep
                ? 'border-green-300 bg-green-50 text-green-700'
                : 'border-[var(--color-border)]'
          ]"
        >
          <span v-if="i < currentStep">✓</span>
          <span v-else>{{ i + 1 }}</span>
          {{ step.title }}
        </span>
      </div>

      <!-- Step 0: Schema Type -->
      <div v-if="currentStep === 0" class="space-y-4">
        <div>
          <label class="text-sm font-medium">Schema Type</label>
          <p class="text-xs text-[var(--color-muted-foreground)] mt-0.5">Select the type of identity to create</p>
        </div>
        <div v-if="loadingSchemas" class="text-sm text-[var(--color-muted-foreground)] py-4">Loading schemas…</div>
        <div v-else class="space-y-2">
          <div
            v-for="s in schemas"
            :key="s.id"
            :class="[
              'rounded-lg border p-3 cursor-pointer transition-colors hover:bg-[var(--color-muted)]',
              selectedSchemaId === s.id ? 'border-[var(--color-primary)] bg-[var(--color-primary)]/5' : 'border-[var(--color-border)]'
            ]"
            @click="selectSchema(s)"
          >
            <div class="text-sm font-medium">{{ s.type }}</div>
            <div class="text-xs text-[var(--color-muted-foreground)]">v{{ s.version }}{{ s.is_default ? ' (default)' : '' }}</div>
          </div>
        </div>
      </div>

      <!-- Step 1: Profile Fields -->
      <div v-if="currentStep === 1" class="space-y-4">
        <div>
          <label class="text-sm font-medium">Profile Information</label>
          <p class="text-xs text-[var(--color-muted-foreground)] mt-0.5">Fields from <strong>{{ selectedSchema?.type }}</strong> schema</p>
        </div>
        <div v-if="loadingFields" class="text-sm text-[var(--color-muted-foreground)] py-4">Loading fields…</div>
        <div v-else class="space-y-3">
          <div v-for="field in schemaFields" :key="field.name" class="space-y-1.5">
            <label class="text-sm font-medium flex items-center gap-1.5">
              {{ field.label }}
              <span v-if="field.required" class="text-red-500 text-xs">*</span>
            </label>
            <!-- Boolean -->
            <select
              v-if="field.type === 'boolean'"
              :value="profileData[field.name] || ''"
              @change="profileData[field.name] = ($event.target as HTMLSelectElement).value"
              class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm"
            >
              <option value="">—</option>
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
            <!-- Text/email/number -->
            <input
              v-else
              :value="profileData[field.name] || ''"
              :type="field.inputType || 'text'"
              :placeholder="field.description || ''"
              @input="profileData[field.name] = ($event.target as HTMLInputElement).value"
              class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
            <p v-if="field.description" class="text-xs text-[var(--color-muted-foreground)]">{{ field.description }}</p>
          </div>
        </div>
      </div>

      <!-- Step 2: Confirmation -->
      <div v-if="currentStep === 2" class="space-y-4">
        <h3 class="text-sm font-medium">Review</h3>
        <div class="rounded-lg border border-[var(--color-border)] overflow-hidden">
          <div class="grid grid-cols-[1fr_auto] p-3 border-b bg-[var(--color-muted)] text-sm">
            <span class="text-[var(--color-muted-foreground)]">Schema</span>
            <span class="font-medium">{{ selectedSchema?.type }} v{{ selectedSchema?.version }}</span>
          </div>
          <div
            v-for="(val, key) in filledFields"
            :key="key"
            class="grid grid-cols-[1fr_auto] p-3 border-b text-sm"
          >
            <span class="text-[var(--color-muted-foreground)] capitalize">{{ String(key).replace(/_/g, ' ') }}</span>
            <span class="font-medium text-right max-w-[200px] truncate">{{ val }}</span>
          </div>
        </div>

        <div v-if="createError" class="p-3 bg-red-50 text-red-700 text-sm rounded-md border border-red-200">
          {{ createError }}
        </div>
      </div>

      <!-- Navigation -->
      <div class="flex items-center justify-between pt-2 border-t border-[var(--color-border)]">
        <button
          v-if="currentStep > 0"
          class="inline-flex items-center rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm hover:bg-[var(--color-muted)] transition-colors"
          @click="currentStep--"
        >Back</button>
        <div v-else></div>
        <button
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50"
          :disabled="!canProceed || creating"
          @click="onNext"
        >{{ currentStep === steps.length - 1 ? (creating ? 'Creating…' : 'Create') : 'Continue' }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-identity-create'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  schemaType?: string
  orgId?: string
  darkMode?: string
  mode?: string
}>(), {
  apiBaseUrl: '',
  schemaType: '',
  orgId: '',
  darkMode: '',
  mode: 'wizard',
})

const isDark = computed(() => isDarkMode(props.darkMode))

let api: WCApiClient

const schemas = ref<any[]>([])
const selectedSchemaId = ref('')
const selectedSchema = ref<any>(null)
const schemaFields = ref<any[]>([])
const profileData = reactive<Record<string, string>>({})
const loadingSchemas = ref(false)
const loadingFields = ref(false)
const creating = ref(false)
const createError = ref('')
const currentStep = ref(0)

const steps = computed(() => {
  const s = []
  if (!props.schemaType) s.push({ title: 'Schema' })
  else s.push({ title: 'Schema' })
  s.push({ title: 'Profile' })
  s.push({ title: 'Create' })
  return s
})

const label = computed(() => {
  if (props.schemaType) return props.schemaType.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase())
  return 'Identity'
})

const filledFields = computed(() => {
  const filled: Record<string, string> = {}
  for (const [k, v] of Object.entries(profileData)) {
    if (v) filled[k] = v
  }
  return filled
})

const canProceed = computed(() => {
  if (currentStep.value === 0) return !!selectedSchemaId.value
  if (currentStep.value === 1) {
    // Check required fields
    for (const f of schemaFields.value) {
      if (f.required && !profileData[f.name]) return false
    }
    return true
  }
  return true
})

function extractFields(schema: any): any[] {
  const fields: any[] = []
  const props = schema?.schema?.properties
  if (!props) return fields

  const required = schema?.schema?.required || []
  for (const [name, def] of Object.entries(props) as [string, any][]) {
    if (name.startsWith('$') || name === 'id') continue
    fields.push({
      name,
      label: def.title || name.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()),
      type: def.type || 'string',
      inputType: def.format === 'email' ? 'email' : def.type === 'number' ? 'number' : 'text',
      description: def.description || '',
      required: required.includes(name),
      enum: def.enum || null,
    })
  }
  return fields
}

async function selectSchema(s: any) {
  selectedSchemaId.value = s.id
  selectedSchema.value = s
  loadingFields.value = true
  try {
    const full = await api.get<any>(`/v1/schemas/${s.id}`)
    schemaFields.value = extractFields(full)
  } catch {
    schemaFields.value = []
  } finally {
    loadingFields.value = false
  }
}

async function onNext() {
  if (currentStep.value < steps.value.length - 1) {
    currentStep.value++
  } else {
    // Create
    creating.value = true
    createError.value = ''
    try {
      const body: Record<string, any> = {
        schema_id: selectedSchemaId.value,
        profile: { ...profileData },
      }
      if (props.orgId) body.org_ids = [props.orgId]

      const result = await api.post<any>('/v1/users', body)
      dispatchWCEvent(TAG_NAME, 'identity-created', {
        id: result.id,
        identifier: result.identifier,
      })
    } catch (e: any) {
      createError.value = e?.message || 'Failed to create identity'
      dispatchWCEvent(TAG_NAME, 'create-error', { error: createError.value })
    } finally {
      creating.value = false
    }
  }
}

function onCancel() {
  dispatchWCEvent(TAG_NAME, 'create-cancelled')
}

onMounted(async () => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  loadingSchemas.value = true
  try {
    const data = await api.get<any>('/v1/schemas')
    schemas.value = data.items || []
    // Auto-select if schemaType prop is set
    if (props.schemaType) {
      const match = schemas.value.find(s => s.type === props.schemaType && s.is_default)
        || schemas.value.find(s => s.type === props.schemaType)
      if (match) {
        await selectSchema(match)
        currentStep.value = 1 // Skip schema selection
      }
    }
  } catch {
    schemas.value = []
  } finally {
    loadingSchemas.value = false
  }
})
</script>

<style>
:host {
  display: block;
  font-family: 'Inter', ui-sans-serif, system-ui, -apple-system, sans-serif;
  --color-background: hsl(0 0% 100%);
  --color-foreground: hsl(240 10% 3.9%);
  --color-card: hsl(0 0% 100%);
  --color-card-foreground: hsl(240 10% 3.9%);
  --color-primary: hsl(240 5.9% 10%);
  --color-primary-foreground: hsl(0 0% 98%);
  --color-secondary: hsl(240 4.8% 95.9%);
  --color-secondary-foreground: hsl(240 5.9% 10%);
  --color-muted: hsl(240 4.8% 95.9%);
  --color-muted-foreground: hsl(240 3.8% 46.1%);
  --color-accent: hsl(240 4.8% 95.9%);
  --color-accent-foreground: hsl(240 5.9% 10%);
  --color-destructive: hsl(0 84.2% 60.2%);
  --color-destructive-foreground: hsl(0 0% 98%);
  --color-border: hsl(240 5.9% 90%);
  --color-input: hsl(240 5.9% 90%);
  --color-ring: hsl(240 5.9% 10%);
  --radius: 0.5rem;
}
:host(.dark) {
  --color-background: hsl(240 10% 3.9%);
  --color-foreground: hsl(0 0% 98%);
  --color-card: hsl(240 10% 3.9%);
  --color-card-foreground: hsl(0 0% 98%);
  --color-primary: hsl(0 0% 98%);
  --color-primary-foreground: hsl(240 5.9% 10%);
  --color-secondary: hsl(240 3.7% 15.9%);
  --color-secondary-foreground: hsl(0 0% 98%);
  --color-muted: hsl(240 3.7% 15.9%);
  --color-muted-foreground: hsl(240 5% 64.9%);
  --color-accent: hsl(240 3.7% 15.9%);
  --color-accent-foreground: hsl(0 0% 98%);
  --color-destructive: hsl(0 62.8% 30.6%);
  --color-destructive-foreground: hsl(0 0% 98%);
  --color-border: hsl(240 3.7% 15.9%);
  --color-input: hsl(240 3.7% 15.9%);
  --color-ring: hsl(240 4.9% 83.9%);
}
.zitadel-wc {
  color: var(--color-foreground);
  background: var(--color-background);
  padding: 1rem;
}
.zitadel-wc.dark { color-scheme: dark; }
</style>
