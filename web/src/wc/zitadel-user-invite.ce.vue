<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <div class="space-y-4">
      <div>
        <h2 class="text-lg font-semibold tracking-tight">Invite User</h2>
        <p class="text-sm text-[var(--color-muted-foreground)]">Send an invitation email to a new user</p>
      </div>

      <div class="space-y-3">
        <div class="space-y-1.5">
          <label class="text-sm font-medium">Email Address</label>
          <input
            v-model="email"
            type="email"
            placeholder="user@example.com"
            class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            @keyup.enter="sendInvite"
          />
        </div>

        <!-- Schema type (optional) -->
        <div v-if="!schemaType && schemas.length > 1" class="space-y-1.5">
          <label class="text-sm font-medium">User Type</label>
          <select
            v-model="selectedSchemaType"
            class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm"
          >
            <option value="">Select type…</option>
            <option v-for="s in schemas" :key="s.type" :value="s.type">{{ s.type }}</option>
          </select>
        </div>

        <!-- Success message -->
        <div v-if="successMsg" class="rounded-md border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-700">
          {{ successMsg }}
        </div>

        <!-- Error -->
        <div v-if="error" class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
          {{ error }}
        </div>

        <button
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50 w-full"
          :disabled="!canSend || sending"
          @click="sendInvite"
        >{{ sending ? 'Sending…' : 'Send Invitation' }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

const TAG_NAME = 'zitadel-user-invite'

const props = withDefaults(defineProps<{
  apiBaseUrl?: string
  schemaType?: string
  orgId?: string
  darkMode?: string
}>(), {
  apiBaseUrl: '',
  schemaType: '',
  orgId: '',
  darkMode: '',
})

const isDark = computed(() => isDarkMode(props.darkMode))

let api: WCApiClient
const email = ref('')
const selectedSchemaType = ref(props.schemaType)
const schemas = ref<any[]>([])
const sending = ref(false)
const error = ref('')
const successMsg = ref('')

const canSend = computed(() => {
  return email.value.includes('@') && email.value.includes('.')
})

async function sendInvite() {
  if (!canSend.value || sending.value) return
  sending.value = true
  error.value = ''
  successMsg.value = ''

  try {
    // Create the user first with minimal profile
    const schemaType = props.schemaType || selectedSchemaType.value || 'human_user'
    
    // Find matching schema
    const matchingSchema = schemas.value.find(s => s.type === schemaType && s.is_default)
      || schemas.value.find(s => s.type === schemaType)
    
    if (!matchingSchema) {
      throw new Error(`No schema found for type "${schemaType}"`)
    }

    const createBody: Record<string, any> = {
      schema_id: matchingSchema.id,
      profile: { email: email.value },
    }
    if (props.orgId) createBody.org_ids = [props.orgId]

    const user = await api.post<any>('/v1/users', createBody)

    // Send magic link invitation
    await api.post<any>('/v1/auth/magic-link', {
      identifier: email.value,
      purpose: 'invite',
    })

    successMsg.value = `Invitation sent to ${email.value}`
    dispatchWCEvent(TAG_NAME, 'invite-sent', {
      email: email.value,
      user_id: user.id,
      purpose: 'invite',
    })

    email.value = ''
  } catch (e: any) {
    error.value = e?.message || 'Failed to send invitation'
    dispatchWCEvent(TAG_NAME, 'invite-error', { error: error.value })
  } finally {
    sending.value = false
  }
}

onMounted(async () => {
  api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
  // Load schemas to determine what types are available
  try {
    const data = await api.get<any>('/v1/schemas')
    schemas.value = data.items || []
  } catch {}
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
