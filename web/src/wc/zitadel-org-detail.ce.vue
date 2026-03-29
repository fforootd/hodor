<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <!-- Loading -->
    <div v-if="loading" class="flex justify-center py-16 text-sm text-[var(--color-muted-foreground)]">
      <div class="space-y-3 w-full max-w-md">
        <div class="skeleton-row h-12" />
        <div class="skeleton-row h-8 w-3/4" />
        <div class="skeleton-row h-8 w-1/2" />
      </div>
    </div>

    <!-- Error -->
    <div v-if="error" class="error-banner">{{ error }}</div>

    <!-- Content -->
    <div v-if="!loading && org" class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-[var(--color-border)] pb-4">
        <div class="flex items-center gap-3">
          <div class="avatar-lg">{{ orgInitial }}</div>
          <div>
            <h2 class="text-lg font-semibold tracking-tight">{{ org.name }}</h2>
            <div class="flex items-center gap-2 text-sm text-[var(--color-muted-foreground)]">
              <span :class="['state-badge', org.state === 'active' ? 'state-active' : 'state-inactive']">
                {{ org.state || 'active' }}
              </span>
              <span class="text-xs">·</span>
              <span class="text-xs">Created {{ formatDate(org.created_at) }}</span>
            </div>
          </div>
        </div>
        <button
          v-if="editable"
          class="btn-danger-outline"
          @click="onDelete"
        >Delete</button>
      </div>

      <!-- Details Section -->
      <div class="space-y-4">
        <h3 class="section-title">Details</h3>

        <div class="field-group">
          <div class="field">
            <label class="label">Name</label>
            <div v-if="editable">
              <input
                :value="editValues.name ?? org.name"
                @input="editValues.name = ($event.target as HTMLInputElement).value"
                class="input-field"
              />
            </div>
            <div v-else class="field-value">{{ org.name }}</div>
          </div>

          <div class="field">
            <label class="label">State</label>
            <div v-if="editable">
              <select
                :value="editValues.state ?? org.state ?? 'active'"
                @change="editValues.state = ($event.target as HTMLSelectElement).value"
                class="input-field"
              >
                <option value="active">Active</option>
                <option value="inactive">Inactive</option>
              </select>
            </div>
            <div v-else>
              <span :class="['state-badge', org.state === 'active' ? 'state-active' : 'state-inactive']">
                {{ org.state || 'active' }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- Metadata Section -->
      <div class="space-y-4">
        <h3 class="section-title">Metadata</h3>
        <div v-if="editable">
          <textarea
            :value="editValues.metadata ?? JSON.stringify(org.metadata || {}, null, 2)"
            @input="editValues.metadata = ($event.target as HTMLTextAreaElement).value"
            rows="5"
            class="input-field textarea font-mono text-xs"
          />
        </div>
        <div v-else-if="org.metadata && Object.keys(org.metadata).length > 0">
          <pre class="metadata-block">{{ JSON.stringify(org.metadata, null, 2) }}</pre>
        </div>
        <div v-else class="text-sm text-[var(--color-muted-foreground)] italic">No metadata.</div>
      </div>

      <!-- Save button -->
      <div v-if="editable && hasChanges" class="flex justify-end pt-3 border-t border-[var(--color-border)]">
        <div class="flex gap-2">
          <button class="btn-outline" @click="resetEdits">Discard</button>
          <button
            class="btn-primary"
            :disabled="saving"
            @click="onSave"
          >{{ saving ? 'Saving…' : 'Save Changes' }}</button>
        </div>
      </div>

      <!-- System Info -->
      <div class="pt-4 border-t border-[var(--color-border)] space-y-2">
        <h3 class="section-title">System</h3>
        <div class="grid grid-cols-2 gap-2 text-sm">
          <span class="text-[var(--color-muted-foreground)]">ID</span>
          <span class="font-mono text-xs break-all">{{ org.id }}</span>
          <span class="text-[var(--color-muted-foreground)]">Instance</span>
          <span class="font-mono text-xs">{{ org.instance_id || '—' }}</span>
          <span class="text-[var(--color-muted-foreground)]">Created</span>
          <span>{{ formatDate(org.created_at) }}</span>
          <span class="text-[var(--color-muted-foreground)]">Updated</span>
          <span>{{ formatDate(org.updated_at) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted, watch } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-org-detail'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  orgId?: string
  editable?: boolean
  darkMode?: string
}>(), {
  apiBaseUrl: '',
  orgId: '',
  editable: true,
  darkMode: '',
})

const isDark = computed(() => isDarkMode(props.darkMode))

const org = ref<any>(null)
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const editValues = reactive<Record<string, string>>({})

let api: WCApiClient

const orgInitial = computed(() =>
  ((org.value?.name || '?')[0] || '?').toUpperCase()
)

const hasChanges = computed(() => {
  if (!org.value) return false
  if (editValues.name !== undefined && editValues.name !== (org.value.name ?? '')) return true
  if (editValues.state !== undefined && editValues.state !== (org.value.state ?? 'active')) return true
  if (editValues.metadata !== undefined) {
    const current = JSON.stringify(org.value.metadata || {}, null, 2)
    if (editValues.metadata !== current) return true
  }
  return false
})

function formatDate(ts: string): string {
  if (!ts) return '—'
  try {
    return new Date(ts).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
  } catch {
    return ts
  }
}

function resetEdits() {
  delete editValues.name
  delete editValues.state
  delete editValues.metadata
}

async function loadOrg() {
  if (!props.orgId) return
  loading.value = true
  error.value = ''
  try {
    org.value = await api.get<any>(`/v1/orgs/${encodeURIComponent(props.orgId)}`)
    resetEdits()
  } catch (e: any) {
    error.value = e?.message || 'Failed to load organization'
  } finally {
    loading.value = false
  }
}

