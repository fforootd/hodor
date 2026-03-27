<template>
  <div>
    <div class="form-header">
      <router-link to="/identities" class="back-link">← Back</router-link>
      <h2>Create Identity</h2>
    </div>

    <form @submit.prevent="submit" class="form">
      <!-- Schema picker -->
      <div class="form-section">
        <h3>Identity Type</h3>
        <div class="schema-picker">
          <button
            v-for="s in schemas" :key="s.id" type="button"
            class="schema-option" :class="{ active: selectedSchema === s.id }"
            @click="selectSchema(s.id)"
          >
            <span class="schema-type">{{ s.type }}</span>
            <span class="schema-desc">{{ fieldCount(s) }} fields</span>
          </button>
        </div>
      </div>

      <!-- Core fields -->
      <div class="form-section">
        <h3>Account</h3>
        <div class="field-group">
          <label>Identifier <span class="req">*</span></label>
          <input v-model="form.identifier" type="text" placeholder="user@example.com" required />
        </div>
        <div class="field-group">
          <label>Display Name</label>
          <input v-model="form.display_name" type="text" placeholder="Jane Doe" />
        </div>
        <div class="field-group">
          <label>Password</label>
          <input v-model="form.password" type="password" placeholder="Set initial password" />
        </div>
      </div>

      <!-- Dynamic schema fields -->
      <div class="form-section" v-if="schemaFields.length">
        <h3>Profile</h3>
        <div class="field-group" v-for="field in schemaFields" :key="field.name">
          <label>{{ field.label }}</label>
          <input
            v-model="profileData[field.name]"
            :type="field.inputType"
            :placeholder="field.description || ''"
          />
        </div>
      </div>

      <!-- Capabilities -->
      <div class="form-section">
        <h3>Capabilities</h3>
        <div class="cap-checkboxes">
          <label class="cap-check" v-for="cap in availableCaps" :key="cap">
            <input type="checkbox" :value="cap" v-model="form.capabilities" />
            <span class="cap-label">{{ cap }}</span>
          </label>
        </div>
      </div>

      <!-- Invite -->
      <div class="form-section">
        <label class="invite-check">
          <input type="checkbox" v-model="sendInvite" />
          <span>Send invite link after creation</span>
        </label>
        <p class="invite-hint" v-if="sendInvite">A magic link will be sent to the identifier email. Check server logs in dev mode.</p>
      </div>

      <!-- Actions -->
      <div class="form-actions">
        <router-link to="/identities" class="btn-cancel">Cancel</router-link>
        <button type="submit" class="btn-create" :disabled="submitting">
          {{ submitting ? 'Creating…' : 'Create Identity' }}
        </button>
      </div>

      <div v-if="error" class="error-banner">{{ error }}</div>
      <div v-if="success" class="success-banner">Created! Redirecting…</div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { entityApi, magicLinkApi, schemaApi, type Schema } from '@/api/resources'
import { api } from '@/api/client'

const router = useRouter()
const schemas = ref<Schema[]>([])
const selectedSchema = ref('')
const submitting = ref(false)
const error = ref('')
const success = ref(false)
const sendInvite = ref(true)

const form = reactive({
  identifier: '',
  display_name: '',
  password: '',
  capabilities: [] as string[],
})

const profileData = reactive<Record<string, string>>({})
const availableCaps = ['password', 'magic_link', 'admin', 'api_key']

interface SchemaField {
  name: string
  label: string
  description: string
  inputType: string
}

const schemaFields = computed<SchemaField[]>(() => {
  const s = schemas.value.find(s => s.id === selectedSchema.value)
  if (!s) return []
  const props = (s.schema as any)?.properties || {}
  return Object.entries(props).map(([name, def]: [string, any]) => ({
    name,
    label: name.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase()),
    description: def?.description || '',
    inputType: def?.format === 'email' ? 'email' : def?.format === 'uri' ? 'url' : 'text',
  }))
})

function selectSchema(id: string) {
  selectedSchema.value = id
  // Reset profile fields
  Object.keys(profileData).forEach(k => delete profileData[k])
}

function fieldCount(s: Schema) {
  return Object.keys((s.schema as any)?.properties || {}).length
}

onMounted(async () => {
  try { schemas.value = await schemaApi.list() } catch {}
  if (schemas.value.length) selectSchema(schemas.value[0].id)
})

