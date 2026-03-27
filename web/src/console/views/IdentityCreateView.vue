<template>
  <div>
    <div class="form-header">
      <router-link :to="`/s/${schemaType}`" class="back-link">← Back</router-link>
      <h2>Create {{ currentLabel }}</h2>
    </div>

    <!-- Tab toggle -->
    <div class="mode-tabs">
      <button :class="{ active: mode === 'form' }" @click="mode = 'form'">📝 Form</button>
      <button :class="{ active: mode === 'json' }" @click="switchToJson">{ } JSON</button>
    </div>

    <form @submit.prevent="submit" class="form">
      <!-- Version picker (only if multiple versions exist) -->
      <div class="form-section" v-if="versions.length > 1">
        <h3>Schema Version</h3>
        <div class="schema-picker">
          <button
            v-for="v in versions" :key="v.id" type="button"
            class="schema-option" :class="{ active: selectedSchema === v.id }"
            @click="selectSchema(v.id)"
          >
            <span class="schema-type">v{{ v.version }}</span>
            <span class="schema-desc" v-if="v.is_default">default</span>
            <span class="schema-desc" v-else>{{ v.message || 'draft' }}</span>
          </button>
        </div>
      </div>

      <!-- ═══ FORM MODE ═══ -->
      <template v-if="mode === 'form'">
        <!-- Core fields: identifier + display_name -->
        <div class="form-section">
          <h3>{{ isInteractiveIdentity ? 'Account' : 'Identity' }}</h3>
          <div class="field-group">
            <label>Identifier <span class="req">*</span></label>
            <input v-model="form.identifier" type="text" :placeholder="identifierPlaceholder" required />
          </div>
          <div class="field-group">
            <label>Display Name</label>
            <input v-model="form.display_name" type="text" placeholder="Display name" />
          </div>
          <div class="field-group" v-if="hasPassword">
            <label>Password</label>
            <input v-model="form.password" type="password" placeholder="Set initial password" />
          </div>
        </div>

        <!-- Dynamic schema fields (auto-generated from properties) -->
        <div class="form-section" v-if="schemaFields.length">
          <h3>Properties</h3>
          <div class="field-group" v-for="field in schemaFields" :key="field.name">
            <label>{{ field.label }}</label>
            <select v-if="field.type === 'boolean'" v-model="profileData[field.name]">
              <option value="">—</option>
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
            <select v-else-if="field.enum" v-model="profileData[field.name]">
              <option value="">—</option>
              <option v-for="opt in field.enum" :key="opt" :value="opt">{{ opt }}</option>
            </select>
            <input
              v-else
              v-model="profileData[field.name]"
              :type="field.inputType"
              :placeholder="field.description || ''"
            />
            <span class="field-hint" v-if="field.description">{{ field.description }}</span>
          </div>
        </div>

        <!-- Capabilities (only for identity-type entities) -->
        <div class="form-section" v-if="isInteractiveIdentity">
          <h3>Capabilities</h3>
          <div class="cap-checkboxes">
            <label class="cap-check" v-for="cap in availableCaps" :key="cap">
              <input type="checkbox" :value="cap" v-model="form.capabilities" />
              <span class="cap-label">{{ cap }}</span>
            </label>
          </div>
        </div>

        <!-- Invite (only for interactive schemas) -->
        <div class="form-section" v-if="isInteractiveIdentity && hasLogin">
          <label class="invite-check">
            <input type="checkbox" v-model="sendInvite" />
            <span>Send invite link after creation</span>
          </label>
          <p class="invite-hint" v-if="sendInvite">A magic link will be sent to the identifier email.</p>
        </div>
      </template>

      <!-- ═══ JSON MODE ═══ -->
      <template v-if="mode === 'json'">
        <div class="form-section">
          <h3>Entity JSON</h3>
          <p class="json-hint">Edit the full entity payload. Schema validation is live.</p>
          <JsonEditor v-model="jsonContent" label="Entity Data" :schema="currentSchema?.schema" @valid="onJsonValid" @error="onJsonError" />
        </div>
      </template>

      <!-- Actions -->
      <div class="form-actions">
        <router-link :to="`/s/${schemaType}`" class="btn-cancel">Cancel</router-link>
        <button type="submit" class="btn-create" :disabled="submitting || (mode === 'json' && !!jsonError)">
          {{ submitting ? 'Creating…' : `Create ${currentLabel}` }}
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
import JsonEditor from '@/console/components/JsonEditor.vue'

