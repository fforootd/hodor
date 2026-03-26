<template>
  <div v-if="loading" class="loading">Loading schema…</div>
  <div v-else-if="!schema" class="loading">Schema not found</div>
  <div v-else class="editor-layout">
    <!-- Quick Settings Sidebar -->
    <aside class="sidebar">
      <div class="sidebar-section">
        <h4 class="sidebar-heading">Schema</h4>
        <div class="field-row">
          <span class="field-label">Type</span>
          <span class="field-value mono">{{ schema.type }}</span>
        </div>
        <div class="field-row">
          <span class="field-label">Version</span>
          <span class="version-badge">v{{ schema.version }}</span>
        </div>
        <div v-if="identityCount >= 0" class="field-row">
          <span class="field-label">Identities</span>
          <span class="impact-badge" :class="{ warn: identityCount > 0 }">
            {{ identityCount.toLocaleString() }} {{ identityCount === 1 ? 'user' : 'users' }}
          </span>
        </div>
      </div>

      <div class="sidebar-section">
        <h4 class="sidebar-heading">Login Flow</h4>
        <div class="field-row">
          <span class="field-label">Preset</span>
          <select v-model="loginPreset" class="select-input" @change="onQuickSettingChange">
            <option value="identifier_first">Identifier first</option>
            <option value="passkey_first">Passkey first</option>
            <option value="sso_only">SSO only</option>
            <option value="custom">Custom</option>
          </select>
        </div>
        <div class="toggle-group">
          <label class="toggle-row">
            <input type="checkbox" v-model="authPassword" @change="onQuickSettingChange" />
            <span>Password</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" v-model="authMagicLink" @change="onQuickSettingChange" />
            <span>Magic link</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" v-model="authPasskey" @change="onQuickSettingChange" />
            <span>Passkey</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" v-model="authSSO" @change="onQuickSettingChange" />
            <span>SSO</span>
          </label>
        </div>
        <label class="toggle-row mfa-row">
          <input type="checkbox" v-model="mfaRequired" @change="onQuickSettingChange" />
          <span>Require MFA</span>
        </label>
        <label class="toggle-row">
          <input type="checkbox" v-model="registrationAllowed" @change="onQuickSettingChange" />
          <span>Allow registration</span>
        </label>
      </div>

      <div class="sidebar-section">
        <h4 class="sidebar-heading">Branding</h4>
        <div class="field-row">
          <span class="field-label">Heading</span>
          <input type="text" v-model="brandHeading" class="text-input" @input="onQuickSettingChange" />
        </div>
        <div class="field-row">
          <span class="field-label">Primary</span>
          <div class="color-row">
            <input type="color" v-model="brandPrimary" class="color-input" @input="onQuickSettingChange" />
            <span class="mono">{{ brandPrimary }}</span>
          </div>
        </div>
      </div>

      <div class="sidebar-section">
        <h4 class="sidebar-heading">Fields</h4>
        <div v-for="f in schemaFields" :key="f.name" class="field-chip">
          <span class="field-name">{{ f.name }}</span>
          <span v-if="f.identifier" class="chip-tag id">ID</span>
          <span v-if="f.sensitive" class="chip-tag sens">PII</span>
          <span v-if="f.mfa" class="chip-tag mfa">MFA</span>
        </div>
        <div v-if="!schemaFields.length" class="empty-fields">No properties defined</div>
      </div>

      <div class="sidebar-actions">
        <button class="btn-save" :disabled="!dirty || saving" @click="saveSchema">
          {{ saving ? 'Saving…' : 'Save changes' }}
        </button>
        <span v-if="saveSuccess" class="save-msg success">✓ Saved</span>
        <span v-if="saveError" class="save-msg error">{{ saveError }}</span>
      </div>
    </aside>

    <!-- Monaco Editor -->
    <div class="editor-main">
      <div class="editor-toolbar">
        <span class="editor-title">{{ schema.id }}</span>
        <span v-if="dirty" class="dirty-dot">●</span>
        <div class="toolbar-right">
          <button class="btn-copy" @click="copyToClipboard">Copy JSON</button>
          <button class="btn-format" @click="formatJson">Format</button>
        </div>
      </div>
      <div class="editor-container">
        <textarea
          ref="editorEl"
          v-model="editorContent"
          class="code-editor"
          spellcheck="false"
          @input="onEditorChange"
        ></textarea>
        <div v-if="jsonError" class="json-error">⚠ {{ jsonError }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { schemaApi, type Schema } from '@/api/resources'

const route = useRoute()
const router = useRouter()

const schema = ref<Schema | null>(null)
const loading = ref(true)
const editorContent = ref('')
const originalContent = ref('')
const jsonError = ref('')
const saving = ref(false)
const saveSuccess = ref(false)
const saveError = ref('')
const identityCount = ref(-1)

// Quick settings state
const loginPreset = ref('identifier_first')
const authPassword = ref(true)
const authMagicLink = ref(true)
const authPasskey = ref(false)
const authSSO = ref(true)
const mfaRequired = ref(false)
const registrationAllowed = ref(true)
const brandHeading = ref('Welcome back')
const brandPrimary = ref('#6366f1')

const dirty = computed(() => editorContent.value !== originalContent.value)

interface FieldInfo {
  name: string
  identifier: boolean
  sensitive: boolean
  mfa: string
}

const schemaFields = computed<FieldInfo[]>(() => {
  try {
    const parsed = JSON.parse(editorContent.value)
    const props = parsed?.properties || {}
    return Object.entries(props).map(([name, def]: [string, any]) => ({
      name,
      identifier: def?.['x-auth']?.identifier || false,
      sensitive: def?.['x-sensitive'] || false,
      mfa: def?.['x-auth']?.mfa || '',
    }))
  } catch { return [] }
})

onMounted(async () => {
  const id = route.params.id as string
  try {
    const s = await schemaApi.get(id)
    schema.value = s
    const json = JSON.stringify(s.schema, null, 2)
    editorContent.value = json
    originalContent.value = json
    syncSidebarFromJson(json)

    schemaApi.identityCount(id).then(c => { identityCount.value = c }).catch(() => {})
  } catch {
    schema.value = null
  } finally {
    loading.value = false
  }
})

function syncSidebarFromJson(json: string) {
  try {
    const parsed = JSON.parse(json)
    const login = parsed?.['x-login'] || {}
    const branding = parsed?.['x-branding'] || {}
    const methods = login.auth_methods || {}

    loginPreset.value = login.preset || 'identifier_first'
    authPassword.value = methods.password?.enabled ?? true
    authMagicLink.value = methods.magic_link?.enabled ?? true
    authPasskey.value = methods.passkey?.enabled ?? false
    authSSO.value = methods.sso?.enabled ?? true
    mfaRequired.value = login.mfa_required ?? false
    registrationAllowed.value = login.registration_allowed ?? true
    brandHeading.value = branding.heading || 'Welcome back'
    brandPrimary.value = branding.colors?.primary || '#6366f1'
  } catch {}
}

function onEditorChange() {
  jsonError.value = ''
  saveSuccess.value = false
  saveError.value = ''
  try {
    JSON.parse(editorContent.value)
    syncSidebarFromJson(editorContent.value)
  } catch (e: any) {
    jsonError.value = e.message?.replace('JSON.parse: ', '') || 'Invalid JSON'
  }
}

function onQuickSettingChange() {
  try {
    const parsed = JSON.parse(editorContent.value)

    // Update x-login
    if (!parsed['x-login']) parsed['x-login'] = {}
    parsed['x-login'].preset = loginPreset.value
    if (!parsed['x-login'].auth_methods) parsed['x-login'].auth_methods = {}
    const m = parsed['x-login'].auth_methods
    m.password = { ...(m.password || {}), enabled: authPassword.value }
    m.magic_link = { ...(m.magic_link || {}), enabled: authMagicLink.value }
    m.passkey = { ...(m.passkey || {}), enabled: authPasskey.value }
    m.sso = { ...(m.sso || {}), enabled: authSSO.value }
    parsed['x-login'].mfa_required = mfaRequired.value
    parsed['x-login'].registration_allowed = registrationAllowed.value

    // Update x-branding
    if (!parsed['x-branding']) parsed['x-branding'] = {}
    parsed['x-branding'].heading = brandHeading.value
    if (!parsed['x-branding'].colors) parsed['x-branding'].colors = {}
    parsed['x-branding'].colors.primary = brandPrimary.value

    editorContent.value = JSON.stringify(parsed, null, 2)
    jsonError.value = ''
  } catch {}
}

async function saveSchema() {
  if (!schema.value || jsonError.value) return
  saving.value = true
  saveSuccess.value = false
  saveError.value = ''
  try {
    const parsed = JSON.parse(editorContent.value)
    const updated = await schemaApi.update(schema.value.id, parsed)
    schema.value = updated
    originalContent.value = editorContent.value
    saveSuccess.value = true
    setTimeout(() => { saveSuccess.value = false }, 3000)
  } catch (e: any) {
    saveError.value = e.message || 'Save failed'
  } finally {
    saving.value = false
  }
}

function formatJson() {
  try {
    const parsed = JSON.parse(editorContent.value)
    editorContent.value = JSON.stringify(parsed, null, 2)
    jsonError.value = ''
  } catch {}
}

function copyToClipboard() {
  navigator.clipboard.writeText(editorContent.value)
}
</script>

<style scoped>
.loading { padding: 3rem; text-align: center; color: #9ca3af; }

.editor-layout {
  display: flex; gap: 0; min-height: calc(100vh - 140px);
  background: #fff; border: 1px solid #e5e7eb; border-radius: 12px; overflow: hidden;
}

/* Sidebar */
.sidebar {
  width: 280px; border-right: 1px solid #e5e7eb; padding: 1.25rem;
  overflow-y: auto; display: flex; flex-direction: column; gap: 0; background: #fafbfc;
}
.sidebar-section { padding: 0.75rem 0; border-bottom: 1px solid #f0f1f3; }
.sidebar-section:first-child { padding-top: 0; }
.sidebar-section:last-of-type { border-bottom: none; }
.sidebar-heading {
  font-size: 0.6875rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.06em;
  color: #9ca3af; margin-bottom: 0.625rem;
}

.field-row {
  display: flex; align-items: center; justify-content: space-between; gap: 0.5rem;
  margin-bottom: 0.5rem;
}
.field-label { font-size: 0.8125rem; color: #4b5563; }
.field-value { font-size: 0.8125rem; color: #1a1a2e; font-weight: 500; }

.version-badge {
  font-size: 0.6875rem; font-weight: 600; padding: 0.125rem 0.5rem;
  background: #f0f2ff; color: #6366f1; border-radius: 4px;
}
.impact-badge {
  font-size: 0.75rem; font-weight: 600; padding: 0.125rem 0.5rem;
  background: #f3f4f6; color: #6b7280; border-radius: 4px;
}
.impact-badge.warn { background: #fef3c7; color: #92400e; }

.select-input {
  flex: 1; max-width: 160px; padding: 0.25rem 0.5rem; border: 1px solid #d1d5db;
  border-radius: 6px; font-size: 0.8125rem; font-family: inherit; background: #fff;
}
.select-input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 2px rgba(99,102,241,.1); }

.text-input {
  flex: 1; max-width: 160px; padding: 0.25rem 0.5rem; border: 1px solid #d1d5db;
  border-radius: 6px; font-size: 0.8125rem; font-family: inherit; background: #fff;
}
.text-input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 2px rgba(99,102,241,.1); }

.toggle-group { display: flex; flex-direction: column; gap: 0.25rem; margin-bottom: 0.5rem; }
.toggle-row {
  display: flex; align-items: center; gap: 0.5rem; font-size: 0.8125rem; color: #374151;
  cursor: pointer;
}
.toggle-row input[type="checkbox"] {
  width: 16px; height: 16px; accent-color: #6366f1; cursor: pointer;
}
.mfa-row { margin-top: 0.25rem; }

.color-row { display: flex; align-items: center; gap: 0.5rem; }
.color-input {
  width: 28px; height: 28px; border: 1px solid #d1d5db; border-radius: 6px;
  cursor: pointer; padding: 0;
}
.mono { font-family: 'SF Mono', 'Fira Code', monospace; font-size: 0.75rem; color: #6b7280; }

/* Fields list */
.field-chip {
  display: flex; align-items: center; gap: 0.375rem; padding: 0.25rem 0;
}
.field-name { font-size: 0.8125rem; color: #1a1a2e; font-weight: 500; }
.chip-tag {
  font-size: 0.5625rem; font-weight: 700; padding: 0.0625rem 0.375rem; border-radius: 3px;
  text-transform: uppercase; letter-spacing: 0.04em;
}
.chip-tag.id { background: #dbeafe; color: #1d4ed8; }
.chip-tag.sens { background: #fee2e2; color: #991b1b; }
.chip-tag.mfa { background: #d1fae5; color: #065f46; }
.empty-fields { font-size: 0.8125rem; color: #9ca3af; }

.sidebar-actions { padding-top: 0.75rem; margin-top: auto; }
.btn-save {
  width: 100%; padding: 0.5rem; border: none; border-radius: 8px;
  background: #6366f1; color: #fff; font-size: 0.875rem; font-weight: 600;
  font-family: inherit; cursor: pointer; transition: background 0.15s;
}
.btn-save:hover:not(:disabled) { background: #4f46e5; }
.btn-save:disabled { opacity: 0.5; cursor: not-allowed; }
.save-msg { display: block; margin-top: 0.5rem; font-size: 0.75rem; text-align: center; }
.save-msg.success { color: #16a34a; }
.save-msg.error { color: #ef4444; }

/* Editor */
.editor-main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.editor-toolbar {
  display: flex; align-items: center; gap: 0.75rem; padding: 0.75rem 1.25rem;
  border-bottom: 1px solid #e5e7eb; background: #fafbfc;
}
.editor-title { font-size: 0.8125rem; font-weight: 600; color: #1a1a2e; font-family: 'SF Mono', monospace; }
.dirty-dot { color: #f59e0b; font-size: 1rem; }
.toolbar-right { margin-left: auto; display: flex; gap: 0.5rem; }
.btn-copy, .btn-format {
  padding: 0.25rem 0.75rem; border: 1px solid #d1d5db; border-radius: 6px;
  background: #fff; font-size: 0.75rem; font-family: inherit; color: #4b5563;
  cursor: pointer; transition: all 0.15s;
}
.btn-copy:hover, .btn-format:hover { background: #f3f4f6; border-color: #9ca3af; }

.editor-container { flex: 1; position: relative; }
.code-editor {
  width: 100%; height: 100%; padding: 1.25rem; border: none; resize: none;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 0.8125rem; line-height: 1.65; color: #1a1a2e; background: #fff;
  tab-size: 2;
}
.code-editor:focus { outline: none; }

.json-error {
  position: absolute; bottom: 0; left: 0; right: 0;
  padding: 0.5rem 1.25rem; background: #fef2f2; color: #991b1b;
  font-size: 0.75rem; border-top: 1px solid #fecaca;
}
</style>
