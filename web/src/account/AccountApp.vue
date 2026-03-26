<template>
  <div class="account-shell">
    <header class="account-header">
      <div class="header-brand">
        <span class="logo-text">{{ branding.org_name || 'ZITADEL' }}</span>
        <span class="sep">·</span>
        <span class="header-title">My Account</span>
      </div>
      <a class="sign-out" href="/login" @click.prevent="signOut">Sign out</a>
    </header>

    <main class="account-content" v-if="profile">
      <div v-if="message" class="message" :class="messageType">{{ message }}</div>

      <!-- Profile Section -->
      <section class="section">
        <div class="section-header">
          <div class="avatar">{{ initial }}</div>
          <div>
            <h2>{{ profile.display_name || profile.identifier }}</h2>
            <p class="meta">{{ profile.identifier }} · <span class="badge" :class="profile.state">{{ profile.state }}</span></p>
          </div>
        </div>

        <div class="field-group" v-for="(perm, field) in visibleFields" :key="field">
          <label>
            {{ formatLabel(field) }}
            <span v-if="!perm.editable" class="lock-icon" :title="'Set by ' + perm.source">🔒</span>
            <span v-if="perm.sensitive" class="sensitive-badge">sensitive</span>
          </label>
          <div class="field-row">
            <template v-if="perm.editable">
              <input
                v-model="editFields[field]"
                :type="perm.sensitive && !showSensitive[field] ? 'password' : 'text'"
                :placeholder="field"
              />
              <button v-if="perm.sensitive" class="toggle-btn" @click="showSensitive[field] = !showSensitive[field]">
                {{ showSensitive[field] ? 'Hide' : 'Show' }}
              </button>
            </template>
            <template v-else>
              <span class="readonly-value">
                <template v-if="perm.sensitive && !showSensitive[field]">•••••</template>
                <template v-else>{{ profileData[field] || '—' }}</template>
              </span>
              <button v-if="perm.sensitive" class="toggle-btn" @click="showSensitive[field] = !showSensitive[field]">
                {{ showSensitive[field] ? 'Hide' : 'Show' }}
              </button>
              <span class="source-hint">{{ perm.source === 'user' ? '' : 'Set by ' + perm.source }}</span>
            </template>
          </div>
        </div>

        <button class="btn-primary" @click="saveProfile" :disabled="saving">
          {{ saving ? 'Saving…' : 'Save changes' }}
        </button>
      </section>

      <!-- Sessions Section -->
      <section class="section">
        <h3>Active Sessions</h3>
        <div class="session-list">
          <div class="session-item" v-for="s in sessions" :key="s.id" :class="{ current: s.current }">
            <div class="session-info">
              <span class="session-agent">{{ parseUserAgent(s.user_agent) }}</span>
              <span class="session-ip">{{ s.ip_address || 'Unknown IP' }}</span>
              <span v-if="s.current" class="current-badge">This device</span>
            </div>
            <button v-if="!s.current" class="btn-revoke" @click="revokeSession(s.id)">Revoke</button>
          </div>
          <div v-if="!sessions.length" class="empty">No active sessions</div>
        </div>
        <button v-if="sessions.length > 1" class="btn-secondary" @click="revokeOthers">
          Revoke all other sessions
        </button>
      </section>

      <!-- Activity Section -->
      <section class="section">
        <h3>My Activity</h3>
        <div class="activity-list">
          <div class="activity-item" v-for="e in events" :key="e.id">
            <span class="event-type">{{ e.event_type }}</span>
            <span class="event-time">{{ e.time_ago }}</span>
          </div>
          <div v-if="!events.length" class="empty">No activity yet</div>
        </div>
      </section>
    </main>

    <div v-if="!profile && !loadError" class="loading">Loading your profile…</div>
    <div v-if="loadError" class="load-error">
      <h2>Session expired</h2>
      <p>Please <a href="/login">sign in</a> to access your account.</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { api } from '@/api/client'
import { brandingApi, type Branding } from '@/api/branding'

const DEFAULT_BRANDING: Branding = {
  org_id: '', org_name: 'ZITADEL', logo_url: '',
  heading: '', description: '',
  colors: { primary: '#6366f1', background: '#f0f2ff', surface: '#fff', text: '#1a1a2e', error: '#ef4444' },
  font_family: 'Inter, system-ui, sans-serif', hide_zitadel_branding: false,
}

