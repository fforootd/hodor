<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-4">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Sessions</h2>
          <p class="text-sm text-[var(--color-muted-foreground)]">
            {{ activeCount }} active of {{ sessions.length }} total
          </p>
        </div>
      </div>

      <!-- Stats -->
      <div class="flex items-center gap-4 p-3 rounded-lg border border-[var(--color-border)] text-sm text-[var(--color-muted-foreground)] bg-[var(--color-card)]">
        <div class="flex items-center gap-1.5 text-green-700">
          <span class="w-2 h-2 rounded-full bg-green-500"></span>
          <span class="font-medium">{{ activeCount }} active</span>
        </div>
        <div class="w-px h-4 bg-[var(--color-border)]"></div>
        <div class="flex items-center gap-1.5">
          <span class="w-2 h-2 rounded-full bg-gray-400"></span>
          <span>{{ expiredCount }} expired</span>
        </div>
        <div class="w-px h-4 bg-[var(--color-border)]"></div>
        <div class="flex items-center gap-1.5 text-red-600">
          <span class="w-2 h-2 rounded-full bg-red-500"></span>
          <span>{{ revokedCount }} revoked</span>
        </div>
      </div>

      <!-- Search -->
      <div v-if="showSearch" class="relative">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search by user, IP, or device…"
          class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
        />
      </div>

      <!-- Loading -->
      <div v-if="loading" class="flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]">Loading sessions…</div>

      <!-- Error -->
      <div v-if="error" class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">{{ error }}</div>

      <!-- Table -->
      <div v-if="!loading && filteredSessions.length" class="rounded-md border border-[var(--color-border)] overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b bg-[var(--color-muted)]">
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">User</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Organization</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">IP</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Device</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Status</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Created</th>
              <th class="h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]">Action</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="s in filteredSessions"
              :key="s.id"
              class="border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors"
              @click="onSessionClick(s)"
            >
              <td class="p-4">
                <div class="flex flex-col min-w-0">
                  <span class="text-sm font-medium truncate">{{ userDict[s.user_id]?.name || 'Unknown User' }}</span>
                  <span class="text-xs text-[var(--color-muted-foreground)] truncate">{{ userDict[s.user_id]?.identifier || s.user_id }}</span>
                </div>
              </td>
              <td class="p-4">
                <span class="inline-flex items-center rounded-full bg-[var(--color-muted)] px-2 py-0.5 text-xs font-medium text-[var(--color-muted-foreground)]">
                  {{ orgDict[s.org_id] || s.org_id || '—' }}
                </span>
              </td>
              <td class="p-4 font-mono text-xs text-[var(--color-muted-foreground)]">{{ s.ip_address || '—' }}</td>
              <td class="p-4 text-[var(--color-muted-foreground)]">{{ parseUserAgent(s.user_agent) }}</td>
              <td class="p-4">
                <span
                  :class="[
                    'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border',
                    s._state === 'active' ? 'bg-green-50 text-green-700 border-green-200' :
                    s._state === 'revoked' ? 'bg-red-50 text-red-700 border-red-200' :
                    'bg-gray-50 text-gray-600 border-gray-200'
                  ]"
                >{{ s._state }}</span>
              </td>
              <td class="p-4 text-[var(--color-muted-foreground)]">{{ formatDate(s.created_at) }}</td>
              <td class="p-4 text-right">
                <button
                  v-if="s._state === 'active'"
                  class="inline-flex items-center rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50 transition-colors"
                  @click.stop="revokeSession(s.id)"
                >Revoke</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Empty -->
      <div v-if="!loading && !error && !filteredSessions.length" class="text-center py-12 text-sm text-[var(--color-muted-foreground)]">
        {{ searchQuery ? 'No sessions match your search.' : 'No sessions found.' }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-session-list'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  darkMode?: string
  showSearch?: boolean
  userId?: string
}>(), {
  apiBaseUrl: '',
  darkMode: '',
  showSearch: true,
  userId: '',
})

const isDark = computed(() => isDarkMode(props.darkMode))

let api: WCApiClient
const sessions = ref<any[]>([])
const userDict = ref<Record<string, { name: string; identifier: string }>>({})
const orgDict = ref<Record<string, string>>({})
const loading = ref(false)
const error = ref('')
const searchQuery = ref('')

