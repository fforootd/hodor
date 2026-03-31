<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <WCToaster :dark="isDark" />

    <!-- Loading -->
    <div v-if="loading" class="flex justify-center py-16 text-sm text-[var(--color-muted-foreground)]">
      Loading identity…
    </div>

    <!-- Error -->
    <div v-if="loadError" class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
      {{ loadError }}
    </div>

    <!-- Content -->
    <div v-if="!loading && identity" class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-[var(--color-border)] pb-4">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-full bg-[var(--color-primary)] text-[var(--color-primary-foreground)] font-semibold text-sm">
            {{ initial }}
          </div>
          <div>
            <h2 class="text-lg font-semibold tracking-tight">{{ identity.display_name || identity.identifier }}</h2>
            <div class="flex items-center gap-2 text-sm text-[var(--color-muted-foreground)]">
              <span>{{ identity.identifier }}</span>
              <span
                :class="[
                  'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border',
                  identity.state === 'active'
                    ? 'bg-green-50 text-green-700 border-green-200'
                    : 'bg-red-50 text-red-700 border-red-200'
                ]"
              >{{ identity.state }}</span>
            </div>
          </div>
        </div>
        <button
          v-if="editable"
          class="inline-flex items-center rounded-md border border-red-200 bg-transparent px-3 py-1.5 text-xs font-medium text-red-600 hover:bg-red-50 transition-colors"
          @click="showDeleteConfirm = true"
        >Delete</button>
      </div>

      <!-- Profile fields (schema-driven) -->
      <div class="space-y-4">
        <h3 class="text-sm font-medium text-[var(--color-muted-foreground)] uppercase tracking-wider">Profile</h3>
        <div
          v-for="(value, key) in profileFields"
          :key="key"
          class="space-y-1.5"
        >
          <label class="text-sm font-medium">{{ formatLabel(String(key)) }}</label>
          <div v-if="editable" class="flex gap-2">
            <input
              :value="editValues[String(key)] ?? value ?? ''"
              class="flex-1 h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
              @input="editValues[String(key)] = ($event.target as HTMLInputElement).value"
            />
          </div>
          <div v-else class="flex-1 rounded-md border bg-[var(--color-muted)] px-3 py-2 text-sm text-[var(--color-muted-foreground)]">
            {{ value || '—' }}
          </div>
        </div>

        <div v-if="Object.keys(profileFields).length === 0" class="text-sm text-[var(--color-muted-foreground)] italic py-4 text-center">
          No profile fields found for this identity.
        </div>
      </div>

      <!-- Save button -->
      <div v-if="editable && hasChanges" class="flex justify-end pt-2 border-t border-[var(--color-border)]">
        <button
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50"
          :disabled="saving"
          @click="onSave"
        >{{ saving ? 'Saving…' : 'Save Changes' }}</button>
      </div>

      <!-- Metadata -->
      <div class="pt-4 border-t border-[var(--color-border)] space-y-2">
        <h3 class="text-sm font-medium text-[var(--color-muted-foreground)] uppercase tracking-wider">Details</h3>
        <div class="grid grid-cols-2 gap-2 text-sm">
          <span class="text-[var(--color-muted-foreground)]">ID</span>
          <span class="font-mono text-xs break-all">{{ identity.id }}</span>
          <span class="text-[var(--color-muted-foreground)]">Schema</span>
          <span>{{ identity.schema_name || '—' }}</span>
          <span class="text-[var(--color-muted-foreground)]">Created</span>
          <span>{{ formatDate(identity.created_at) }}</span>
          <span class="text-[var(--color-muted-foreground)]">Updated</span>
          <span>{{ formatDate(identity.updated_at) }}</span>
        </div>
      </div>
    </div>

    <WCConfirmDialog
      :open="showDeleteConfirm"
      title="Delete Identity"
      :description="`Are you sure you want to delete ${identity?.display_name || identity?.identifier || 'this identity'}? This action cannot be undone.`"
      :loading="deleting"
      @update:open="showDeleteConfirm = $event"
      @confirm="onDelete"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted, watch } from 'vue'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'
import WCConfirmDialog from '@/wc/components/WCConfirmDialog.vue'
import WCToaster from '@/wc/components/WCToaster.vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-identity-detail'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  identityId?: string
  editable?: boolean
  darkMode?: string
}>(), {
  apiBaseUrl: '',
  identityId: '',
  editable: true,
  darkMode: '',
})

const isDark = computed(() => isDarkMode(props.darkMode))

const identity = ref<any>(null)
const loading = ref(false)
const saving = ref(false)
const deleting = ref(false)
const loadError = ref('')
const showDeleteConfirm = ref(false)
const editValues = reactive<Record<string, string>>({})

let api: WCApiClient

const initial = computed(() =>
  ((identity.value?.display_name || identity.value?.identifier || '?')[0] || '?').toUpperCase()
)

const profileFields = computed(() => {
  if (!identity.value?.profile) return {}
  return identity.value.profile
})

const hasChanges = computed(() => {
  if (!identity.value?.profile) return false
  for (const [key, val] of Object.entries(editValues)) {
    if (val !== (identity.value.profile[key] ?? '')) return true
  }
  return false
})

function formatLabel(field: string): string {
  return field.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase())
}

function formatDate(ts: string): string {
  if (!ts) return '—'
  return new Date(ts).toLocaleDateString()
}

async function loadIdentity() {
  if (!props.identityId) return
  loading.value = true
  loadError.value = ''
  try {
    identity.value = await api.get<any>(`/v1/users/${encodeURIComponent(props.identityId)}`)
    // Reset edit values from profile
    for (const [k, v] of Object.entries(identity.value.profile || {})) {
      editValues[k] = String(v ?? '')
    }
  } catch (e: any) {
    loadError.value = e?.message || 'Failed to load identity'
  } finally {
    loading.value = false
  }
}

async function onSave() {
  saving.value = true
  try {
    const profileUpdates: Record<string, any> = {}
    for (const [k, v] of Object.entries(editValues)) {
      if (v !== (identity.value?.profile?.[k] ?? '')) {
        profileUpdates[k] = v
      }
    }
    await api.patch<any>(`/v1/users/${encodeURIComponent(props.identityId)}`, {
      profile: profileUpdates,
    })
    dispatchWCEvent(TAG_NAME, 'identity-updated', {
      id: props.identityId,
      changes: profileUpdates,
    })
    await loadIdentity()
    notifyMutationSuccess('Identity', 'update')
  } catch (e: any) {
    notifyMutationError('Identity', 'update', e)
  } finally {
    saving.value = false
  }
}

async function onDelete() {
  if (!props.identityId) return
  deleting.value = true
  try {
    await api.delete(`/v1/users/${encodeURIComponent(props.identityId)}`)
    dispatchWCEvent(TAG_NAME, 'identity-deleted', { id: props.identityId })
    notifyMutationSuccess('Identity', 'delete')
    showDeleteConfirm.value = false
  } catch (e: any) {
    notifyMutationError('Identity', 'delete', e)
  } finally {
    deleting.value = false
  }
}

onMounted(() => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  loadIdentity()
})

// Reload if identityId changes dynamically
watch(() => props.identityId, () => {
  if (api) loadIdentity()
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
  --color-popover: hsl(0 0% 100%);
  --color-popover-foreground: hsl(240 10% 3.9%);
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