const branding = ref<Branding>(DEFAULT_BRANDING)
const profile = ref<any>(null)
const profileData = ref<Record<string, any>>({})
const fieldPermissions = ref<Record<string, any>>({})
const editFields = reactive<Record<string, string>>({})
const showSensitive = reactive<Record<string, boolean>>({})
const sessions = ref<any[]>([])
const events = ref<any[]>([])
const saving = ref(false)
const message = ref('')
const messageType = ref<'success' | 'error'>('success')
const loadError = ref(false)

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
  // Extract browser
  let browser = 'Unknown browser'
  if (ua.includes('Firefox/')) browser = 'Firefox'
  else if (ua.includes('Edg/')) browser = 'Edge'
  else if (ua.includes('Chrome/')) browser = 'Chrome'
  else if (ua.includes('Safari/') && !ua.includes('Chrome')) browser = 'Safari'
  else if (ua.includes('curl')) browser = 'curl'
  // Extract OS
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

    // Init edit fields with current values.
    for (const [field, perm] of Object.entries(fieldPermissions.value) as any[]) {
      if (perm.editable) {
        editFields[field] = profileData.value[field] || ''
      }
    }
  } catch {
    loadError.value = true
  }
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

async function saveProfile() {
  saving.value = true
  message.value = ''
  try {
    const profileUpdates: Record<string, any> = {}
    for (const [field, perm] of Object.entries(fieldPermissions.value) as any[]) {
      if (perm.editable && editFields[field] !== (profileData.value[field] || '')) {
        profileUpdates[field] = editFields[field]
      }
    }

    const body: any = {}
    if (Object.keys(profileUpdates).length) body.profile = profileUpdates
    if (editFields['display_name'] !== profile.value.display_name) {
      body.display_name = editFields['display_name'] || profile.value.display_name
    }

    await api.patch<any>('/v1/account/profile', body)
    message.value = 'Profile updated successfully'
    messageType.value = 'success'
    await loadProfile()
    await loadActivity()
  } catch (e: any) {
    message.value = e?.message || 'Update failed'
    messageType.value = 'error'
  } finally {
    saving.value = false
  }
}

async function revokeSession(id: string) {
  try {
    await api.post<any>(`/v1/account/sessions/${id}/revoke`, {})
    sessions.value = sessions.value.filter(s => s.id !== id)
    await loadActivity()
  } catch {}
}

async function revokeOthers() {
  try {
    await api.post<any>('/v1/account/sessions/revoke-others', {})
    await loadSessions()
    await loadActivity()
    message.value = 'All other sessions revoked'
    messageType.value = 'success'
  } catch {}
}

function signOut() {
  document.cookie = '__zitadel_session=; Path=/; Max-Age=0'
  window.location.href = '/login'
}

onMounted(async () => {
  try { branding.value = await brandingApi.get() } catch {}
  await Promise.all([loadProfile(), loadSessions(), loadActivity()])
})
</script>

