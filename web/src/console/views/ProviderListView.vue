<template>
  <div>
    <div class="toolbar">
      <h3>{{ providers.length }} provider{{ providers.length !== 1 ? 's' : '' }}</h3>
      <button v-if="!showCreate" class="btn-primary" @click="showCreate = true">+ Add Provider</button>
      <button v-else class="btn-secondary" @click="showCreate = false; selectedTemplate = null">Cancel</button>
    </div>

    <!-- Template Picker -->
    <div v-if="showCreate && !selectedTemplate" class="template-picker">
      <h4 class="picker-title">Choose a provider template</h4>
      <div class="template-grid">
        <div
          v-for="t in templates" :key="t.id"
          class="template-card"
          @click="pickTemplate(t)"
        >
          <div class="template-icon">{{ templateIcon(t.id) }}</div>
          <div class="template-name">{{ t.name }}</div>
          <div class="template-desc">{{ t.description }}</div>
          <span class="template-protocol">{{ t.protocol }}</span>
        </div>
      </div>
    </div>

    <!-- Create Form -->
    <div v-if="showCreate && selectedTemplate" class="create-form">
      <h4 class="form-title">Configure {{ selectedTemplate.name }} Provider</h4>
      <div class="form-grid">
        <label>Name <input v-model="createForm.name" placeholder="e.g. Google Production" /></label>
        <label>Issuer <input v-model="createForm.issuer" placeholder="https://accounts.google.com" /></label>
        <label>Client ID <input v-model="createForm.client_id" placeholder="your-client-id" /></label>
        <label>Client Secret <input v-model="createForm.client_secret" type="password" placeholder="your-client-secret" /></label>
        <label>Scopes <input v-model="createForm.scopes" placeholder="openid email profile" /></label>
        <label class="checkbox-label">
          <input type="checkbox" v-model="createForm.auto_register" /> Auto-register new users
        </label>
      </div>
      <div v-if="selectedTemplate.claim_overrides && Object.keys(selectedTemplate.claim_overrides).length" class="overrides-info">
        <h5>Default Claim Overrides</h5>
        <div v-for="(expr, field) in selectedTemplate.claim_overrides" :key="field" class="override-row">
          <code>{{ field }}</code> → <code>{{ expr }}</code>
        </div>
      </div>
      <div class="form-actions">
        <button class="btn-secondary" @click="selectedTemplate = null">← Back</button>
        <button class="btn-primary" @click="createProvider" :disabled="!createForm.name || !createForm.issuer || !createForm.client_id">
          Create Provider
        </button>
      </div>
      <div v-if="createError" class="error-msg">{{ createError }}</div>
    </div>

    <!-- Provider List -->
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Name</th>
            <th>Protocol</th>
            <th>Template</th>
            <th>Status</th>
            <th>Auto Register</th>
            <th>Created</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in providers" :key="p.id" class="clickable" @click="toggleDetail(p)">
            <td class="name-cell">
              <span class="provider-icon">{{ templateIcon(p.template) }}</span>
              {{ p.name }}
            </td>
            <td><span class="protocol-badge">{{ p.protocol }}</span></td>
            <td>{{ p.template }}</td>
            <td>
              <span class="badge" :class="p.enabled ? 'active' : 'deactivated'">
                {{ p.enabled ? 'enabled' : 'disabled' }}
              </span>
            </td>
            <td>
              <span class="badge" :class="p.auto_register ? 'active' : 'deactivated'">
                {{ p.auto_register ? 'yes' : 'no' }}
              </span>
            </td>
            <td class="time">{{ formatTime(p.created_at) }}</td>
            <td class="actions-cell" @click.stop>
              <button class="icon-btn" :title="p.enabled ? 'Disable' : 'Enable'" @click="toggleEnabled(p)">
                {{ p.enabled ? '⏸' : '▶' }}
              </button>
              <button class="icon-btn delete-btn" title="Delete" @click="deleteProvider(p)">🗑</button>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-if="!providers.length" class="empty">No providers configured yet. Add one above.</div>
    </div>

    <!-- Inline Detail Panel -->
    <div v-if="detailProvider" class="detail-panel">
      <div class="detail-header">
        <h4>{{ detailProvider.name }}</h4>
        <button class="btn-secondary btn-sm" @click="detailProvider = null">Close</button>
      </div>
      <div class="detail-grid">
        <div class="detail-item">
          <label>ID</label>
          <code>{{ detailProvider.id }}</code>
        </div>
        <div class="detail-item">
          <label>Issuer</label>
          <code>{{ detailProvider.config?.issuer || '—' }}</code>
        </div>
        <div class="detail-item">
          <label>Client ID</label>
          <code>{{ detailProvider.config?.client_id || '—' }}</code>
        </div>
        <div class="detail-item">
          <label>Scopes</label>
          <code>{{ detailProvider.config?.scopes || '—' }}</code>
        </div>
        <div class="detail-item" v-if="detailProvider.claim_overrides && Object.keys(detailProvider.claim_overrides).length">
          <label>Claim Overrides</label>
          <div v-for="(expr, field) in detailProvider.claim_overrides" :key="field" class="override-row">
            <code>{{ field }}</code> → <code>{{ expr }}</code>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