async function onSave() {
  saving.value = true
  try {
    const body: Record<string, any> = {}
    if (editValues.name !== undefined && editValues.name !== (org.value?.name ?? '')) {
      body.name = editValues.name
    }
    if (editValues.state !== undefined && editValues.state !== (org.value?.state ?? 'active')) {
      body.state = editValues.state
    }
    if (editValues.metadata !== undefined) {
      try {
        body.metadata = JSON.parse(editValues.metadata)
      } catch {
        error.value = 'Invalid metadata JSON'
        saving.value = false
        return
      }
    }
    await api.patch<any>(`/v1/orgs/${encodeURIComponent(props.orgId)}`, body)
    dispatchWCEvent(TAG_NAME, 'org-updated', {
      id: props.orgId,
      changes: body,
    })
    await loadOrg()
  } catch (e: any) {
    error.value = e?.message || 'Failed to save'
  } finally {
    saving.value = false
  }
}

async function onDelete() {
  if (!confirm('Delete this organization? This cannot be undone.')) return
  try {
    await api.delete(`/v1/orgs/${encodeURIComponent(props.orgId)}`)
    dispatchWCEvent(TAG_NAME, 'org-deleted', { id: props.orgId })
  } catch (e: any) {
    error.value = e?.message || 'Failed to delete'
    dispatchWCEvent(TAG_NAME, 'org-error', { error: error.value })
  }
}

onMounted(() => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  loadOrg()
})

watch(() => props.orgId, () => {
  if (api) loadOrg()
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
  --color-destructive: hsl(0 84.2% 60.2%);
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

.section-title {
  font-size: 0.75rem; font-weight: 500; text-transform: uppercase;
  letter-spacing: 0.05em; color: var(--color-muted-foreground);
}
.label { display: block; font-size: 0.875rem; font-weight: 500; margin-bottom: 0.375rem; }

.field-group { display: flex; flex-direction: column; gap: 1rem; }
.field-value {
  border-radius: 0.375rem; border: 1px solid var(--color-border);
  background: var(--color-muted); padding: 0.5rem 0.75rem;
  font-size: 0.875rem; color: var(--color-muted-foreground);
}

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

.metadata-block {
  border-radius: 0.375rem; background: var(--color-muted);
  padding: 0.75rem; font-size: 0.75rem; font-family: monospace;
  overflow-x: auto; white-space: pre-wrap; word-break: break-all;
  color: var(--color-foreground); margin: 0;
}

.avatar-lg {
  display: flex; align-items: center; justify-content: center;
  width: 2.5rem; height: 2.5rem; border-radius: 0.625rem;
  background: linear-gradient(135deg, hsl(240 5.9% 10%), hsl(240 5.9% 25%));
  color: hsl(0 0% 98%); font-size: 1rem; font-weight: 600;
  flex-shrink: 0;
}
:host(.dark) .avatar-lg { background: linear-gradient(135deg, hsl(240 5% 60%), hsl(240 5% 40%)); }

.state-badge {
  display: inline-flex; align-items: center; border-radius: 9999px;
  padding: 0.125rem 0.5rem; font-size: 0.75rem; font-weight: 500; border: 1px solid;
}
.state-active { background: hsl(142 70% 95%); color: hsl(142 70% 30%); border-color: hsl(142 50% 80%); }
.state-inactive { background: hsl(0 70% 95%); color: hsl(0 70% 40%); border-color: hsl(0 50% 80%); }
:host(.dark) .state-active { background: hsl(142 30% 15%); color: hsl(142 60% 65%); border-color: hsl(142 30% 25%); }
:host(.dark) .state-inactive { background: hsl(0 30% 15%); color: hsl(0 60% 65%); border-color: hsl(0 30% 25%); }

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

.btn-danger-outline {
  display: inline-flex; align-items: center; justify-content: center;
  border-radius: 0.375rem; font-size: 0.75rem; font-weight: 500;
  height: 2rem; padding: 0.25rem 0.75rem;
  background: transparent; color: hsl(0 70% 50%);
  border: 1px solid hsl(0 50% 80%); cursor: pointer;
  transition: background-color 0.15s;
}
.btn-danger-outline:hover { background: hsl(0 80% 96%); }
:host(.dark) .btn-danger-outline { color: hsl(0 60% 65%); border-color: hsl(0 30% 30%); }
:host(.dark) .btn-danger-outline:hover { background: hsl(0 40% 15%); }

.error-banner {
  border-radius: 0.375rem; border: 1px solid hsl(0 60% 80%);
  background: hsl(0 80% 96%); padding: 0.5rem 0.75rem;
  font-size: 0.75rem; color: hsl(0 60% 40%);
}
:host(.dark) .error-banner { background: hsl(0 40% 15%); border-color: hsl(0 40% 30%); color: hsl(0 70% 75%); }

.skeleton-row {
  border-radius: 0.375rem;
  background: linear-gradient(90deg, var(--color-muted) 25%, var(--color-background) 50%, var(--color-muted) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}
@keyframes shimmer { 0% { background-position: 200% 0; } 100% { background-position: -200% 0; } }

.space-y-2 > * + * { margin-top: 0.5rem; }
.space-y-3 > * + * { margin-top: 0.75rem; }
.space-y-4 > * + * { margin-top: 1rem; }
.space-y-6 > * + * { margin-top: 1.5rem; }
.grid { display: grid; }
.grid-cols-2 { grid-template-columns: repeat(2, 1fr); }
.gap-2 { gap: 0.5rem; }
</style>