const props = defineProps<{ schemaType: string }>()

const router = useRouter()
const versions = ref<Schema[]>([])
const selectedSchema = ref('')
const submitting = ref(false)
const error = ref('')
const success = ref(false)
const sendInvite = ref(true)
const displayMeta = ref<any>({})
const mode = ref<'form' | 'json'>('form')
const jsonContent = ref('{\n  \n}')
const jsonError = ref('')
const jsonParsed = ref<any>({})

const form = reactive({
  identifier: '',
  display_name: '',
  password: '',
  capabilities: [] as string[],
})

const profileData = reactive<Record<string, string>>({})
const availableCaps = ['password', 'magic_link', 'admin', 'api_key']

const currentSchema = computed(() => versions.value.find(s => s.id === selectedSchema.value))
const currentLabel = computed(() => displayMeta.value.singular || props.schemaType.replace(/_/g, ' '))

// Detect interactive identity types by checking for x-identifier or x-auth-methods
const isInteractiveIdentity = computed(() => {
  const s = currentSchema.value?.schema as any
  if (!s) return false
  return !!(s['x-identifier'] || s['x-auth-methods'])
})

const hasLogin = computed(() => !!(currentSchema.value?.schema as any)?.['x-login'])
const hasPassword = computed(() => {
  const methods = (currentSchema.value?.schema as any)?.['x-auth-methods'] || {}
  return methods.password?.enabled ?? false
})

const identifierPlaceholder = computed(() => {
  if (isInteractiveIdentity.value) return 'user@example.com'
  return `${currentLabel.value.toLowerCase()}-name`
})

interface SchemaField {
  name: string
  label: string
  description: string
  inputType: string
  type: string
  enum?: string[]
}

const schemaFields = computed<SchemaField[]>(() => {
  const s = currentSchema.value
  if (!s) return []
  const schemaProps = (s.schema as any)?.properties || {}
  return Object.entries(schemaProps)
    .filter(([, def]: [string, any]) => !def?.['x-hidden'])
    .map(([name, def]: [string, any]) => ({
      name,
      label: name.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()),
      description: def?.description || '',
      type: def?.type || 'string',
      enum: def?.enum,
      inputType: def?.type === 'integer' || def?.type === 'number' ? 'number'
        : def?.format === 'email' ? 'email'
        : def?.format === 'uri' ? 'url'
        : 'text',
    }))
})

function selectSchema(id: string) {
  selectedSchema.value = id
  Object.keys(profileData).forEach(k => delete profileData[k])
}

function switchToJson() {
  // Sync form data into JSON before switching
  const data: any = {
    identifier: form.identifier || undefined,
    display_name: form.display_name || undefined,
  }
  // Add profile fields
  for (const [k, v] of Object.entries(profileData)) {
    if (v) data[k] = v
  }
  jsonContent.value = JSON.stringify(data, null, 2)
  mode.value = 'json'
}

function onJsonValid(parsed: any) {
  jsonError.value = ''
  jsonParsed.value = parsed
}

function onJsonError(msg: string) {
  jsonError.value = msg
}

onMounted(async () => {
  try {
    const allSchemas = await schemaApi.list()
    versions.value = allSchemas
      .filter((s: Schema) => s.type === props.schemaType)
      .sort((a: Schema, b: Schema) => b.version - a.version)

    // Extract display metadata from catalog
    try {
      const metaRes = await fetch('/v1/schemas/$meta')
      const metaData = await metaRes.json()
      const entry = (metaData['x-catalog'] || {})[props.schemaType]
      if (entry) {
        displayMeta.value = {
          singular: entry.singular,
          alias: entry.alias,
          path: entry.path,
          icon: entry.icon,
        }
      }
    } catch { /* ignore */ }

    const defaultVersion = versions.value.find(s => s.is_default) || versions.value[0]
    if (defaultVersion) {
      selectSchema(defaultVersion.id)
    }
  } catch {}
})

