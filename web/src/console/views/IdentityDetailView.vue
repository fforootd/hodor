<template>
  <div v-if="identity">
    <div class="detail-header">
      <div class="avatar">{{ (identity.display_name || identity.identifier)[0]?.toUpperCase() }}</div>
      <div class="header-info">
        <h2>{{ identity.display_name || identity.identifier }}</h2>
        <p class="meta">{{ identity.identifier }} · <span class="badge" :class="identity.state">{{ identity.state }}</span></p>
      </div>
      <div class="header-actions">
        <button v-if="!editing && isInteractiveIdentity" class="btn-invite" @click="sendInviteLink" :disabled="inviting">
          {{ inviting ? 'Sending…' : '✉ Invite' }}
        </button>
        <button v-if="!editing" class="btn-edit" @click="startEdit">✎ Edit</button>
        <button v-if="!editing" class="btn-delete" @click="showDeleteConfirm = true">✕ Delete</button>
        <template v-if="editing">
          <button class="btn-save" @click="save" :disabled="saving">{{ saving ? 'Saving…' : '✓ Save' }}</button>
          <button class="btn-cancel" @click="cancelEdit">Cancel</button>
        </template>
      </div>
    </div>

    <!-- Delete confirmation -->
    <div v-if="showDeleteConfirm" class="confirm-overlay" @click.self="showDeleteConfirm = false">
      <div class="confirm-dialog">
        <h3>Delete {{ displayMeta.singular || 'Entity' }}</h3>
        <p>Are you sure you want to delete <strong>{{ identity.identifier }}</strong>? This action cannot be undone.</p>
        <div class="confirm-actions">
          <button class="btn-cancel" @click="showDeleteConfirm = false">Cancel</button>
          <button class="btn-danger" @click="deleteIdentity" :disabled="deleting">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </button>
        </div>
      </div>
    </div>

    <div v-if="message" class="message" :class="messageType">{{ message }}</div>

    <!-- View/Edit mode tabs -->
    <div class="mode-tabs">
      <button :class="{ active: viewMode === 'form' }" @click="viewMode = 'form'">📝 Form</button>
      <button :class="{ active: viewMode === 'json' }" @click="switchToJson">{ } JSON</button>
    </div>

    <!-- ═══ FORM VIEW ═══ -->
    <template v-if="viewMode === 'form' && !editing">
      <div class="cards">
        <div class="card">
          <h4>Profile</h4>
          <div class="fields">
            <div class="field" v-for="(val, key) in profileFields" :key="key">
              <span class="field-key">{{ formatKey(key as string) }}</span>
              <span class="field-val">{{ formatValue(val) }}</span>
            </div>
            <div v-if="!Object.keys(profileFields).length" class="empty">No profile data</div>
          </div>
        </div>
        <div class="card" v-if="isInteractiveIdentity">
          <h4>Capabilities</h4>
          <div class="cap-list" v-if="identity.capabilities?.length">
            <span v-for="cap in identity.capabilities" :key="cap" class="cap-tag" :class="cap">{{ cap }}</span>
          </div>
          <div v-else class="empty">No capabilities</div>
        </div>
      </div>
      <div class="cards">
        <div class="card">
          <h4>Metadata</h4>
          <div class="fields" v-if="Object.keys(metaFields).length">
            <div class="field" v-for="(val, key) in metaFields" :key="key">
              <span class="field-key">{{ formatKey(key as string) }}</span>
              <span class="field-val">{{ val }}</span>
            </div>
          </div>
          <div v-else class="empty">No metadata</div>
        </div>
        <div class="card">
          <h4>Details</h4>
          <dl class="detail-grid">
            <dt>ID</dt><dd class="mono">{{ identity.id }}</dd>
            <dt>Org ID</dt><dd>{{ identity.org_id }}</dd>
            <dt>Schema</dt><dd>{{ identity.schema_type || '—' }}</dd>
            <dt>Created</dt><dd>{{ formatTime(identity.created_at) }}</dd>
            <dt>Updated</dt><dd>{{ formatTime(identity.updated_at) }}</dd>
          </dl>
        </div>
      </div>
    </template>

    <!-- ═══ JSON VIEW ═══ -->
    <template v-if="viewMode === 'json' && !editing">
      <div class="json-view-section">
        <JsonEditor
          :modelValue="entityJsonReadonly"
          label="Stored Entity (read-only)"
          :schema="entitySchema"
          height="480px"
        />
        <p class="json-view-hint">This is the raw entity data as stored. Click <strong>Edit</strong> to modify.</p>
      </div>
    </template>

    <!-- ═══ FORM EDIT ═══ -->
    <template v-if="editing && viewMode === 'form'">
      <div class="edit-form">
        <div class="form-section">
          <h4>Account</h4>
          <div class="field-group">
            <label>Display Name</label>
            <input v-model="editForm.display_name" type="text" />
          </div>
          <div class="field-group">
            <label>State</label>
            <select v-model="editForm.state">
              <option value="active">Active</option>
              <option value="deactivated">Deactivated</option>
              <option value="locked">Locked</option>
            </select>
          </div>
        </div>

        <div class="form-section">
          <h4>Profile</h4>
          <div class="field-group" v-for="(val, key) in editForm.profile" :key="key">
            <label>
              {{ formatKey(key as string) }}
              <button type="button" class="remove-field" @click="removeProfileField(key as string)">×</button>
            </label>
            <input v-model="editForm.profile[key as string]" type="text" />
          </div>
          <div class="add-field-row">
            <input v-model="newFieldName" type="text" placeholder="New field name" class="add-field-input" />
            <button type="button" class="btn-add-field" @click="addProfileField">+ Add</button>
          </div>
        </div>
      </div>
    </template>

    <!-- ═══ JSON EDIT ═══ -->
    <template v-if="editing && viewMode === 'json'">
      <div class="json-edit-section">
        <JsonEditor
          v-model="editJsonContent"
          label="Edit Entity JSON"
          :schema="entitySchema"
          height="480px"
          @valid="onEditJsonValid"
          @error="onEditJsonError"
        />
        <div v-if="editJsonError" class="json-edit-error">{{ editJsonError }}</div>
      </div>
    </template>

    <router-link :to="backRoute" class="back-link">← Back to {{ displayMeta.alias || 'list' }}</router-link>
  </div>
  <div v-else class="loading">Loading...</div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { entityApi, magicLinkApi, schemaApi, type Identity } from '@/api/resources'
