<template>
  <div class="create-user-wizard" :class="{ 'wizard-standalone': standalone }">
    <!-- Standalone wrapper when used as web component (no Dialog) -->
    <template v-if="standalone">
      <div class="wizard-container">
        <WizardInner
          :steps="steps"
          :current-step="currentStep"
          :loading-orgs="loadingOrgs"
          :loading-schema="loadingSchema"
          :submitting="submitting"
          :error-msg="errorMsg"
          :available-orgs="availableOrgs"
          :selected-orgs="selectedOrgs"
          :schema-fields="schemaFields"
          :schema-versions="schemaVersions"
          :selected-schema-id="selectedSchemaId"
          :profile-data="profileData"
          :auth-methods-config="authMethodsConfig"
          :auth-method="authMethod"
          :initial-password="initialPassword"
          :send-welcome-message="sendWelcomeMessage"
          :can-proceed="canProceed"
          :validation-errors="validationErrors"
          :selected-org-names="selectedOrgNames"
          :profile-data-overview="profileDataOverview"
          :primary-identifier="primaryIdentifier"
          :auth-method-label="authMethodLabel"
          @toggle-org="toggleOrg"
          @select-schema="selectSchema"
          @update:profile-field="updateProfileField"
          @update:auth-method="authMethod = $event"
          @update:initial-password="initialPassword = $event"
          @update:send-welcome-message="sendWelcomeMessage = $event"
          @prev="prevStep"
          @next="nextStep"
          @close="handleClose"
        />
      </div>
    </template>

    <!-- Dialog wrapper when used as Vue component -->
    <template v-else>
      <Dialog :open="open" @update:open="$emit('update:open', $event)">
        <DialogContent class="sm:max-w-[780px] p-0 overflow-hidden flex flex-col max-h-[90vh]">
          <WizardInner
            :steps="steps"
            :current-step="currentStep"
            :loading-orgs="loadingOrgs"
            :loading-schema="loadingSchema"
            :submitting="submitting"
            :error-msg="errorMsg"
            :available-orgs="availableOrgs"
            :selected-orgs="selectedOrgs"
            :schema-fields="schemaFields"
            :schema-versions="schemaVersions"
            :selected-schema-id="selectedSchemaId"
            :profile-data="profileData"
            :auth-methods-config="authMethodsConfig"
            :auth-method="authMethod"
            :initial-password="initialPassword"
            :send-welcome-message="sendWelcomeMessage"
            :can-proceed="canProceed"
            :validation-errors="validationErrors"
            :selected-org-names="selectedOrgNames"
            :profile-data-overview="profileDataOverview"
            :primary-identifier="primaryIdentifier"
            :auth-method-label="authMethodLabel"
            @toggle-org="toggleOrg"
            @select-schema="selectSchema"
            @update:profile-field="updateProfileField"
            @update:auth-method="authMethod = $event"
            @update:initial-password="initialPassword = $event"
            @update:send-welcome-message="sendWelcomeMessage = $event"
            @prev="prevStep"
            @next="nextStep"
            @close="handleClose"
          />
        </DialogContent>
      </Dialog>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, watch, defineAsyncComponent } from 'vue'
import WizardInner from './CreateUserWizardInner.vue'

// Only import Dialog when not standalone (tree-shaken in web component build)
import { Dialog, DialogContent } from '@/components/ui/dialog'

// ---------- Props ----------
const props = withDefaults(defineProps<{
  /** Whether the dialog is open (ignored in standalone mode) */
  open?: boolean
  /** Run as standalone element (for web component usage, no Dialog wrapper) */
  standalone?: boolean
  /** Schema type to create (e.g. 'human_user', 'service_user') */
  schemaType?: string
  /** Pre-select org by identifier */
  orgId?: string
  /** API base URL override (for web component usage outside the SPA) */
  apiBaseUrl?: string
}>(), {
  open: false,
  standalone: false,
  schemaType: 'human_user',
  orgId: '',
  apiBaseUrl: '',
})

const emit = defineEmits<{
  (e: 'update:open', val: boolean): void
  (e: 'created', entityId: string): void
  (e: 'close'): void
  (e: 'error', error: string): void
}>()

// ---------- Types ----------
interface SchemaField {
  name: string
  label: string
  description: string
  inputType: string
  type: string
  format?: string
  enum?: string[]
  required: boolean
  xIdentifier?: boolean
  xUnique?: string
  xEditable?: boolean
  xSensitive?: boolean
  xVerify?: string
  minLength?: number
  maxLength?: number
  pattern?: string
}

interface SchemaVersion {
  id: string
  type: string
  version: number
  is_default: boolean
  message?: string
  schema: Record<string, any>
}

