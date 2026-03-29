<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-4">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">{{ label }}</h2>
          <p class="text-sm text-muted-foreground">{{ identities.length }} total</p>
        </div>
        <button
          v-if="showCreate"
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity"
          @click="onCreateClick"
        >+ New {{ singularLabel }}</button>
      </div>

      <!-- Search -->
      <div v-if="showSearch" class="relative">
        <input
          v-model="searchQuery"
          type="text"
          :placeholder="`Search ${label.toLowerCase()}…`"
          class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
        />
      </div>

      <!-- Loading -->
      <div v-if="loading" class="flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]">
        Loading…
      </div>

      <!-- Error -->
      <div v-if="error" class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
        {{ error }}
      </div>

      <!-- Table -->
      <div v-if="!loading && filteredIdentities.length" class="rounded-md border border-[var(--color-border)] overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b bg-[var(--color-muted)]">
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Identifier</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Display Name</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">State</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Created</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="item in paginatedIdentities"
              :key="item.id"
              class="border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors"
              @click="onRowClick(item)"
            >
              <td class="p-4 font-medium">{{ item.identifier }}</td>
              <td class="p-4 text-[var(--color-muted-foreground)]">{{ item.display_name || '—' }}</td>
              <td class="p-4">
                <span
                  :class="[
                    'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border',
                    item.state === 'active'
                      ? 'bg-green-50 text-green-700 border-green-200'
                      : 'bg-red-50 text-red-700 border-red-200'
                  ]"
                >{{ item.state }}</span>
              </td>
              <td class="p-4 text-[var(--color-muted-foreground)]">{{ formatDate(item.created_at) }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Empty -->
      <div v-if="!loading && !error && !filteredIdentities.length" class="text-center py-12 text-sm text-[var(--color-muted-foreground)]">
        {{ searchQuery ? 'No results found.' : `No ${label.toLowerCase()} yet.` }}
      </div>

      <!-- Pagination -->
      <div v-if="totalPages > 1" class="flex items-center justify-between pt-2">
        <p class="text-xs text-[var(--color-muted-foreground)]">
          Showing {{ startIndex + 1 }}–{{ Math.min(endIndex, filteredIdentities.length) }} of {{ filteredIdentities.length }}
        </p>
        <div class="flex gap-1">
          <button
            v-for="p in totalPages"
            :key="p"
            :class="[
              'h-8 w-8 rounded-md text-xs font-medium transition-colors',
              p === currentPage
                ? 'bg-[var(--color-primary)] text-[var(--color-primary-foreground)]'
                : 'border border-[var(--color-border)] hover:bg-[var(--color-muted)]'
            ]"
            @click="currentPage = p"
          >{{ p }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { createWCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-identity-list'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  schemaType?: string
  orgId?: string
  pageSize?: number
  darkMode?: string
  showSearch?: boolean
  showCreate?: boolean
}>(), {
  apiBaseUrl: '',
  schemaType: '',
  orgId: '',
  pageSize: 20,
  darkMode: '',
  showSearch: true,
  showCreate: true,
})

const isDark = computed(() => isDarkMode(props.darkMode))

const identities = ref<any[]>([])
const loading = ref(false)
const error = ref('')
const searchQuery = ref('')
const currentPage = ref(1)

const label = computed(() => {
  if (!props.schemaType) return 'Identities'
  return props.schemaType.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()) + 's'
})

const singularLabel = computed(() => label.value.replace(/s$/, ''))

const filteredIdentities = computed(() => {
  if (!searchQuery.value.trim()) return identities.value
  const q = searchQuery.value.toLowerCase()
  return identities.value.filter(i =>
    (i.identifier || '').toLowerCase().includes(q) ||
    (i.display_name || '').toLowerCase().includes(q)
  )
})

const itemsPerPage = computed(() => Number(props.pageSize) || 20)
const totalPages = computed(() => Math.ceil(filteredIdentities.value.length / itemsPerPage.value))
const startIndex = computed(() => (currentPage.value - 1) * itemsPerPage.value)
const endIndex = computed(() => startIndex.value + itemsPerPage.value)
const paginatedIdentities = computed(() => filteredIdentities.value.slice(startIndex.value, endIndex.value))

// Reset page when search changes
watch(searchQuery, () => { currentPage.value = 1 })

function formatDate(ts: string): string {
  if (!ts) return '—'
  return new Date(ts).toLocaleDateString()
}

function onRowClick(item: any) {
  dispatchWCEvent(TAG_NAME, 'identity-selected', {
    id: item.id,
    identifier: item.identifier,
    schema_type: item.schema_name || props.schemaType,
  })
}

function onCreateClick() {
  dispatchWCEvent(TAG_NAME, 'identity-create', {
    schema_type: props.schemaType,
  })
}

onMounted(async () => {
  loading.value = true
  error.value = ''
  try {
    const api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
    let url = '/v1/users'
    const params: string[] = []
    if (props.schemaType) params.push(`schema_type=${encodeURIComponent(props.schemaType)}`)
    if (props.orgId) params.push(`org_id=${encodeURIComponent(props.orgId)}`)
    if (params.length) url += '?' + params.join('&')

    const data = await api.get<any>(url)
    identities.value = data.items || []
  } catch (e: any) {
    error.value = e?.message || 'Failed to load identities'
  } finally {
    loading.value = false
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
