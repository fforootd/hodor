<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-5">
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-[var(--color-border)] pb-4">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Create Organization</h2>
          <p class="text-sm text-[var(--color-muted-foreground)]">Set up a new organization for your project</p>
        </div>
        <button class="btn-ghost" @click="onCancel">✕</button>
      </div>

      <!-- Form -->
      <div class="space-y-4">
        <div class="space-y-1.5">
          <label class="label">Name <span class="text-red-500">*</span></label>
          <input
            v-model="formData.name"
            type="text"
            placeholder="e.g. Acme Corporation"
            class="input-field"
            @keyup.enter="onSubmit"
          />
          <p class="hint">A unique name for your organization.</p>
        </div>

        <div class="space-y-1.5">
          <label class="label">Description</label>
          <textarea
            v-model="formData.description"
            rows="3"
            placeholder="Optional description…"
            class="input-field textarea"
          />
        </div>

        <div class="space-y-1.5">
          <label class="label">Metadata</label>
          <p class="hint">Optional JSON metadata for this organization.</p>
          <textarea
            v-model="metadataRaw"
            rows="4"
            placeholder='{ "department": "engineering" }'
            class="input-field textarea font-mono text-xs"
          />
          <p v-if="metadataError" class="text-xs text-red-500">{{ metadataError }}</p>
        </div>
      </div>

      <!-- Error -->
      <div v-if="createError" class="error-banner">{{ createError }}</div>

      <!-- Actions -->
      <div class="flex items-center justify-end gap-3 pt-3 border-t border-[var(--color-border)]">
        <button class="btn-outline" @click="onCancel">Cancel</button>
        <button
          class="btn-primary"
          :disabled="!canSubmit || creating"
          @click="onSubmit"
        >{{ creating ? 'Creating…' : 'Create Organization' }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-org-create'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  darkMode?: string
}>(), {
  apiBaseUrl: '',
  darkMode: '',
})

const isDark = computed(() => isDarkMode(props.darkMode))

let api: WCApiClient

const formData = reactive({
  name: '',
  description: '',
})
const metadataRaw = ref('')
const metadataError = ref('')
const creating = ref(false)
const createError = ref('')

const canSubmit = computed(() => {
  return formData.name.trim().length > 0 && !metadataError.value
})

function parseMetadata(): Record<string, any> | null {
  const raw = metadataRaw.value.trim()
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw)
    metadataError.value = ''
    return parsed
  } catch {
    metadataError.value = 'Invalid JSON'
    return null
  }
}

async function onSubmit() {
  if (!canSubmit.value || creating.value) return

  const metadata = metadataRaw.value.trim() ? parseMetadata() : undefined
  if (metadataRaw.value.trim() && metadata === null) return

  creating.value = true
  createError.value = ''

  try {
    const body: Record<string, any> = {
      name: formData.name.trim(),
    }
    if (metadata) {
      body.metadata = metadata
    }
    const result = await api.post<any>('/v1/orgs', body)
    dispatchWCEvent(TAG_NAME, 'org-created', {
      id: result.id,
      name: result.name,
    })
  } catch (e: any) {
    createError.value = e?.message || 'Failed to create organization'
    dispatchWCEvent(TAG_NAME, 'org-error', { error: createError.value })
  } finally {
    creating.value = false
  }
}

function onCancel() {
  dispatchWCEvent(TAG_NAME, 'create-cancelled')
}

onMounted(() => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
})
</script>