interface Template {
  id: string; name: string; protocol: string; description: string;
  default_config?: Record<string, any>; claim_overrides?: Record<string, string>;
}
interface Provider {
  id: string; name: string; protocol: string; template: string;
  enabled: boolean; auto_register: boolean; config?: Record<string, any>;
  claim_overrides?: Record<string, string>; created_at: string;
}

const providers = ref<Provider[]>([])
const templates = ref<Template[]>([])
const showCreate = ref(false)
const selectedTemplate = ref<Template | null>(null)
const detailProvider = ref<Provider | null>(null)
const createError = ref('')
const createForm = ref({
  name: '', issuer: '', client_id: '', client_secret: '', scopes: 'openid email profile', auto_register: true
})

onMounted(async () => {
  await Promise.all([fetchProviders(), fetchTemplates()])
})

async function fetchProviders() {
  try {
    const res = await fetch('/v1/providers')
    const data = await res.json()
    providers.value = data.providers || []
  } catch { /* ignore */ }
}

async function fetchTemplates() {
  try {
    const res = await fetch('/v1/providers/templates')
    const data = await res.json()
    templates.value = data.templates || []
  } catch { /* ignore */ }
}

function pickTemplate(t: Template) {
  selectedTemplate.value = t
  createForm.value.name = ''
  createForm.value.issuer = t.default_config?.issuer || ''
  createForm.value.scopes = (t.default_config?.scopes as string) || 'openid email profile'
  createForm.value.client_id = ''
  createForm.value.client_secret = ''
  createError.value = ''
}

async function createProvider() {
  createError.value = ''
  try {
    const res = await fetch('/v1/providers', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: createForm.value.name,
        protocol: selectedTemplate.value?.protocol || 'oidc',
        template: selectedTemplate.value?.id || 'custom',
        config: {
          issuer: createForm.value.issuer,
          client_id: createForm.value.client_id,
          client_secret: createForm.value.client_secret,
          scopes: createForm.value.scopes,
        },
        auto_register: createForm.value.auto_register,
      })
    })
    if (!res.ok) {
      const err = await res.json()
      createError.value = err.error || 'Create failed'
      return
    }
    showCreate.value = false
    selectedTemplate.value = null
    await fetchProviders()
  } catch (e: any) {
    createError.value = e.message || 'Network error'
  }
}

