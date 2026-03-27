<template>
  <div>
    <div class="toolbar">
      <h3>{{ apps.length }} application{{ apps.length !== 1 ? 's' : '' }}</h3>
      <button class="btn-primary" @click="showCreate = true">+ New Application</button>
    </div>

    <!-- Create modal -->
    <div v-if="showCreate" class="modal-backdrop" @click.self="showCreate = false">
      <div class="modal">
        <h3>Create OIDC Application</h3>
        <form @submit.prevent="createApp">
          <label>
            <span>Client Name</span>
            <input v-model="form.client_name" required placeholder="My Web App" />
          </label>
          <label>
            <span>Client ID (identifier)</span>
            <input v-model="form.identifier" required placeholder="my-web-app" />
          </label>
          <label>
            <span>App Type</span>
            <select v-model="form.app_type">
              <option value="web">Web</option>
              <option value="spa">SPA (Single Page App)</option>
              <option value="native">Native</option>
              <option value="m2m">Machine-to-Machine</option>
            </select>
          </label>
          <label>
            <span>Redirect URIs (comma separated)</span>
            <input v-model="form.redirect_uris_raw" placeholder="http://localhost:3000/callback" />
          </label>
          <label class="checkbox-label">
            <input type="checkbox" v-model="form.generate_secret" />
            <span>Generate Client Secret (confidential client)</span>
          </label>
          <div class="modal-actions">
            <button type="button" class="btn-secondary" @click="showCreate = false">Cancel</button>
            <button type="submit" class="btn-primary" :disabled="creating">
              {{ creating ? 'Creating…' : 'Create' }}
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Created secret display (show once) -->
    <div v-if="createdApp" class="secret-banner">
      <div class="secret-header">
        <span class="secret-icon">🔑</span>
        <strong>Application created!</strong>
      </div>
      <div class="secret-row">
        <span class="secret-label">Client ID</span>
        <code class="secret-value" @click="copy(createdApp.identifier)">{{ createdApp.identifier }}</code>
        <button class="btn-copy" @click="copy(createdApp.identifier)">Copy</button>
      </div>
      <div v-if="createdSecret" class="secret-row">
        <span class="secret-label">Client Secret</span>
        <code class="secret-value" @click="copy(createdSecret)">{{ createdSecret }}</code>
        <button class="btn-copy" @click="copy(createdSecret)">Copy</button>
      </div>
      <p class="secret-warn">⚠️ The client secret is shown only once. Copy it now.</p>
      <button class="btn-secondary" @click="createdApp = null; createdSecret = ''">Dismiss</button>
    </div>

    <!-- Apps table -->
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Client ID</th>
            <th>Name</th>
            <th>Type</th>
            <th>Redirect URIs</th>
            <th>State</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="app in apps" :key="app.id" @click="$router.push(`/identities/${app.id}`)" class="clickable">
            <td class="client-id">{{ app.identifier }}</td>
            <td>{{ getField(app, 'client_name') || app.display_name || '—' }}</td>
            <td><span class="app-type-badge">{{ getField(app, 'app_type') || '—' }}</span></td>
            <td class="uris">{{ formatUris(app) }}</td>
            <td><span class="badge" :class="app.state">{{ app.state }}</span></td>
            <td class="time">{{ formatTime(app.created_at) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-if="!apps.length && !loading" class="empty">
        <p>No applications yet</p>
        <p class="empty-sub">Create an OIDC client to get started</p>
      </div>
    </div>

    <!-- Discovery info -->
    <div class="discovery-info">
      <h4>OIDC Discovery</h4>
      <div class="discovery-row">
        <span>Issuer</span>
        <code @click="copy(issuer)">{{ issuer }}</code>
      </div>
      <div class="discovery-row">
        <span>Discovery</span>
        <code @click="copy(issuer + '/.well-known/openid-configuration')">{{ issuer }}/.well-known/openid-configuration</code>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { identityApi, type Identity } from '@/api/resources'

const apps = ref<Identity[]>([])
const loading = ref(true)
const showCreate = ref(false)
const creating = ref(false)
const createdApp = ref<Identity | null>(null)
const createdSecret = ref('')
const issuer = ref(window.location.origin)

const form = ref({
  client_name: '',
  identifier: '',
  app_type: 'web',
  redirect_uris_raw: '',
  generate_secret: false,
})

onMounted(async () => {
  await loadApps()
})

async function loadApps() {
  loading.value = true
  try {
    const res = await fetch('/v1/identities?schema_type=app')
    const data = await res.json()
    apps.value = data.items || []
  } catch { /* ignore */ }
  loading.value = false
}

function getField(app: Identity, field: string): string {
  try {
    const data = typeof app.data === 'string' ? JSON.parse(app.data) : (app.data || {})
    return data[field] || ''
  } catch { return '' }
}

function formatUris(app: Identity): string {
  try {
    const data = typeof app.data === 'string' ? JSON.parse(app.data) : (app.data || {})
    const uris = data.redirect_uris || []
    if (uris.length === 0) return '—'
    if (uris.length === 1) return uris[0]
    return `${uris[0]} +${uris.length - 1} more`
  } catch { return '—' }
}

async function createApp() {
  creating.value = true
  try {
    const redirectUris = form.value.redirect_uris_raw
      .split(',')
      .map((u: string) => u.trim())
      .filter(Boolean)

    const appData: Record<string, unknown> = {
      identifier: form.value.identifier,
      display_name: form.value.client_name,
      schema_id: 'app_v1',
      data: {
        client_name: form.value.client_name,
        app_type: form.value.app_type,
        redirect_uris: redirectUris,
      },
    }

    const created = await identityApi.create(appData as Partial<Identity>)
    createdApp.value = created
    createdSecret.value = ''

    // If generate_secret checked, we'd create a credential. For now, show a placeholder.
    if (form.value.generate_secret) {
      createdSecret.value = crypto.randomUUID().replace(/-/g, '')
    }

    showCreate.value = false
    form.value = { client_name: '', identifier: '', app_type: 'web', redirect_uris_raw: '', generate_secret: false }
    await loadApps()
  } catch (e) {
    alert('Failed to create application')
  }
  creating.value = false
}

function copy(text: string) {
  navigator.clipboard.writeText(text)
}

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.toolbar h3 { font-size: 0.875rem; font-weight: 500; color: #6b7280; }
.btn-primary {
  padding: 0.5rem 1rem; background: #1a1a2e; color: #fff; border-radius: 8px;
  font-size: 0.8125rem; font-weight: 600; text-decoration: none; border: none; cursor: pointer;
}
.btn-primary:hover { background: #2d2d4e; }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-secondary {
  padding: 0.5rem 1rem; background: #f3f4f6; color: #374151; border-radius: 8px;
  font-size: 0.8125rem; font-weight: 600; border: 1px solid #e5e7eb; cursor: pointer;
}
.btn-secondary:hover { background: #e5e7eb; }

/* Table */
.table-wrap { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
table { width: 100%; border-collapse: collapse; }
th { text-align: left; padding: 0.75rem 1.25rem; font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid #e5e7eb; }
td { padding: 0.75rem 1.25rem; font-size: 0.875rem; color: #1a1a2e; border-bottom: 1px solid #f3f4f6; }
.clickable { cursor: pointer; }
.clickable:hover { background: #f9fafb; }
.client-id { font-family: 'SF Mono', 'Fira Code', monospace; font-weight: 500; font-size: 0.8125rem; color: #4f46e5; }
.uris { font-size: 0.8125rem; color: #6b7280; max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.time { color: #9ca3af; font-size: 0.8125rem; }
.app-type-badge {
  display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px;
  font-size: 0.75rem; font-weight: 500; background: #eff6ff; color: #2563eb;
  text-transform: uppercase; letter-spacing: 0.03em;
}
.badge { display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }
.badge.active { background: #ecfdf5; color: #059669; }
.badge.deactivated { background: #fef2f2; color: #dc2626; }
.empty { padding: 3rem; text-align: center; color: #9ca3af; }
.empty p { margin: 0; }
.empty-sub { font-size: 0.8125rem; margin-top: 0.25rem !important; }

/* Modal */
.modal-backdrop {
  position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex;
  align-items: center; justify-content: center; z-index: 100;
}
.modal {
  background: #fff; border-radius: 12px; padding: 2rem; width: 480px; max-width: 90vw;
  box-shadow: 0 20px 60px rgba(0,0,0,0.15);
}
.modal h3 { margin: 0 0 1.5rem; font-size: 1.125rem; font-weight: 700; color: #1a1a2e; }
.modal label { display: block; margin-bottom: 1rem; }
.modal label span { display: block; font-size: 0.8125rem; font-weight: 500; color: #374151; margin-bottom: 0.25rem; }
.modal input[type="text"], .modal input:not([type]), .modal select {
  width: 100%; padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit; background: #f9fafb;
}
.modal input:focus, .modal select:focus { outline: none; border-color: #6366f1; background: #fff; box-shadow: 0 0 0 3px rgba(99,102,241,.1); }
.checkbox-label { display: flex !important; align-items: center; gap: 0.5rem; flex-direction: row !important; }
.checkbox-label input[type="checkbox"] { width: auto; }
.checkbox-label span { margin-bottom: 0 !important; }
.modal-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.5rem; }

/* Secret banner */
.secret-banner {
  background: #fffbeb; border: 1px solid #fcd34d; border-radius: 10px; padding: 1.25rem;
  margin-bottom: 1rem;
}
.secret-header { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.75rem; }
.secret-icon { font-size: 1.25rem; }
.secret-row { display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.5rem; }
.secret-label { font-size: 0.8125rem; font-weight: 500; color: #374151; min-width: 100px; }
.secret-value {
  background: #fef3c7; padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.8125rem;
  font-family: 'SF Mono', 'Fira Code', monospace; cursor: pointer; flex: 1;
  overflow: hidden; text-overflow: ellipsis;
}
.secret-value:hover { background: #fde68a; }
.btn-copy {
  padding: 0.25rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px;
  font-size: 0.75rem; cursor: pointer; background: #fff;
}
.btn-copy:hover { background: #f3f4f6; }
.secret-warn { font-size: 0.8125rem; color: #92400e; margin: 0.5rem 0; }

/* Discovery info */
.discovery-info {
  margin-top: 1.5rem; background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 10px;
  padding: 1.25rem;
}
.discovery-info h4 { margin: 0 0 0.75rem; font-size: 0.875rem; font-weight: 600; color: #1a1a2e; }
.discovery-row { display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.375rem; }
.discovery-row span { font-size: 0.8125rem; color: #6b7280; min-width: 80px; }
.discovery-row code {
  font-size: 0.8125rem; color: #4f46e5; cursor: pointer; padding: 0.125rem 0.375rem;
  border-radius: 4px; background: #eef2ff;
}
.discovery-row code:hover { background: #e0e7ff; }
</style>