const activeCount = computed(() => sessions.value.filter(s => s._state === 'active').length)
const expiredCount = computed(() => sessions.value.filter(s => s._state === 'expired').length)
const revokedCount = computed(() => sessions.value.filter(s => s._state === 'revoked').length)

const filteredSessions = computed(() => {
  if (!searchQuery.value.trim()) return sessions.value
  const q = searchQuery.value.toLowerCase()
  return sessions.value.filter(s => {
    const userName = userDict.value[s.user_id]?.name || ''
    const userIdentifier = userDict.value[s.user_id]?.identifier || ''
    const orgName = orgDict.value[s.org_id] || ''
    return userName.toLowerCase().includes(q) ||
      userIdentifier.toLowerCase().includes(q) ||
      (s.user_id || '').toLowerCase().includes(q) ||
      (s.ip_address || '').toLowerCase().includes(q) ||
      (s.user_agent || '').toLowerCase().includes(q) ||
      orgName.toLowerCase().includes(q)
  })
})

function parseUserAgent(ua: string): string {
  if (!ua) return 'Unknown'
  let browser = 'Unknown'
  if (ua.includes('Firefox/')) browser = 'Firefox'
  else if (ua.includes('Edg/')) browser = 'Edge'
  else if (ua.includes('Chrome/')) browser = 'Chrome'
  else if (ua.includes('Safari/') && !ua.includes('Chrome')) browser = 'Safari'
  else if (ua.includes('curl')) browser = 'curl'
  let os = ''
  if (ua.includes('Mac OS X') || ua.includes('Macintosh')) os = 'macOS'
  else if (ua.includes('Windows')) os = 'Windows'
  else if (ua.includes('Linux')) os = 'Linux'
  return os ? `${browser} · ${os}` : browser
}

function formatDate(ts: string): string {
  if (!ts) return '—'
  const d = new Date(ts)
  return d.toLocaleDateString()
}

function sessionState(s: any): string {
  if (s.revoked_at) return 'revoked'
  if (s.expires_at && new Date(s.expires_at) < new Date()) return 'expired'
  return 'active'
}

function onSessionClick(s: any) {
  dispatchWCEvent(TAG_NAME, 'session-selected', {
    id: s.id,
    user_id: s.user_id,
    state: s._state,
  })
}

async function revokeSession(id: string) {
  try {
    await api.post<any>(`/v1/sessions/${id}/revoke`, {})
    const idx = sessions.value.findIndex(s => s.id === id)
    if (idx !== -1) {
      sessions.value[idx] = { ...sessions.value[idx], _state: 'revoked', revoked_at: new Date().toISOString() }
      sessions.value = [...sessions.value]
    }
    dispatchWCEvent(TAG_NAME, 'session-revoked', { session_id: id })
  } catch (e: any) {
    error.value = e?.message || 'Failed to revoke'
  }
}

onMounted(async () => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  loading.value = true
  error.value = ''
  try {
    // Fetch sessions, users, and orgs in parallel.
    const [sessData, usersData, orgsData] = await Promise.all([
      api.get<any>('/v1/sessions'),
      api.get<any>('/v1/users').catch(() => ({ items: [] })),
      api.get<any>('/v1/orgs').catch(() => ({ items: [] })),
    ])

    const items = sessData.items || []
    sessions.value = items.map((s: any) => ({ ...s, _state: sessionState(s) }))

    // Build user lookup.
    const uDict: Record<string, { name: string; identifier: string }> = {}
    for (const u of (usersData.items || usersData || [])) {
      uDict[u.id] = {
        name: u.display_name || 'Unknown User',
        identifier: u.profile?.email || u.identifier || u.id,
      }
    }
    userDict.value = uDict

    // Build org lookup.
    const oDict: Record<string, string> = {}
    for (const o of (orgsData.items || orgsData || [])) {
      oDict[o.id] = o.name || o.id
    }
    orgDict.value = oDict

    // Filter by userId if provided.
    if (props.userId) {
      sessions.value = sessions.value.filter(s => s.user_id === props.userId)
    }
  } catch (e: any) {
    error.value = e?.message || 'Failed to load sessions'
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