import JsonEditor from '@/console/components/JsonEditor.vue'

const route = useRoute()
const router = useRouter()
const identity = ref<Identity | null>(null)
const editing = ref(false)
const saving = ref(false)
const deleting = ref(false)
const showDeleteConfirm = ref(false)
const inviting = ref(false)
const message = ref('')
const messageType = ref<'success' | 'error' | 'invite'>('success')
const newFieldName = ref('')
const viewMode = ref<'form' | 'json'>('form')
const editJsonContent = ref('{}')
const editJsonError = ref('')
const editJsonParsed = ref<any>({})
const displayMeta = ref<any>({})
const entitySchema = ref<any>(null)

// Detect schema type from route params or identity data
const schemaType = computed(() => (route.params as any).schemaType || identity.value?.schema_type || '')

// Detect interactive identity types
const isInteractiveIdentity = computed(() => {
  if (!entitySchema.value) return true // default to showing full UI
  return !!(entitySchema.value['x-identifier'] || entitySchema.value['x-auth-methods'])
})

const backRoute = computed(() => schemaType.value ? `/s/${schemaType.value}` : '/')

const editForm = reactive({
  display_name: '',
  state: '',
  profile: {} as Record<string, string>,
})

const profileFields = computed(() => {
  const p = identity.value?.profile
  return (p && typeof p === 'object') ? p as Record<string, unknown> : {}
})

const metaFields = computed(() => {
  const m = identity.value?.metadata
  return (m && typeof m === 'object') ? m as Record<string, unknown> : {}
})

const entityJsonReadonly = computed(() => {
  if (!identity.value) return '{}'
  return JSON.stringify(identity.value, null, 2)
})