async function submit() {
  submitting.value = true
  error.value = ''

  try {
    let payload: any

    if (mode.value === 'json') {
      // JSON mode: use parsed JSON directly
      const data = jsonParsed.value
      payload = {
        identifier: data.identifier || form.identifier || props.schemaType + '-' + Date.now(),
        display_name: data.display_name || data.identifier || '',
        profile: {},
        data: data,
        schema_id: selectedSchema.value,
      }
    } else {
      // Form mode: build from form fields
      if (!form.identifier.trim()) { error.value = 'Identifier is required'; submitting.value = false; return }
      const profile: Record<string, any> = {}
      if (form.display_name) profile.display_name = form.display_name
      for (const [k, v] of Object.entries(profileData)) {
        if (v !== '') {
          // Convert types
          const fieldDef = schemaFields.value.find(f => f.name === k)
          if (fieldDef?.type === 'boolean') profile[k] = v === 'true'
          else if (fieldDef?.type === 'integer') profile[k] = parseInt(v) || 0
          else if (fieldDef?.type === 'number') profile[k] = parseFloat(v) || 0
          else profile[k] = v
        }
      }

      payload = {
        identifier: form.identifier.trim(),
        display_name: form.display_name.trim() || form.identifier.trim(),
        profile,
        capabilities: isInteractiveIdentity.value ? form.capabilities : [],
        schema_id: selectedSchema.value,
      }
    }

    const created = await entityApi.create(payload)

    if (form.password && created.id && isInteractiveIdentity.value) {
      await api.post(`/v1/entities/${created.id}/password`, { password: form.password })
        .catch(() => {})
    }

    if (sendInvite.value && hasLogin.value && created.id) {
      await magicLinkApi.send(form.identifier.trim()).catch(() => {})
    }

    success.value = true
    setTimeout(() => router.push(`/s/${props.schemaType}`), 800)
  } catch (e: any) {
    error.value = e?.message || 'Failed to create'
  } finally {
    submitting.value = false
  }
}
</script>

<style scoped>
.form-header { display: flex; align-items: center; gap: 1rem; margin-bottom: 1rem; }
.form-header h2 { font-size: 1.25rem; font-weight: 700; color: #1a1a2e; }
.back-link { font-size: 0.8125rem; color: #6b7280; text-decoration: none; }
.back-link:hover { color: #6366f1; }

.mode-tabs {
  display: flex; gap: 0; margin-bottom: 1.25rem; background: #f3f4f6; border-radius: 8px;
  padding: 0.25rem; width: fit-content;
}
.mode-tabs button {
  padding: 0.375rem 1rem; border: none; border-radius: 6px; background: transparent;
  font-size: 0.8125rem; font-weight: 500; color: #6b7280; cursor: pointer; transition: all 0.15s;
}
.mode-tabs button.active { background: #fff; color: #1a1a2e; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }

.form { max-width: 720px; }
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
.field-group input, .field-group select {
  width: 100%; padding: 0.5rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit; transition: border-color 0.15s;
}
.field-group input:focus, .field-group select:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 3px rgba(99,102,241,.1); }
.field-hint { display: block; font-size: 0.6875rem; color: #9ca3af; margin-top: 0.25rem; }

.cap-checkboxes { display: flex; flex-wrap: wrap; gap: 0.75rem; }
.cap-check { display: flex; align-items: center; gap: 0.375rem; cursor: pointer; }
.cap-check input { accent-color: #6366f1; }
.cap-label { font-size: 0.875rem; color: #4b5563; }

.json-hint { font-size: 0.8125rem; color: #6b7280; margin-bottom: 0.75rem; }

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