interface AuthMethodConfig {
  enabled: boolean
  interactive: boolean
  position?: number
}

interface StepDef {
  title: string
  description: string
}

// ---------- API helpers (works both in SPA and standalone) ----------
async function apiFetch<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const base = props.apiBaseUrl || (window as any).__ZITADEL_BASE_PATH__ || ''
  const resp = await fetch(`${base}${path}`, {
    ...opts,
    headers: { 'Content-Type': 'application/json', ...opts.headers },
    credentials: 'same-origin',
  })
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({ error: resp.statusText }))
    throw new Error(body.error || `HTTP ${resp.status}`)
  }
  const text = await resp.text()
  if (!text) return undefined as T
  return JSON.parse(text)
}

async function apiGet<T>(path: string): Promise<T> {
  return apiFetch<T>(path)
}

async function apiPost<T>(path: string, body: unknown): Promise<T> {
  return apiFetch<T>(path, { method: 'POST', body: JSON.stringify(body) })
}

// ---------- State ----------
const steps = computed<StepDef[]>(() => {
  const base: StepDef[] = [
    { title: 'Organization', description: 'Select target organization' },
    { title: 'User Profile', description: 'Enter user details' },
  ]
  // Only show auth step if schema has x-auth-methods
  if (hasAuthMethods.value) {
    base.push({ title: 'Authentication', description: 'Set up login method' })
  }
  base.push({ title: 'Confirmation', description: 'Review and create' })
  return base
})

const currentStep = ref(0)
const loadingOrgs = ref(false)
const loadingSchema = ref(false)
const submitting = ref(false)
const errorMsg = ref('')

const availableOrgs = ref<any[]>([])
const selectedOrgs = ref<string[]>([])
const schemaVersions = ref<SchemaVersion[]>([])
const selectedSchemaId = ref('')
const authMethod = ref('invite')
const sendWelcomeMessage = ref(true)
const initialPassword = ref('')
const profileData = reactive<Record<string, string>>({})

// ---------- Schema introspection ----------
const currentSchema = computed<SchemaVersion | null>(() =>
  schemaVersions.value.find(s => s.id === selectedSchemaId.value) || null
)

const rawSchema = computed(() => (currentSchema.value?.schema as any) || {})

const authMethodsConfig = computed<Record<string, AuthMethodConfig>>(() =>
  rawSchema.value['x-auth-methods'] || {}
)

const hasAuthMethods = computed(() =>
  Object.keys(authMethodsConfig.value).length > 0
)

const enabledAuthMethods = computed(() => {
  const methods = authMethodsConfig.value
  return Object.entries(methods)
    .filter(([, cfg]) => cfg.enabled)
    .sort((a, b) => (a[1].position ?? 99) - (b[1].position ?? 99))
    .map(([key, cfg]) => ({ key, ...cfg }))
})

const hasPassword = computed(() => authMethodsConfig.value.password?.enabled ?? false)
const hasMagicLink = computed(() => authMethodsConfig.value.magic_link?.enabled ?? false)
const hasPasskey = computed(() => authMethodsConfig.value.passkey?.enabled ?? false)

const schemaFields = computed<SchemaField[]>(() => {
  const schemaProps = rawSchema.value.properties || {}
  const requiredFields: string[] = rawSchema.value.required || []

  return Object.entries(schemaProps)
    .filter(([, def]: [string, any]) => !def?.['x-hidden'])
    .map(([name, def]: [string, any]) => ({
      name,
      label: def?.title || name.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()),
      description: def?.description || '',
      type: def?.type || 'string',
      format: def?.format,
      enum: def?.enum,
      required: requiredFields.includes(name),
      xIdentifier: def?.['x-identifier'] ?? false,
      xUnique: def?.['x-unique'],
      xEditable: def?.['x-editable'] ?? true,
      xSensitive: def?.['x-sensitive'] ?? false,
      xVerify: def?.['x-verify'],
      minLength: def?.minLength,
      maxLength: def?.maxLength,
      pattern: def?.pattern,
      inputType:
        def?.type === 'integer' || def?.type === 'number' ? 'number'
        : def?.format === 'email' ? 'email'
        : def?.format === 'uri' ? 'url'
        : def?.['x-sensitive'] ? 'password'
        : 'text',
    }))
})

// Fields that are x-identifier (used for login)
const identifierFields = computed(() =>
  schemaFields.value.filter(f => f.xIdentifier)
)

