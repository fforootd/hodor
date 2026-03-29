<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold tracking-tight">Active Sessions</h2>
        <button
          v-if="sessions.length > 1"
          class="inline-flex items-center rounded-md border border-red-200 px-3 py-1.5 text-xs font-medium text-red-600 hover:bg-red-50 transition-colors"
          @click="revokeOthers"
        >Revoke all others</button>
      </div>

      <!-- Loading -->
      <div v-if="loading" class="flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]">
        Loading sessions…
      </div>

      <!-- Error -->
      <div v-if="error" class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
        {{ error }}
      </div>

      <!-- Sessions table -->
      <div v-if="!loading && sessions.length" class="rounded-md border border-[var(--color-border)] overflow-hidden">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b bg-[var(--color-muted)]">
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Device</th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">IP Address</th>
              <th class="h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]">Action</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="s in sessions" :key="s.id" class="border-b last:border-0">
              <td class="p-4">
                <span class="text-sm font-medium">{{ parseUserAgent(s.user_agent) }}</span>
                <span v-if="s.current" class="ml-2 inline-flex items-center rounded-full bg-[var(--color-primary)] text-[var(--color-primary-foreground)] px-2 py-0.5 text-[10px] font-medium">This device</span>
              </td>
              <td class="p-4 text-[var(--color-muted-foreground)] font-mono text-xs">{{ s.ip_address || '—' }}</td>
              <td class="p-4 text-right">
                <button
                  v-if="!s.current"
                  class="inline-flex items-center rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50 transition-colors"
                  @click="revokeSession(s.id)"
                >Revoke</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Empty -->
      <div v-if="!loading && !sessions.length && !error" class="text-center py-12 text-sm text-[var(--color-muted-foreground)]">
        No active sessions.
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-session-manager'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  darkMode?: string
}>(), {
  apiBaseUrl: '',
  darkMode: '',
})

const isDark = computed(() => isDarkMode(props.darkMode))

let api: WCApiClient
const sessions = ref<any[]>([])
const loading = ref(false)
const error = ref('')

function parseUserAgent(ua: string): string {
  if (!ua) return 'Unknown device'
  let browser = 'Unknown'
  if (ua.includes('Firefox/')) browser = 'Firefox'
  else if (ua.includes('Edg/')) browser = 'Edge'
  else if (ua.includes('Chrome/')) browser = 'Chrome'
  else if (ua.includes('Safari/') && !ua.includes('Chrome')) browser = 'Safari'
  let os = ''
  if (ua.includes('Mac OS X') || ua.includes('Macintosh')) os = 'macOS'
  else if (ua.includes('Windows')) os = 'Windows'
  else if (ua.includes('Linux')) os = 'Linux'
  return os ? `${browser} on ${os}` : browser
}

async function loadSessions() {
  loading.value = true
  error.value = ''
  try {
    const data = await api.get<any>('/v1/account/sessions')
    sessions.value = data.sessions || []
  } catch (e: any) {
    error.value = e?.message || 'Failed to load sessions'
  } finally {
    loading.value = false
  }
}

async function revokeSession(id: string) {
  try {
    await api.post<any>(`/v1/account/sessions/${id}/revoke`, {})
    sessions.value = sessions.value.filter(s => s.id !== id)
    dispatchWCEvent(TAG_NAME, 'session-revoked', { session_id: id })
  } catch (e: any) {
    error.value = e?.message || 'Failed to revoke session'
  }
}

async function revokeOthers() {
  try {
    await api.post<any>('/v1/account/sessions/revoke-others', {})
    dispatchWCEvent(TAG_NAME, 'all-sessions-revoked')
    await loadSessions()
  } catch (e: any) {
    error.value = e?.message || 'Failed to revoke sessions'
  }
}

onMounted(() => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  loadSessions()
})
</script>

<style>
:host {
  display: block;
  font-family: 'Inter', ui-sans-serif, system-ui, -apple-system, sans-serif;
  --color-background: hsl(0 0% 100%);
  --color-foreground: hsl(240 10% 3.9%);
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
  --color-primary: hsl(0 0% 98%);
  --color-primary-foreground: hsl(240 5.9% 10%);
  --color-muted: hsl(240 3.7% 15.9%);
  --color-muted-foreground: hsl(240 5% 64.9%);
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
