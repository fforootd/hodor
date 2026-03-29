<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <!-- Loading -->
    <div v-if="!profile && !loadError" class="flex justify-center py-16 text-sm text-[var(--color-muted-foreground)]">
      Loading…
    </div>

    <!-- Error -->
    <div v-if="loadError" class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 text-center">
      <p class="font-semibold mb-1">Session expired</p>
      <p>Please sign in to access your account.</p>
    </div>

    <!-- Content -->
    <div v-if="profile" class="space-y-6">
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-[var(--color-border)] pb-4">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-full bg-[var(--color-primary)] text-[var(--color-primary-foreground)] font-semibold text-sm">
            {{ initial }}
          </div>
          <div>
            <p class="text-sm font-semibold">{{ profile.display_name || profile.identifier || 'My Account' }}</p>
            <p class="text-xs text-[var(--color-muted-foreground)]">{{ branding.org_name || 'Zitadel' }}</p>
          </div>
        </div>
        <button
          class="inline-flex items-center rounded-md border border-[var(--color-border)] px-3 py-1.5 text-xs font-medium hover:bg-[var(--color-muted)] transition-colors"
          @click="onSignOut"
        >Sign out</button>
      </div>

      <!-- Tab Navigation -->
      <div class="flex border-b border-[var(--color-border)]">
        <button
          v-for="tab in visibleTabs"
          :key="tab.id"
          :class="[
            'px-4 py-2 text-sm font-medium border-b-2 -mb-[1px] transition-colors',
            activeTab === tab.id
              ? 'border-[var(--color-primary)] text-[var(--color-foreground)]'
              : 'border-transparent text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]'
          ]"
          @click="activeTab = tab.id"
        >{{ tab.label }}</button>
      </div>

      <!-- PROFILE TAB -->
      <div v-if="activeTab === 'profile'" class="space-y-4">
        <div v-for="(perm, field) in visibleFields" :key="field" class="space-y-1.5">
          <label class="text-sm font-medium flex items-center gap-2">
            {{ formatLabel(field as string) }}
            <span v-if="!perm.editable" class="text-xs" :title="'Set by ' + perm.source">🔒</span>
          </label>
          <div class="flex items-center gap-2">
            <input
              v-if="perm.editable"
              v-model="editFields[field as string]"
              :type="perm.sensitive && !showSensitive[field as string] ? 'password' : 'text'"
              :placeholder="field as string"
              class="flex-1 h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
            <div v-else class="flex-1 rounded-md border bg-[var(--color-muted)] px-3 py-2 text-sm text-[var(--color-muted-foreground)]">
              <template v-if="perm.sensitive && !showSensitive[field as string]">•••••</template>
              <template v-else>{{ profileData[field as string] || '—' }}</template>
            </div>
            <button
              v-if="perm.sensitive"
              class="inline-flex items-center rounded-md border border-[var(--color-border)] px-2 py-1.5 text-xs hover:bg-[var(--color-muted)] transition-colors"
              @click="showSensitive[field as string] = !showSensitive[field as string]"
            >{{ showSensitive[field as string] ? 'Hide' : 'Show' }}</button>
          </div>
          <p v-if="!perm.editable && perm.source !== 'user'" class="text-xs text-[var(--color-muted-foreground)] italic">
            Set by {{ perm.source }}
          </p>
        </div>

        <button
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50 mt-2"
          :disabled="saving"
          @click="saveProfile"
        >{{ saving ? 'Saving…' : 'Save changes' }}</button>
      </div>

      <!-- SESSIONS TAB -->
      <div v-if="activeTab === 'sessions'" class="space-y-4">
        <div class="rounded-md border border-[var(--color-border)] overflow-hidden">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b bg-[var(--color-muted)]">
                <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Device</th>
                <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">IP</th>
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
              <tr v-if="!sessions.length">
                <td colspan="3" class="text-center text-[var(--color-muted-foreground)] py-8">No active sessions</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- ACTIVITY TAB -->
      <div v-if="activeTab === 'activity'" class="space-y-4">
        <div class="rounded-md border border-[var(--color-border)] overflow-hidden">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b bg-[var(--color-muted)]">
                <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">Event</th>
                <th class="h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]">Time</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="e in events" :key="e.id" class="border-b last:border-0">
                <td class="p-4">
                  <span class="inline-flex items-center rounded-md border border-[var(--color-border)] px-2 py-0.5 text-xs font-mono">{{ e.event_type }}</span>
                </td>
                <td class="p-4 text-right text-sm text-[var(--color-muted-foreground)]">{{ e.time_ago }}</td>
              </tr>
              <tr v-if="!events.length">
                <td colspan="2" class="text-center text-[var(--color-muted-foreground)] py-8">No activity yet</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-account'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  darkMode?: string
  showSessions?: boolean
  showActivity?: boolean
}>(), {
  apiBaseUrl: '',
  darkMode: '',
  showSessions: true,
  showActivity: true,
})