async function toggleEnabled(p: Provider) {
  await fetch(`/v1/providers/${p.id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled: !p.enabled })
  })
  await fetchProviders()
}

async function deleteProvider(p: Provider) {
  if (!confirm(`Delete provider "${p.name}"?`)) return
  await fetch(`/v1/providers/${p.id}`, { method: 'DELETE' })
  if (detailProvider.value?.id === p.id) detailProvider.value = null
  await fetchProviders()
}

async function toggleDetail(p: Provider) {
  if (detailProvider.value?.id === p.id) {
    detailProvider.value = null
    return
  }
  // Fetch full detail with config
  try {
    const res = await fetch(`/v1/providers/${p.id}`)
    detailProvider.value = await res.json()
  } catch {
    detailProvider.value = p
  }
}

function templateIcon(id: string): string {
  const icons: Record<string, string> = {
    google: '🔵', entraid: '🟦', gitlab: '🦊', apple: '🍎', custom: '⚙'
  }
  return icons[id] || '🔗'
}

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.toolbar h3 { font-size: 0.875rem; font-weight: 500; color: #6b7280; }

/* Buttons */
.btn-primary {
  padding: 0.5rem 1rem; border-radius: 8px; border: none; cursor: pointer;
  background: #1a1a2e; color: #fff; font-size: 0.8125rem; font-weight: 600;
  font-family: inherit; transition: background 0.15s;
}
.btn-primary:hover { background: #2d2d4e; }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-secondary {
  padding: 0.5rem 1rem; border-radius: 8px; border: 1px solid #d1d5db; cursor: pointer;
  background: #fff; color: #4b5563; font-size: 0.8125rem; font-weight: 500;
  font-family: inherit; transition: all 0.15s;
}
.btn-secondary:hover { border-color: #9ca3af; }
.btn-sm { padding: 0.25rem 0.75rem; font-size: 0.75rem; }

/* Template Picker */
.template-picker { margin-bottom: 1.5rem; }
.picker-title { font-size: 0.9375rem; font-weight: 600; color: #1a1a2e; margin-bottom: 0.75rem; }
.template-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 0.75rem; }
.template-card {
  padding: 1rem; border: 1px solid #e5e7eb; border-radius: 10px; cursor: pointer;
  background: #fff; transition: all 0.15s; position: relative;
}
.template-card:hover { border-color: #6366f1; box-shadow: 0 2px 8px rgba(99,102,241,0.1); }
.template-icon { font-size: 1.5rem; margin-bottom: 0.5rem; }
.template-name { font-weight: 600; font-size: 0.875rem; color: #1a1a2e; margin-bottom: 0.25rem; }
.template-desc { font-size: 0.75rem; color: #6b7280; line-height: 1.4; }
.template-protocol {
  position: absolute; top: 0.75rem; right: 0.75rem;
  font-size: 0.625rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;
  padding: 0.125rem 0.375rem; border-radius: 4px; background: #eff6ff; color: #2563eb;
}

/* Create Form */
.create-form {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px;
  padding: 1.25rem; margin-bottom: 1.5rem;
}
.form-title { font-size: 0.9375rem; font-weight: 600; color: #1a1a2e; margin-bottom: 1rem; }
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; margin-bottom: 1rem; }
.form-grid label {
  display: flex; flex-direction: column; gap: 0.25rem;
  font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em;
}
.form-grid input[type="text"], .form-grid input[type="password"], .form-grid input:not([type]) {
  padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 6px;
  font-size: 0.8125rem; font-family: inherit; transition: border-color 0.15s;
}
.form-grid input:focus { outline: none; border-color: #6366f1; }
.checkbox-label {
  flex-direction: row !important; align-items: center !important; gap: 0.5rem !important;
  text-transform: none !important; font-weight: 500 !important; color: #4b5563 !important;
  font-size: 0.8125rem !important;
}
.form-actions { display: flex; gap: 0.75rem; justify-content: flex-end; }
.error-msg { color: #dc2626; font-size: 0.8125rem; margin-top: 0.5rem; }

.overrides-info {
  margin-bottom: 1rem; padding: 0.75rem; background: #f9fafb; border-radius: 6px;
}
.overrides-info h5 {
  font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase;
  letter-spacing: 0.05em; margin-bottom: 0.5rem;
}
.override-row { font-size: 0.8125rem; color: #4b5563; margin-bottom: 0.25rem; }
.override-row code { background: #eff6ff; padding: 0.125rem 0.375rem; border-radius: 3px; font-size: 0.75rem; }

/* Table */
.table-wrap { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
table { width: 100%; border-collapse: collapse; }
th {
  text-align: left; padding: 0.75rem 1.25rem; font-size: 0.75rem; font-weight: 600;
  color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid #e5e7eb;
}
td { padding: 0.75rem 1.25rem; font-size: 0.875rem; color: #1a1a2e; border-bottom: 1px solid #f3f4f6; }
.clickable { cursor: pointer; }
.clickable:hover { background: #f9fafb; }
.name-cell { font-weight: 500; display: flex; align-items: center; gap: 0.5rem; }
.provider-icon { font-size: 1.125rem; }
.protocol-badge {
  display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px;
  font-size: 0.75rem; font-weight: 500; background: #eff6ff; color: #2563eb;
  text-transform: uppercase;
}
.time { color: #9ca3af; font-size: 0.8125rem; }
.badge { display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }
.badge.active { background: #ecfdf5; color: #059669; }
.badge.deactivated { background: #fef2f2; color: #dc2626; }
.empty { padding: 3rem; text-align: center; color: #9ca3af; }
.actions-cell { display: flex; gap: 0.25rem; }
.icon-btn {
  width: 28px; height: 28px; border: 1px solid #e5e7eb; border-radius: 6px;
  cursor: pointer; background: #fff; font-size: 0.75rem; display: flex;
  align-items: center; justify-content: center; transition: all 0.15s;
}
.icon-btn:hover { border-color: #9ca3af; background: #f9fafb; }
.delete-btn:hover { border-color: #fca5a5; background: #fef2f2; }

/* Detail Panel */
.detail-panel {
  margin-top: 1rem; background: #fff; border: 1px solid #e5e7eb; border-radius: 10px;
  padding: 1.25rem;
}
.detail-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.detail-header h4 { font-size: 1rem; font-weight: 600; color: #1a1a2e; }
.detail-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
.detail-item label {
  font-size: 0.6875rem; font-weight: 600; color: #9ca3af; text-transform: uppercase;
  letter-spacing: 0.05em; display: block; margin-bottom: 0.25rem;
}
.detail-item code {
  background: #f3f4f6; padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.8125rem;
  display: inline-block; word-break: break-all;
}
</style>
