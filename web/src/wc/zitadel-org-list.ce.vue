<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-5">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Organizations</h2>
          <p class="text-sm text-[var(--color-muted-foreground)]">
            {{ loading ? 'Loading…' : `${orgs.length} organization${orgs.length !== 1 ? 's' : ''}` }}
          </p>
        </div>
        <button
          v-if="showCreate"
          class="btn-primary"
          @click="showCreateForm = !showCreateForm"
        >
          <span v-if="showCreateForm">✕ Cancel</span>
          <span v-else>+ New Organization</span>
        </button>
      </div>

      <!-- Inline Create Form -->
      <div v-if="showCreateForm" class="card create-form animate-slide-in">
        <h3 class="text-sm font-semibold mb-3">Create Organization</h3>
        <div class="space-y-3">
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Name <span class="text-red-500">*</span></label>
            <input
              v-model="newOrgName"
              type="text"
              placeholder="e.g. Acme Corporation"
              class="input-field"
              @keyup.enter="createOrg"
            />
          </div>
          <div v-if="createError" class="error-banner">{{ createError }}</div>
          <button
            class="btn-primary w-full"
            :disabled="!newOrgName.trim() || creating"
            @click="createOrg"
          >{{ creating ? 'Creating…' : 'Create Organization' }}</button>
        </div>
      </div>

      <!-- Search -->
      <div v-if="showSearch && orgs.length > 0" class="relative">
        <svg class="search-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search organizations…"
          class="input-field pl-9"
        />
      </div>

      <!-- Loading skeleton -->
      <div v-if="loading" class="space-y-3">
        <div v-for="i in 3" :key="i" class="skeleton-row" />
      </div>

      <!-- Error -->
      <div v-if="error" class="error-banner">{{ error }}</div>

      <!-- Table -->
      <div v-if="!loading && filteredOrgs.length" class="rounded-lg border border-[var(--color-border)] overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b bg-[var(--color-muted)]">
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Organization</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">State</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Created</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">ID</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="org in filteredOrgs"
              :key="org.id"
              class="table-row-hover"
              @click="onOrgClick(org)"
            >
              <td class="p-4">
                <div class="flex items-center gap-3">
                  <div class="avatar">{{ orgInitial(org) }}</div>
                  <span class="font-medium">{{ org.name || '—' }}</span>
                </div>
              </td>
              <td class="p-4">
                <span :class="['state-badge', org.state === 'active' ? 'state-active' : 'state-inactive']">
                  {{ org.state || 'active' }}
                </span>
              </td>
              <td class="p-4 text-[var(--color-muted-foreground)] text-xs">{{ formatDate(org.created_at) }}</td>
              <td class="p-4 font-mono text-xs text-[var(--color-muted-foreground)]">{{ org.id }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Empty state -->
      <div v-if="!loading && !error && !filteredOrgs.length" class="empty-state">
        <div class="empty-icon">🏢</div>
        <p class="text-sm font-medium">{{ searchQuery ? 'No organizations match your search.' : 'No organizations yet.' }}</p>
        <p v-if="!searchQuery" class="text-xs text-[var(--color-muted-foreground)] mt-1">Create your first organization to get started.</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-org-list'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  darkMode?: string
  showSearch?: boolean
  showCreate?: boolean
}>(), {
  apiBaseUrl: '',
  darkMode: '',
  showSearch: true,
  showCreate: true,
})

const isDark = computed(() => isDarkMode(props.darkMode))

let api: WCApiClient
const orgs = ref<any[]>([])
const loading = ref(false)
const error = ref('')
const searchQuery = ref('')
const showCreateForm = ref(false)
const newOrgName = ref('')
const creating = ref(false)
const createError = ref('')

const filteredOrgs = computed(() => {
  if (!searchQuery.value.trim()) return orgs.value
  const q = searchQuery.value.toLowerCase()
  return orgs.value.filter(o =>
    (o.name || '').toLowerCase().includes(q) ||
    (o.id || '').toLowerCase().includes(q)
  )
})

function orgInitial(org: any): string {
  return ((org.name || '?')[0] || '?').toUpperCase()
}

function formatDate(ts: string): string {
  if (!ts) return '—'
  try {
    return new Date(ts).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
  } catch {
    return ts
  }
}

function onOrgClick(org: any) {
  dispatchWCEvent(TAG_NAME, 'org-selected', {
    id: org.id,
    name: org.name,
  })
}