const isDark = computed(() => isDarkMode(props.darkMode))

let api: WCApiClient
const activeTab = ref('profile')

const branding = ref<Record<string, any>>({ org_name: 'Zitadel' })
const profile = ref<any>(null)
const profileData = ref<Record<string, any>>({})
const fieldPermissions = ref<Record<string, any>>({})
const editFields = reactive<Record<string, string>>({})
const showSensitive = reactive<Record<string, boolean>>({})
const sessions = ref<any[]>([])
const events = ref<any[]>([])
const saving = ref(false)
const loadError = ref(false)

const visibleTabs = computed(() => {
  const tabs = [{ id: 'profile', label: 'Profile' }]
  if (props.showSessions) tabs.push({ id: 'sessions', label: 'Sessions' })
  if (props.showActivity) tabs.push({ id: 'activity', label: 'Activity' })
  return tabs
})

const initial = computed(() =>
  ((profile.value?.display_name || profile.value?.identifier || '?')[0] || '?').toUpperCase()
)

const visibleFields = computed(() => {
  const result: Record<string, any> = {}
  for (const [field, perm] of Object.entries(fieldPermissions.value)) {
    if (!(perm as any).hidden) result[field] = perm
  }
  return result
})

function formatLabel(field: string) {
  return field.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

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
  else if (ua.includes('Android')) os = 'Android'
  else if (ua.includes('iPhone') || ua.includes('iPad')) os = 'iOS'
  return os ? `${browser} on ${os}` : browser
}

async function loadProfile() {
  try {
    const data = await api.get<any>('/v1/account/profile')
    profile.value = data.identity
    profileData.value = data.identity.profile || {}
    fieldPermissions.value = data.field_permissions || {}
    for (const [field, perm] of Object.entries(fieldPermissions.value) as any[]) {
      if (perm.editable) editFields[field] = profileData.value[field] || ''
    }
  } catch {
    loadError.value = true
  }
}

async function saveProfile() {
  saving.value = true
  try {
    const updates: Record<string, any> = {}
    for (const [field, perm] of Object.entries(fieldPermissions.value) as any[]) {
      if (perm.editable && editFields[field] !== (profileData.value[field] || '')) {
        updates[field] = editFields[field]
      }
    }
    const body: any = {}
    if (Object.keys(updates).length) body.profile = updates
    await api.patch<any>('/v1/account/profile', body)
    dispatchWCEvent(TAG_NAME, 'profile-updated', { changes: updates })
    await loadProfile()
  } catch {}
  saving.value = false
}

async function loadSessions() {
  try {
    const data = await api.get<any>('/v1/account/sessions')
    sessions.value = data.sessions || []
  } catch {}
}

async function loadActivity() {
  try {
    const data = await api.get<any>('/v1/account/activity?limit=10')
    events.value = data.events || []
  } catch {}
}

async function revokeSession(id: string) {
  try {
    await api.post<any>(`/v1/account/sessions/${id}/revoke`, {})
    sessions.value = sessions.value.filter(s => s.id !== id)
    dispatchWCEvent(TAG_NAME, 'session-revoked', { session_id: id })
  } catch {}
}

function onSignOut() {
  dispatchWCEvent(TAG_NAME, 'sign-out')
}

onMounted(async () => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  try {
    const b = await api.get<any>('/v1/branding')
    branding.value = b
  } catch {}
  await Promise.all([
    loadProfile(),
    ...(props.showSessions ? [loadSessions()] : []),
    ...(props.showActivity ? [loadActivity()] : []),
  ])
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
