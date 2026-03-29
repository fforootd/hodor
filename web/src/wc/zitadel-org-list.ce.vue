<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-4">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Organizations</h2>
          <p class="text-sm text-[var(--color-muted-foreground)]">{{ orgs.length }} organization{{ orgs.length !== 1 ? 's' : '' }}</p>
        </div>
        <button
          v-if="showCreate"
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity"
          @click="showCreateForm = !showCreateForm"
        >{{ showCreateForm ? 'Cancel' : '+ New' }}</button>
      </div>

      <!-- Inline Create Form -->
      <div v-if="showCreateForm" class="rounded-lg border border-[var(--color-border)] p-4 space-y-3 bg-[var(--color-card)]">
        <div class="space-y-1.5">
          <label class="text-sm font-medium">Identifier</label>
          <input
            v-model="newOrgIdentifier"
            type="text"
            placeholder="e.g. acme-corp"
            class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            @keyup.enter="createOrg"
          />
        </div>
        <div class="space-y-1.5">
          <label class="text-sm font-medium">Display Name</label>
          <input
            v-model="newOrgDisplayName"
            type="text"
            placeholder="e.g. Acme Corporation"
            class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            @keyup.enter="createOrg"
          />
        </div>
        <div v-if="createError" class="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">{{ createError }}</div>
        <button
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50"
          :disabled="!newOrgIdentifier.trim() || creating"
          @click="createOrg"
        >{{ creating ? 'Creating…' : 'Create Organization' }}</button>
      </div>

      <!-- Search -->
      <div v-if="showSearch" class="relative">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search organizations…"
          class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
        />
      </div>

      <!-- Loading -->
      <div v-if="loading" class="flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]">
        Loading organizations…
      </div>

      <!-- Error -->
      <div v-if="error" class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">{{ error }}</div>

      <!-- Table -->
      <div v-if="!loading && filteredOrgs.length" class="rounded-md border border-[var(--color-border)] overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b bg-[var(--color-muted)]">
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Identifier</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Display Name</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">ID</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="org in filteredOrgs"
              :key="org.id"
              class="border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors"
              @click="onOrgClick(org)"
            >
              <td class="p-4 font-medium">{{ org.identifier }}</td>
              <td class="p-4 text-[var(--color-muted-foreground)]">{{ org.display_name || '—' }}</td>
              <td class="p-4 font-mono text-xs text-[var(--color-muted-foreground)]">{{ org.id }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Empty -->
      <div v-if="!loading && !error && !filteredOrgs.length" class="text-center py-12 text-sm text-[var(--color-muted-foreground)]">
        {{ searchQuery ? 'No organizations match your search.' : 'No organizations yet.' }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
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
const newOrgIdentifier = ref('')
const newOrgDisplayName = ref('')
const creating = ref(false)
const createError = ref('')

const filteredOrgs = computed(() => {
  if (!searchQuery.value.trim()) return orgs.value
  const q = searchQuery.value.toLowerCase()
  return orgs.value.filter(o =>
    (o.identifier || '').toLowerCase().includes(q) ||
    (o.display_name || '').toLowerCase().includes(q)
  )
})

function onOrgClick(org: any) {
  dispatchWCEvent(TAG_NAME, 'org-selected', {
    id: org.id,
    identifier: org.identifier,
    display_name: org.display_name,
  })
}

async function createOrg() {
  if (!newOrgIdentifier.value.trim() || creating.value) return
  creating.value = true
  createError.value = ''
  try {
    const body: Record<string, any> = {
      identifier: newOrgIdentifier.value.trim(),
    }
    if (newOrgDisplayName.value.trim()) {
      body.display_name = newOrgDisplayName.value.trim()
    }
    const result = await api.post<any>('/v1/orgs', body)
    dispatchWCEvent(TAG_NAME, 'org-created', {
      id: result.id,
      identifier: result.identifier,
    })
    // Reset and reload
    newOrgIdentifier.value = ''
    newOrgDisplayName.value = ''
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
  --color-card: hsl(240 10% 3.9%);
  --color-primary: hsl(0 0% 98%);
  --color-primary-foreground: hsl(240 5.9% 10%);
  --color-muted: hsl(240 3.7% 15.9%);
  --color-muted-foreground: hsl(240 5% 64.9%);
  --color-border: hsl(240 3.7% 15.9%);
  --color-input: hsl(240 3.7% 15.9%);
  --color-ring: hsl(240 4.9% 83.9%);
}
.zitadel-wc { color: var(--color-foreground); background: var(--color-background); padding: 1rem; }
.zitadel-wc.dark { color-scheme: dark; }
</style>