// ---------- Validation ----------
const validationErrors = computed<Record<string, string>>(() => {
  const errors: Record<string, string> = {}

  for (const field of schemaFields.value) {
    const val = profileData[field.name] || ''

    // Required check
    if (field.required && (!val || val === 'none')) {
      errors[field.name] = `${field.label} is required`
      continue
    }

    if (!val || val === 'none') continue

    // Format-specific validation
    if (field.format === 'email' && val) {
      const emailRe = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
      if (!emailRe.test(val)) {
        errors[field.name] = 'Invalid email address'
      }
    }

    if (field.format === 'uri' && val) {
      try {
        new URL(val)
      } catch {
        errors[field.name] = 'Invalid URL'
      }
    }

    // Min/max length
    if (field.minLength && val.length < field.minLength) {
      errors[field.name] = `Minimum ${field.minLength} characters`
    }
    if (field.maxLength && val.length > field.maxLength) {
      errors[field.name] = `Maximum ${field.maxLength} characters`
    }

    // Pattern
    if (field.pattern && val) {
      try {
        if (!new RegExp(field.pattern).test(val)) {
          errors[field.name] = `Must match pattern: ${field.pattern}`
        }
      } catch { /* invalid regex in schema, skip */ }
    }
  }

  return errors
})

const profileDataOverview = computed(() => {
  const dict: Record<string, string> = {}
  for (const [k, v] of Object.entries(profileData)) {
    if (v && v !== 'none') {
      const field = schemaFields.value.find(f => f.name === k)
      // Don't show sensitive fields in review
      if (field?.xSensitive) {
        dict[k] = '••••••••'
      } else {
        dict[k] = v
      }
    }
  }
  return dict
})

const primaryIdentifier = computed(() => {
  // Prefer the first x-identifier field that has a value
  for (const field of identifierFields.value) {
    if (profileData[field.name]?.trim()) return profileData[field.name].trim()
  }
  // Fallback
  return profileData.email || profileData.username || profileData.display_name || ''
})

const selectedOrgNames = computed(() => {
  if (selectedOrgs.value.length === 0) return 'No organization'
  return selectedOrgs.value
    .map(orgId => availableOrgs.value.find(o => String(o.id) === orgId)?.name || orgId)
    .join(', ')
})

const authMethodLabel = computed(() => {
  if (authMethod.value === 'invite') return 'Email Invitation'
  if (authMethod.value === 'password') return 'Initial Password'
  if (authMethod.value === 'passwordless') return 'Passwordless'
  return authMethod.value
})

// ---------- Step logic (adapts to whether auth step is shown) ----------
const authStepIndex = computed(() => hasAuthMethods.value ? 2 : -1)
const confirmStepIndex = computed(() => hasAuthMethods.value ? 3 : 2)

const canProceed = computed(() => {
  // Step 1: Profile validation
  if (currentStep.value === 1) {
    // Must have at least a display_name or one identifier field
    if (!primaryIdentifier.value.trim()) return false
    // Check required schema fields
    for (const f of schemaFields.value) {
      if (f.required && (!profileData[f.name] || profileData[f.name] === 'none')) {
        return false
      }
    }
    // No validation errors
    if (Object.keys(validationErrors.value).length > 0) return false
    return true
  }
  // Step 2: Auth (if applicable)
  if (currentStep.value === authStepIndex.value) {
    if (authMethod.value === 'password' && !initialPassword.value) return false
  }
  return true
})

// ---------- Actions ----------
function toggleOrg(id: string) {
  if (selectedOrgs.value.includes(id)) {
    selectedOrgs.value = selectedOrgs.value.filter(o => o !== id)
  } else {
    selectedOrgs.value.push(id)
  }
}

function selectSchema(id: string) {
  selectedSchemaId.value = id
  // Reset profile data when switching schema versions
  Object.keys(profileData).forEach(k => delete profileData[k])
}

function updateProfileField(name: string, value: string) {
  profileData[name] = value
}

function prevStep() {
  if (currentStep.value > 0) currentStep.value--
}

function handleClose() {
  emit('update:open', false)
  emit('close')
}