function formatKey(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

function formatValue(val: unknown): string {
  if (val === null || val === undefined) return '—'
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}

function formatTime(ts: string) {
  return new Date(ts).toLocaleString()
}

function switchToJson() {
  if (editing.value) {
    // Sync form data to JSON
    const data: any = { ...identity.value }
    data.display_name = editForm.display_name
    data.state = editForm.state
    data.profile = { ...editForm.profile }
    editJsonContent.value = JSON.stringify(data, null, 2)
  }
  viewMode.value = 'json'
}

function startEdit() {
  if (!identity.value) return
  editForm.display_name = identity.value.display_name || ''
  editForm.state = identity.value.state
  const p = identity.value.profile || {}
  editForm.profile = {}
  for (const [k, v] of Object.entries(p)) {
    editForm.profile[k] = String(v ?? '')
  }
  // Also prepare JSON edit
  editJsonContent.value = JSON.stringify(identity.value, null, 2)
  editing.value = true
  message.value = ''
}

function cancelEdit() {
  editing.value = false
  message.value = ''
}

function addProfileField() {
  const name = newFieldName.value.trim()
  if (name && !(name in editForm.profile)) {
    editForm.profile[name] = ''
    newFieldName.value = ''
  }
}

function removeProfileField(key: string) {
  delete editForm.profile[key]
}

function onEditJsonValid(parsed: any) {
  editJsonError.value = ''
  editJsonParsed.value = parsed
}

function onEditJsonError(msg: string) {
  editJsonError.value = msg
}

async function save() {
  if (!identity.value) return
  saving.value = true
  message.value = ''
  try {
    let payload: any

    if (viewMode.value === 'json') {
      // JSON mode: send parsed JSON
      const data = editJsonParsed.value
      payload = {
        display_name: data.display_name || editForm.display_name,
        state: data.state || editForm.state,
        profile: data.profile || {},
      }
    } else {
      // Form mode
      const profile: Record<string, string> = {}
      for (const [k, v] of Object.entries(editForm.profile)) {
        if (v.trim()) profile[k] = v.trim()
      }
      payload = {
        display_name: editForm.display_name.trim(),
        state: editForm.state,
        profile,
      }
    }

    await entityApi.update(identity.value.id, payload as any)
    identity.value = await entityApi.get(route.params.id as string)
    editing.value = false
    message.value = 'Updated successfully'
    messageType.value = 'success'
  } catch (e: any) {
    message.value = e?.message || 'Update failed'
    messageType.value = 'error'
  } finally {
    saving.value = false
  }
}

async function sendInviteLink() {
  if (!identity.value) return
  inviting.value = true
  message.value = ''
  try {
    const resp = await magicLinkApi.send(identity.value.identifier)
    message.value = resp.purpose === 'register'
      ? 'Registration invite sent — check server logs.'
      : 'Login link sent — check server logs.'
    messageType.value = 'invite'
  } catch (e: any) {
    message.value = e?.message || 'Failed to send invite'
    messageType.value = 'error'
  } finally {
    inviting.value = false
  }
}

async function deleteIdentity() {
  if (!identity.value) return
  deleting.value = true
  try {
    await entityApi.delete(identity.value.id)
    router.push(backRoute.value)
  } catch (e: any) {
    showDeleteConfirm.value = false
    message.value = e?.message || 'Delete failed'
    messageType.value = 'error'
    deleting.value = false
  }
}

onMounted(async () => {
  try {
    identity.value = await entityApi.get(route.params.id as string)

    // Fetch schema for this entity type
    if (identity.value?.schema_type) {
      const allSchemas = await schemaApi.list()
      const match = allSchemas.find((s: any) => s.type === identity.value!.schema_type && s.is_default)
        || allSchemas.find((s: any) => s.type === identity.value!.schema_type)
      if (match) {
        entitySchema.value = match.schema
      }
    }

    // Fetch display metadata from catalog
    try {
      const metaRes = await fetch('/v1/schemas/$meta')
      const metaData = await metaRes.json()
      const st = identity.value?.schema_type
      if (st) {
        const entry = (metaData['x-catalog'] || {})[st]
        if (entry) {
          displayMeta.value = { singular: entry.singular, alias: entry.alias, path: entry.path, icon: entry.icon }
        }
      }
    } catch { /* ignore */ }
  } catch {}
})
</script>

<style scoped>
.detail-header { display: flex; align-items: center; gap: 1rem; margin-bottom: 1rem; }
.header-info { flex: 1; }
.header-actions { display: flex; gap: 0.5rem; }
.avatar {
  width: 48px; height: 48px; border-radius: 12px; background: #6366f1; color: #fff;
  display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 1.25rem;
}
h2 { font-size: 1.25rem; font-weight: 700; color: #1a1a2e; }
.meta { font-size: 0.8125rem; color: #6b7280; margin-top: 0.125rem; }
.badge { padding: 0.125rem 0.375rem; border-radius: 4px; font-size: 0.75rem; }
.badge.active { background: #ecfdf5; color: #059669; }
.badge.deactivated { background: #fef2f2; color: #dc2626; }
.badge.locked { background: #fef3c7; color: #92400e; }

.mode-tabs {
  display: flex; gap: 0; margin-bottom: 1.25rem; background: #f3f4f6; border-radius: 8px;
  padding: 0.25rem; width: fit-content;
}
.mode-tabs button {
  padding: 0.375rem 1rem; border: none; border-radius: 6px; background: transparent;
  font-size: 0.8125rem; font-weight: 500; color: #6b7280; cursor: pointer; transition: all 0.15s;
}
.mode-tabs button.active { background: #fff; color: #1a1a2e; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }

.btn-edit, .btn-save, .btn-cancel, .btn-delete, .btn-invite {
  padding: 0.375rem 0.875rem; border-radius: 8px; font-size: 0.8125rem; font-weight: 500;
  cursor: pointer; border: 1px solid #d1d5db; background: #fff; color: #4b5563; transition: all 0.15s;
}
.btn-invite { color: #1d4ed8; border-color: #bfdbfe; }
.btn-invite:hover { background: #eff6ff; }
.btn-invite:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-edit:hover { border-color: #6366f1; color: #6366f1; }
.btn-save { background: #1a1a2e; color: #fff; border-color: #1a1a2e; }
.btn-save:hover { opacity: 0.9; }
.btn-save:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-delete { color: #dc2626; border-color: #fecaca; }
.btn-delete:hover { background: #fef2f2; }
.btn-cancel:hover { background: #f9fafb; }
.btn-danger {
  padding: 0.375rem 0.875rem; border-radius: 8px; font-size: 0.8125rem; font-weight: 600;
  cursor: pointer; border: none; background: #dc2626; color: #fff;
}
.btn-danger:hover { background: #b91c1c; }
.btn-danger:disabled { opacity: 0.5; }

.message { padding: 0.625rem 1rem; border-radius: 8px; font-size: 0.8125rem; margin-bottom: 1rem; }
.message.success { background: #ecfdf5; color: #059669; border: 1px solid #a7f3d0; }
.message.error { background: #fef2f2; color: #dc2626; border: 1px solid #fecaca; }
.message.invite { background: #eff6ff; color: #1d4ed8; border: 1px solid #bfdbfe; }

.cards { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-bottom: 1rem; }
.card { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1.25rem; }
.card h4, .form-section h4 { font-size: 0.8125rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 0.75rem; }

.fields { display: flex; flex-direction: column; gap: 0.5rem; }
.field { display: flex; gap: 1rem; }
.field-key { font-size: 0.8125rem; font-weight: 500; color: #6b7280; min-width: 100px; }
.field-val { font-size: 0.875rem; color: #1a1a2e; word-break: break-all; }

.cap-list { display: flex; flex-wrap: wrap; gap: 0.375rem; }
.cap-tag { font-size: 0.75rem; padding: 0.25rem 0.625rem; border-radius: 6px; font-weight: 500; background: #f3f4f6; color: #4b5563; }
.cap-tag.admin { background: #fef3c7; color: #92400e; }
.cap-tag.password { background: #eff6ff; color: #2563eb; }

.detail-grid { display: grid; grid-template-columns: 80px 1fr; gap: 0.5rem; }
dt { font-size: 0.8125rem; font-weight: 500; color: #6b7280; }
dd { font-size: 0.875rem; color: #1a1a2e; }
.mono { font-family: monospace; font-size: 0.8125rem; }

/* JSON view */
.json-view-section { margin-bottom: 1rem; }
.json-view-hint { font-size: 0.75rem; color: #9ca3af; margin-top: 0.5rem; }
.json-edit-section { margin-bottom: 1rem; }
.json-edit-error { margin-top: 0.5rem; padding: 0.375rem 0.75rem; background: #fef2f2; color: #dc2626; font-size: 0.75rem; border-radius: 6px; }

/* Edit form */
.edit-form { max-width: 640px; }
.form-section { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1.25rem; margin-bottom: 1rem; }
.field-group { margin-bottom: 0.75rem; }
.field-group label { display: flex; align-items: center; gap: 0.5rem; font-size: 0.8125rem; font-weight: 500; color: #4b5563; margin-bottom: 0.25rem; }
.field-group input, .field-group select {
  width: 100%; padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit;
}
.field-group input:focus, .field-group select:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 3px rgba(99,102,241,.1); }
.remove-field { background: none; border: none; color: #dc2626; cursor: pointer; font-size: 1rem; padding: 0; line-height: 1; }
.add-field-row { display: flex; gap: 0.5rem; }
.add-field-input { flex: 1; padding: 0.375rem 0.625rem; border: 1px dashed #d1d5db; border-radius: 8px; font-size: 0.8125rem; }
.add-field-input:focus { outline: none; border-color: #6366f1; }
.btn-add-field { padding: 0.375rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px; background: #fff; color: #6b7280; font-size: 0.8125rem; cursor: pointer; }
.btn-add-field:hover { border-color: #6366f1; color: #6366f1; }

/* Delete confirmation overlay */
.confirm-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex;
  align-items: center; justify-content: center; z-index: 100;
}
.confirm-dialog {
  background: #fff; border-radius: 12px; padding: 1.5rem; max-width: 400px; width: 90%;
  box-shadow: 0 20px 60px rgba(0,0,0,0.2);
}
.confirm-dialog h3 { font-size: 1.125rem; font-weight: 700; color: #1a1a2e; margin-bottom: 0.5rem; }
.confirm-dialog p { font-size: 0.875rem; color: #6b7280; margin-bottom: 1.25rem; }
.confirm-actions { display: flex; gap: 0.5rem; justify-content: flex-end; }

.back-link { display: inline-block; margin-top: 1.5rem; font-size: 0.8125rem; color: #6b7280; text-decoration: none; }
.back-link:hover { color: #6366f1; }
.loading { padding: 3rem; text-align: center; color: #9ca3af; }
.empty { color: #9ca3af; font-size: 0.875rem; }
</style>