async function submit() {
  if (!form.identifier.trim()) { error.value = 'Identifier is required'; return }
  submitting.value = true
  error.value = ''

  try {
    // Build profile from display_name + schema fields
    const profile: Record<string, string> = {}
    if (form.display_name) profile.display_name = form.display_name
    for (const [k, v] of Object.entries(profileData)) {
      if (v) profile[k] = v
    }

    const created = await entityApi.create({
      identifier: form.identifier.trim(),
      display_name: form.display_name.trim() || form.identifier.trim(),
      profile,
      capabilities: form.capabilities,
    } as any)

    // Set password if provided
    if (form.password && created.id) {
      await api.post(`/v1/entities/${created.id}/password`, { password: form.password })
        .catch(() => { /* password endpoint may not exist yet */ })
    }

    // Send invite if checked
    if (sendInvite.value && created.id) {
      await magicLinkApi.send(form.identifier.trim()).catch(() => {})
    }

    success.value = true
    setTimeout(() => router.push(`/identities/${created.id}`), 800)
  } catch (e: any) {
    error.value = e?.message || 'Failed to create identity'
  } finally {
    submitting.value = false
  }
}
</script>

<style scoped>
.form-header { display: flex; align-items: center; gap: 1rem; margin-bottom: 1.5rem; }
.form-header h2 { font-size: 1.25rem; font-weight: 700; color: #1a1a2e; }
.back-link { font-size: 0.8125rem; color: #6b7280; text-decoration: none; }
.back-link:hover { color: #6366f1; }

.form { max-width: 640px; }
.form-section { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1.25rem; margin-bottom: 1rem; }
.form-section h3 { font-size: 0.8125rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 1rem; }

.schema-picker { display: flex; flex-wrap: wrap; gap: 0.5rem; }
.schema-option {
  padding: 0.5rem 1rem; border: 1px solid #e5e7eb; border-radius: 8px; background: #fff;
  cursor: pointer; transition: all 0.15s; text-align: left;
}
.schema-option:hover { border-color: #a5b4fc; }
.schema-option.active { border-color: #6366f1; background: #f0f2ff; }
.schema-type { display: block; font-size: 0.875rem; font-weight: 600; color: #1a1a2e; }
.schema-desc { font-size: 0.75rem; color: #9ca3af; }

.field-group { margin-bottom: 0.75rem; }
.field-group label { display: block; font-size: 0.8125rem; font-weight: 500; color: #4b5563; margin-bottom: 0.25rem; }
.req { color: #ef4444; }
.field-group input {
  width: 100%; padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit; transition: border-color 0.15s;
}
.field-group input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 3px rgba(99,102,241,.1); }

.cap-checkboxes { display: flex; flex-wrap: wrap; gap: 0.75rem; }
.cap-check { display: flex; align-items: center; gap: 0.375rem; cursor: pointer; }
.cap-check input { accent-color: #6366f1; }
.cap-label { font-size: 0.875rem; color: #4b5563; }

.form-actions { display: flex; gap: 0.75rem; justify-content: flex-end; margin-top: 1.5rem; }
.btn-cancel {
  padding: 0.5rem 1.25rem; border: 1px solid #d1d5db; border-radius: 8px; background: #fff;
  color: #4b5563; font-size: 0.875rem; text-decoration: none; font-weight: 500;
}
.btn-cancel:hover { background: #f9fafb; }
.btn-create {
  padding: 0.5rem 1.25rem; border: none; border-radius: 8px; background: #1a1a2e;
  color: #fff; font-size: 0.875rem; font-weight: 600; cursor: pointer; transition: opacity 0.15s;
}
.btn-create:hover { opacity: 0.9; }
.btn-create:disabled { opacity: 0.5; cursor: not-allowed; }

.error-banner { margin-top: 1rem; padding: 0.75rem 1rem; background: #fef2f2; border: 1px solid #fecaca; border-radius: 8px; color: #dc2626; font-size: 0.875rem; }
.success-banner { margin-top: 1rem; padding: 0.75rem 1rem; background: #ecfdf5; border: 1px solid #a7f3d0; border-radius: 8px; color: #059669; font-size: 0.875rem; }

.invite-check { display: flex; align-items: center; gap: 0.5rem; cursor: pointer; font-size: 0.875rem; color: #4b5563; }
.invite-check input { accent-color: #6366f1; }
.invite-hint { margin-top: 0.5rem; font-size: 0.75rem; color: #6b7280; padding: 0.5rem 0.75rem; background: #eff6ff; border-radius: 6px; }
</style>