async function nextStep() {
  if (currentStep.value < steps.value.length - 1) {
    currentStep.value++
    return
  }

  // Final submit
  submitting.value = true
  errorMsg.value = ''

  try {
    // Build capabilities from schema auth methods
    const caps: string[] = []
    if (hasPassword.value) caps.push('password')
    if (hasMagicLink.value) caps.push('magic_link')
    if (hasPasskey.value) caps.push('passkey')

    // Build profile dict with correct types
    const profileDict: Record<string, any> = {}
    for (const [k, v] of Object.entries(profileData)) {
      if (v !== '' && v !== 'none') {
        const fieldDef = schemaFields.value.find(f => f.name === k)
        if (fieldDef?.type === 'boolean') profileDict[k] = v === 'true'
        else if (fieldDef?.type === 'integer') profileDict[k] = parseInt(v) || 0
        else if (fieldDef?.type === 'number') profileDict[k] = parseFloat(v) || 0
        else profileDict[k] = v
      }
    }

    const payload = {
      schema_id: currentSchema.value?.id || props.schemaType,
      identifier: primaryIdentifier.value.trim(),
      display_name: profileData.display_name?.trim() || primaryIdentifier.value.trim(),
      profile: profileDict,
      capabilities: caps,
    }

    const created = await apiPost<any>('/v1/users', payload)

    // Create org memberships via the memberships API (canonical source of truth)
    if (created.id && selectedOrgs.value.length > 0) {
      for (const orgId of selectedOrgs.value) {
        // Resolve actual org ID from identifier
        const org = availableOrgs.value.find(
          (o: any) => o.identifier === orgId || String(o.id) === orgId
        )
        const resolvedOrgId = org?.id || orgId
        await apiPost(`/v1/orgs/${encodeURIComponent(resolvedOrgId)}/members`, {
          user_id: created.id,
          role: 'member',
        }).catch((err: any) => {
          console.error(`Failed to add user to org ${resolvedOrgId}:`, err)
        })
      }
    }

    // Set password if chosen
    if (authMethod.value === 'password' && created.id) {
      await apiPost(`/v1/users/${created.id}/password`, {
        password: initialPassword.value,
      }).catch((err: any) => {
        console.error('Failed to set password:', err)
      })
    }

    // Send invite
    if (authMethod.value === 'invite' && created.id && primaryIdentifier.value) {
      await apiPost('/v1/auth/magic-link', {
        email: primaryIdentifier.value.trim(),
      }).catch(console.error)
    }

    emit('created', created.id as string)
    emit('update:open', false)

  } catch (err: any) {
    console.error('Failed to create user', err)
    errorMsg.value = err.message || 'An error occurred while creating the user.'
    emit('error', errorMsg.value)
  } finally {
    submitting.value = false
  }
}

// ---------- Lifecycle: load data when dialog opens ----------
watch(() => props.open, async (isOpen) => {
  if (isOpen || props.standalone) {
    await initializeWizard()
  }
}, { immediate: true })

async function initializeWizard() {
  currentStep.value = 0
  errorMsg.value = ''
  submitting.value = false
  selectedOrgs.value = props.orgId ? [props.orgId] : []
  Object.keys(profileData).forEach(k => delete profileData[k])
  initialPassword.value = ''
  authMethod.value = 'invite'

  // Fetch schemas
  loadingSchema.value = true
  try {
    const resp = await apiGet<{ items: SchemaVersion[] }>('/v1/schemas?type=' + encodeURIComponent(props.schemaType))
    const schemas = resp?.items || []
    schemaVersions.value = schemas.sort((a, b) => b.version - a.version)
    const defaultSchema = schemas.find(s => s.is_default) || schemas[0]
    if (defaultSchema) {
      selectedSchemaId.value = defaultSchema.id
      // Set default auth method based on schema
      const methods = (defaultSchema.schema as any)?.['x-auth-methods'] || {}
      if (methods.magic_link?.enabled) authMethod.value = 'invite'
      else if (methods.password?.enabled) authMethod.value = 'password'
      else if (methods.passkey?.enabled) authMethod.value = 'passwordless'
    }
  } catch (err) {
    console.error('Failed to load schemas', err)
  } finally {
    loadingSchema.value = false
  }

  // Fetch orgs
  loadingOrgs.value = true
  try {
    const resp = await apiGet<{ items: any[] }>('/v1/orgs')
    availableOrgs.value = resp?.items || []
    // Auto-select if orgId prop was provided
    if (props.orgId && availableOrgs.value.some(o => o.identifier === props.orgId || String(o.id) === props.orgId)) {
      selectedOrgs.value = [props.orgId]
    }
  } catch (err) {
    console.error('Failed to load orgs', err)
  } finally {
    loadingOrgs.value = false
  }
}

// Expose for web component consumers
defineExpose({
  initializeWizard,
  currentStep,
  errorMsg,
})
</script>

<style scoped>
.wizard-standalone {
  --wizard-bg: var(--color-background, #fff);
  --wizard-fg: var(--color-foreground, #0a0a0a);
  font-family: var(--font-sans, 'Inter', ui-sans-serif, system-ui, sans-serif);
}

.wizard-container {
  background: var(--wizard-bg);
  color: var(--wizard-fg);
  border-radius: 0.75rem;
  border: 1px solid var(--color-border, hsl(240 5.9% 90%));
  overflow: hidden;
  max-width: 780px;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.1);
}
</style>