<style scoped>
.account-shell {
  min-height: 100vh;
  background: linear-gradient(135deg, #f0f2ff 0%, #fafbff 50%, #f5f3ff 100%);
  font-family: Inter, system-ui, sans-serif;
  -webkit-font-smoothing: antialiased;
}
.account-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 1rem 2rem; background: #fff;
  border-bottom: 1px solid #e5e7eb;
}
.header-brand { display: flex; align-items: center; gap: 0.5rem; }
.logo-text { font-size: 1rem; font-weight: 800; color: #1a1a2e; letter-spacing: -0.02em; }
.sep { color: #d1d5db; }
.header-title { font-size: 0.875rem; font-weight: 500; color: #6b7280; }
.sign-out {
  font-size: 0.8125rem; color: #6b7280; text-decoration: none; cursor: pointer;
  padding: 0.375rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 8px;
}
.sign-out:hover { color: #ef4444; border-color: #fecaca; }

.account-content { max-width: 680px; margin: 2rem auto; padding: 0 1rem; }

.section {
  background: #fff; border-radius: 12px; padding: 1.5rem;
  box-shadow: 0 1px 3px rgba(0,0,0,.06); border: 1px solid #e5e7eb;
  margin-bottom: 1rem;
}
.section-header { display: flex; align-items: center; gap: 1rem; margin-bottom: 1.5rem; }
.section h3 {
  font-size: 0.8125rem; font-weight: 600; color: #6b7280;
  text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 1rem;
}
.avatar {
  width: 48px; height: 48px; border-radius: 50%; background: #6366f1; color: #fff;
  display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 1.25rem;
}
h2 { font-size: 1.25rem; font-weight: 700; color: #1a1a2e; }
.meta { font-size: 0.8125rem; color: #6b7280; margin-top: 0.125rem; }
.badge { font-size: 0.6875rem; padding: 0.125rem 0.5rem; border-radius: 99px; font-weight: 500; }
.badge.active { background: #ecfdf5; color: #059669; }

.field-group { margin-bottom: 1rem; }
.field-group label {
  display: flex; align-items: center; gap: 0.375rem;
  font-size: 0.8125rem; font-weight: 500; color: #4b5563; margin-bottom: 0.25rem;
}
.lock-icon { font-size: 0.75rem; }
.sensitive-badge {
  font-size: 0.625rem; background: #fef3c7; color: #92400e;
  padding: 0.0625rem 0.375rem; border-radius: 4px; font-weight: 500;
}
.field-row { display: flex; align-items: center; gap: 0.5rem; }
.field-row input {
  flex: 1; padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit; transition: border-color 0.15s;
}
.field-row input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 3px rgba(99,102,241,.1); }
.readonly-value {
  padding: 0.5rem 0.75rem; background: #f9fafb; border: 1px solid #e5e7eb; border-radius: 8px;
  color: #6b7280; font-size: 0.875rem; flex: 1; min-height: 2.25rem; display: flex; align-items: center;
}
.source-hint { font-size: 0.6875rem; color: #9ca3af; font-style: italic; }
.toggle-btn {
  padding: 0.25rem 0.5rem; border: 1px solid #d1d5db; border-radius: 6px; background: #fff;
  font-size: 0.75rem; color: #6b7280; cursor: pointer;
}
.toggle-btn:hover { border-color: #6366f1; color: #6366f1; }

.btn-primary {
  padding: 0.5rem 1.25rem; background: #1a1a2e; color: #fff;
  border: none; border-radius: 8px; font-size: 0.875rem; font-weight: 600;
  cursor: pointer; transition: opacity 0.15s; margin-top: 0.5rem;
}
.btn-primary:hover { opacity: 0.9; }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-secondary {
  padding: 0.375rem 0.875rem; border: 1px solid #fecaca; border-radius: 8px;
  background: #fff; color: #dc2626; font-size: 0.8125rem; cursor: pointer; margin-top: 0.75rem;
}
.btn-secondary:hover { background: #fef2f2; }

.session-list { display: flex; flex-direction: column; gap: 0.5rem; }
.session-item {
  display: flex; align-items: center; justify-content: space-between;
  padding: 0.75rem; border: 1px solid #e5e7eb; border-radius: 8px;
}
.session-item.current { border-color: #6366f1; background: #f0f2ff; }
.session-info { display: flex; flex-direction: column; gap: 0.125rem; }
.session-agent { font-size: 0.8125rem; font-weight: 500; color: #1a1a2e; }
.session-ip { font-size: 0.75rem; color: #9ca3af; }
.current-badge {
  font-size: 0.625rem; background: #6366f1; color: #fff;
  padding: 0.0625rem 0.375rem; border-radius: 4px; font-weight: 500; width: fit-content;
}
.btn-revoke {
  padding: 0.25rem 0.625rem; border: 1px solid #fecaca; border-radius: 6px;
  background: #fff; color: #dc2626; font-size: 0.75rem; cursor: pointer;
}
.btn-revoke:hover { background: #fef2f2; }

.activity-list { display: flex; flex-direction: column; gap: 0.375rem; }
.activity-item {
  display: flex; align-items: center; justify-content: space-between;
  padding: 0.5rem 0.75rem; border-left: 3px solid #e5e7eb; font-size: 0.8125rem;
}
.event-type { color: #4b5563; font-weight: 500; font-family: 'SF Mono', monospace; font-size: 0.75rem; }
.event-time { color: #9ca3af; font-size: 0.75rem; }

.empty { color: #9ca3af; font-size: 0.8125rem; padding: 0.75rem 0; }

.message { padding: 0.625rem 1rem; border-radius: 8px; font-size: 0.8125rem; margin-bottom: 1rem; }
.message.success { background: #ecfdf5; color: #059669; border: 1px solid #a7f3d0; }
.message.error { background: #fef2f2; color: #dc2626; border: 1px solid #fecaca; }

.loading { text-align: center; padding: 4rem; color: #6b7280; }
.load-error { text-align: center; padding: 4rem; }
.load-error h2 { color: #1a1a2e; margin-bottom: 0.5rem; }
.load-error a { color: #6366f1; }
</style>