<style>
:host {
  display: block;
  font-family: 'Inter', ui-sans-serif, system-ui, -apple-system, sans-serif;
  --color-background: hsl(0 0% 100%);
  --color-foreground: hsl(240 10% 3.9%);
  --color-card: hsl(0 0% 100%);
  --color-primary: hsl(240 5.9% 10%);
  --color-primary-foreground: hsl(0 0% 98%);
  --color-muted: hsl(240 4.8% 95.9%);
  --color-muted-foreground: hsl(240 3.8% 46.1%);
  --color-border: hsl(240 5.9% 90%);
  --color-input: hsl(240 5.9% 90%);
  --color-ring: hsl(240 5.9% 10%);
  --radius: 0.5rem;
}
:host(.dark) {
  --color-background: hsl(240 10% 3.9%);
  --color-foreground: hsl(0 0% 98%);
  --color-card: hsl(240 10% 6%);
  --color-primary: hsl(0 0% 98%);
  --color-primary-foreground: hsl(240 5.9% 10%);
  --color-muted: hsl(240 3.7% 15.9%);
  --color-muted-foreground: hsl(240 5% 64.9%);
  --color-border: hsl(240 3.7% 15.9%);
  --color-input: hsl(240 3.7% 15.9%);
  --color-ring: hsl(240 4.9% 83.9%);
}
.zitadel-wc { color: var(--color-foreground); background: var(--color-background); padding: 1.25rem; }
.zitadel-wc.dark { color-scheme: dark; }

.label { display: block; font-size: 0.875rem; font-weight: 500; }
.hint { font-size: 0.75rem; color: var(--color-muted-foreground); margin-top: 0.25rem; }

.input-field {
  width: 100%; height: 2.25rem; border-radius: 0.375rem;
  border: 1px solid var(--color-input); background: var(--color-background);
  padding: 0.25rem 0.75rem; font-size: 0.875rem;
  box-shadow: 0 1px 2px rgba(0,0,0,0.05);
  color: var(--color-foreground);
  transition: border-color 0.15s, box-shadow 0.15s;
  outline: none; box-sizing: border-box;
}
.input-field::placeholder { color: var(--color-muted-foreground); }
.input-field:focus { border-color: var(--color-ring); box-shadow: 0 0 0 2px var(--color-ring); }
.textarea { height: auto; padding: 0.5rem 0.75rem; resize: vertical; font-family: inherit; }

.btn-primary {
  display: inline-flex; align-items: center; justify-content: center;
  border-radius: 0.375rem; font-size: 0.875rem; font-weight: 500;
  height: 2.25rem; padding: 0.25rem 1rem;
  background: var(--color-primary); color: var(--color-primary-foreground);
  border: none; cursor: pointer;
  transition: opacity 0.15s, transform 0.1s;
}
.btn-primary:hover { opacity: 0.9; }
.btn-primary:active { transform: scale(0.98); }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }

.btn-outline {
  display: inline-flex; align-items: center; justify-content: center;
  border-radius: 0.375rem; font-size: 0.875rem; font-weight: 500;
  height: 2.25rem; padding: 0.25rem 1rem;
  background: transparent; color: var(--color-foreground);
  border: 1px solid var(--color-border); cursor: pointer;
  transition: background-color 0.15s;
}
.btn-outline:hover { background: var(--color-muted); }

.btn-ghost {
  display: inline-flex; align-items: center; justify-content: center;
  width: 2rem; height: 2rem; border-radius: 0.375rem;
  background: transparent; border: none; cursor: pointer;
  color: var(--color-muted-foreground); font-size: 1rem;
  transition: opacity 0.15s, background-color 0.15s;
}
.btn-ghost:hover { opacity: 1; background: var(--color-muted); }

.error-banner {
  border-radius: 0.375rem; border: 1px solid hsl(0 60% 80%);
  background: hsl(0 80% 96%); padding: 0.5rem 0.75rem;
  font-size: 0.75rem; color: hsl(0 60% 40%);
}
:host(.dark) .error-banner { background: hsl(0 40% 15%); border-color: hsl(0 40% 30%); color: hsl(0 70% 75%); }

.space-y-1\.5 > * + * { margin-top: 0.375rem; }
.space-y-3 > * + * { margin-top: 0.75rem; }
.space-y-4 > * + * { margin-top: 1rem; }
.space-y-5 > * + * { margin-top: 1.25rem; }
</style>