async function createOrg() {
  if (!newOrgName.value.trim() || creating.value) return
  creating.value = true
  createError.value = ''
  try {
    const body: Record<string, any> = {
      name: newOrgName.value.trim(),
    }
    const result = await api.post<any>('/v1/orgs', body)
    dispatchWCEvent(TAG_NAME, 'org-created', {
      id: result.id,
      name: result.name,
    })
    // Reset and reload
    newOrgName.value = ''
    showCreateForm.value = false
    await loadOrgs()
  } catch (e: any) {
    createError.value = e?.message || 'Failed to create organization'
    dispatchWCEvent(TAG_NAME, 'org-error', { error: createError.value })
  } finally {
    creating.value = false
  }
}

async function loadOrgs() {
  loading.value = true
  error.value = ''
  try {
    const data = await api.get<any>('/v1/orgs')
    orgs.value = data.items || []
  } catch (e: any) {
    error.value = e?.message || 'Failed to load organizations'
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  loadOrgs()
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

/* Components */
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

.input-field {
  width: 100%; height: 2.25rem; border-radius: 0.375rem;
  border: 1px solid var(--color-input); background: var(--color-background);
  padding: 0.25rem 0.75rem; font-size: 0.875rem;
  box-shadow: 0 1px 2px rgba(0,0,0,0.05);
  color: var(--color-foreground);
  transition: border-color 0.15s, box-shadow 0.15s;
  outline: none;
}
.input-field::placeholder { color: var(--color-muted-foreground); }
.input-field:focus { border-color: var(--color-ring); box-shadow: 0 0 0 2px var(--color-ring); }

.card {
  border-radius: 0.5rem; border: 1px solid var(--color-border);
  padding: 1rem; background: var(--color-card);
}

.error-banner {
  border-radius: 0.375rem; border: 1px solid hsl(0 60% 80%);
  background: hsl(0 80% 96%); padding: 0.5rem 0.75rem;
  font-size: 0.75rem; color: hsl(0 60% 40%);
}
:host(.dark) .error-banner { background: hsl(0 40% 15%); border-color: hsl(0 40% 30%); color: hsl(0 70% 75%); }

.table-row-hover {
  border-bottom: 1px solid var(--color-border); cursor: pointer;
  transition: background-color 0.15s;
}
.table-row-hover:last-child { border-bottom: none; }
.table-row-hover:hover { background: var(--color-muted); }

.avatar {
  display: flex; align-items: center; justify-content: center;
  width: 2rem; height: 2rem; border-radius: 0.5rem;
  background: linear-gradient(135deg, hsl(240 5.9% 10%), hsl(240 5.9% 25%));
  color: hsl(0 0% 98%); font-size: 0.75rem; font-weight: 600;
  flex-shrink: 0;
}
:host(.dark) .avatar { background: linear-gradient(135deg, hsl(240 5% 60%), hsl(240 5% 40%)); }

.state-badge {
  display: inline-flex; align-items: center; border-radius: 9999px;
  padding: 0.125rem 0.5rem; font-size: 0.75rem; font-weight: 500;
  border: 1px solid;
}
.state-active { background: hsl(142 70% 95%); color: hsl(142 70% 30%); border-color: hsl(142 50% 80%); }
.state-inactive { background: hsl(0 70% 95%); color: hsl(0 70% 40%); border-color: hsl(0 50% 80%); }
:host(.dark) .state-active { background: hsl(142 30% 15%); color: hsl(142 60% 65%); border-color: hsl(142 30% 25%); }
:host(.dark) .state-inactive { background: hsl(0 30% 15%); color: hsl(0 60% 65%); border-color: hsl(0 30% 25%); }

.empty-state { text-align: center; padding: 3rem 1rem; }
.empty-icon { font-size: 2.5rem; margin-bottom: 0.75rem; opacity: 0.5; }

.search-icon {
  position: absolute; left: 0.75rem; top: 50%; transform: translateY(-50%);
  color: var(--color-muted-foreground); pointer-events: none;
}

.skeleton-row {
  height: 3rem; border-radius: 0.375rem;
  background: linear-gradient(90deg, var(--color-muted) 25%, var(--color-background) 50%, var(--color-muted) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}
@keyframes shimmer { 0% { background-position: 200% 0; } 100% { background-position: -200% 0; } }
@keyframes slide-in { from { opacity: 0; transform: translateY(-8px); } to { opacity: 1; transform: translateY(0); } }
.animate-slide-in { animation: slide-in 0.2s ease-out; }

.space-y-1\.5 > * + * { margin-top: 0.375rem; }
.space-y-3 > * + * { margin-top: 0.75rem; }
.space-y-5 > * + * { margin-top: 1.25rem; }
</style>
