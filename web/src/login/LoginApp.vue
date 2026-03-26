<template>
  <div class="login-shell" :style="shellStyle">
    <div class="login-card">
      <!-- Logo -->
      <div v-if="branding?.logo_url" class="logo">
        <img :src="branding.logo_url" :alt="branding.org_name">
      </div>
      <div v-else class="logo-text">{{ branding?.org_name || 'ZITADEL' }}</div>

      <!-- Error message (shown above any step) -->
      <div v-if="error" class="error-msg">{{ error }}</div>

      <!-- Loading state -->
      <template v-if="!flowStep">
        <div class="spinner"></div>
      </template>

      <!-- Node Renderer: render whatever the server sends -->
      <template v-else>
        <form @submit.prevent="onSubmit">
          <template v-for="(node, i) in flowStep.nodes" :key="i">
            <!-- Heading -->
            <h1 v-if="node.type === 'heading'">{{ node.text }}</h1>

            <!-- Description -->
            <p v-else-if="node.type === 'description'" class="subtitle">{{ node.text }}</p>

            <!-- Avatar -->
            <div v-else-if="node.type === 'avatar'" class="avatar">{{ node.initial }}</div>

            <!-- Icon -->
            <div v-else-if="node.type === 'icon'" class="check-email-icon">{{ node.text }}</div>

            <!-- Info block -->
            <div v-else-if="node.type === 'info'" class="info-box">{{ node.text }}</div>

            <!-- Spinner -->
            <div v-else-if="node.type === 'spinner'" class="spinner"></div>

            <!-- Text input -->
            <div v-else-if="node.type === 'input'" class="form-group">
              <label :for="node.name">{{ node.label }}</label>
              <input
                :id="node.name"
                v-model="formData[node.name!]"
                :type="node.input_type || 'text'"
                :placeholder="node.placeholder || ''"
                :autocomplete="node.autocomplete || 'off'"
                :required="node.required"
                :autofocus="i === firstInputIndex"
              >
            </div>

            <!-- Submit button -->
            <button
              v-else-if="node.type === 'submit'"
              type="submit"
              :disabled="loading"
              @click="pendingAction = node.action || ''"
            >
              {{ loading ? 'Loading...' : node.label }}
            </button>

            <!-- Divider -->
            <div v-else-if="node.type === 'divider'" class="divider"><span>or</span></div>

            <!-- Regular button (magic link, passkey, etc.) -->
            <button
              v-else-if="node.type === 'button'"
              type="button"
              class="alt-btn"
              :disabled="loading"
              @click="submitAction(node.action || '')"
            >
              {{ node.label }}
            </button>

            <!-- SSO button -->
            <button
              v-else-if="node.type === 'sso_button'"
              type="button"
              class="sso-btn"
              :class="'sso-' + node.template"
              @click="submitAction(node.action || 'sso', { provider_id: node.provider_id || '' })"
            >
              <span class="sso-icon">{{ ssoIcon(node.template || '') }}</span>
              {{ node.label }}
            </button>

            <!-- Link -->
            <a
              v-else-if="node.type === 'link'"
              class="back-link"
              @click="submitAction(node.action || 'back')"
            >{{ node.label }}</a>
          </template>
        </form>
      </template>
    </div>

    <div v-if="!branding?.hide_zitadel_branding" class="powered-by">
      Powered by ZITADEL
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, reactive, watch } from 'vue'
import { flowApi, type FlowStep, type FlowBranding, type FlowCompleteResponse } from '@/api/branding'

const flowStep = ref<FlowStep | null>(null)
const branding = ref<FlowBranding | null>(null)
const error = ref('')
const loading = ref(false)
const formData = reactive<Record<string, string>>({})
const pendingAction = ref('')

// Dynamic styles from branding.
const shellStyle = computed(() => {
  const c = branding.value?.colors || {}
  return {
    '--primary': c.primary || '#6366f1',
    '--background': c.background || '#f0f2ff',
    '--surface': c.surface || '#ffffff',
    '--text': c.text || '#1a1a2e',
    '--error': c.error || '#ef4444',
    background: `linear-gradient(135deg, ${c.background || '#f0f2ff'} 0%, #fafbff 50%, #f5f3ff 100%)`,
    fontFamily: branding.value?.font_family || 'Inter, system-ui, sans-serif',
  }
})

// Find the first input node index for autofocus.
const firstInputIndex = computed(() => {
  if (!flowStep.value) return -1
  return flowStep.value.nodes.findIndex(n => n.type === 'input')
})

const ssoIcons: Record<string, string> = {
  google: '🔵', entraid: '🟦', gitlab: '🦊', apple: '🍎', custom: '🔑',
}
function ssoIcon(template: string) { return ssoIcons[template] || '🔑' }

// Initialize: create a flow.
onMounted(async () => {
  try {
    const step = await flowApi.create()
    flowStep.value = step
    branding.value = step.branding
  } catch (e: any) {
    error.value = 'Failed to initialize login flow'
  }
})

// Form submit handler (triggered by submit buttons).
async function onSubmit() {
  const action = pendingAction.value || 'identifier'
  await submitAction(action)
}

// Universal action handler.
async function submitAction(action: string, extra?: Record<string, string>) {
  if (!flowStep.value) return
  loading.value = true
  error.value = ''

  try {
    const payload: Record<string, string> = { action, ...formData, ...extra }
    const resp = await flowApi.submit(flowStep.value.flow_id, action, payload)

    // Check if this is a completion response (has redirect_uri).
    if ('redirect_uri' in resp && (resp as FlowCompleteResponse).redirect_uri) {
      window.location.href = (resp as FlowCompleteResponse).redirect_uri
      return
    }

    // Check if this is an SSO redirect response.
    if ('redirect_url' in resp) {
      window.location.href = (resp as any).redirect_url
      return
    }

    // Update flow step with new nodes.
    const step = resp as FlowStep
    flowStep.value = step
    if (step.branding) branding.value = step.branding

    // Clear password field after submission.
    if (formData.password) formData.password = ''

  } catch (e: any) {
    const msg = e.message || 'Something went wrong'
    if (msg.includes('invalid_password')) {
      error.value = 'Invalid password. Please try again.'
    } else if (msg.includes('not found')) {
      error.value = 'Account not found.'
    } else {
      error.value = msg
    }
  } finally {
    loading.value = false
    pendingAction.value = ''
  }
}
</script>

<style scoped>
.login-shell {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  min-height: 100vh; padding: 2rem;
  -webkit-font-smoothing: antialiased;
}
.login-card {
  background: var(--surface, #fff); border-radius: 16px; padding: 2.5rem;
  width: 100%; max-width: 400px;
  box-shadow: 0 10px 25px rgba(0,0,0,.08), 0 4px 10px rgba(0,0,0,.04);
  border: 1px solid rgba(0,0,0,.06);
}
.logo img { max-height: 32px; margin-bottom: 1.5rem; }
.logo-text { font-size: 1.25rem; font-weight: 800; letter-spacing: -0.03em; color: var(--text); margin-bottom: 1.5rem; }
h1 { font-size: 1.5rem; font-weight: 700; color: var(--text); margin: 0 0 0.25rem; }
.subtitle { color: #6b7280; font-size: 0.875rem; margin: 0 0 1.5rem; }
.form-group { margin-bottom: 1.25rem; }
label { display: block; font-size: 0.8125rem; font-weight: 500; color: var(--text); margin-bottom: 0.375rem; }
input {
  width: 100%; padding: 0.625rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit; transition: border-color 0.2s, box-shadow 0.2s; box-sizing: border-box;
}
input:focus { outline: none; border-color: var(--primary); box-shadow: 0 0 0 3px rgba(99,102,241,.1); }
input::placeholder { color: #9ca3af; }

button[type="submit"] {
  width: 100%; padding: 0.625rem 1.25rem; background: var(--text, #1a1a2e); color: var(--surface, #fff);
  border: none; border-radius: 8px; font-size: 0.875rem; font-weight: 600;
  font-family: inherit; cursor: pointer; transition: all 0.15s; margin-top: 0.5rem;
}
button[type="submit"]:hover { opacity: 0.9; }
button:active { transform: scale(0.98); }
button:disabled { opacity: 0.6; cursor: not-allowed; }

.alt-btn {
  width: 100%; padding: 0.625rem 1.25rem;
  background: #fff; color: var(--text);
  border: 1px solid #e5e7eb; border-radius: 8px;
  font-size: 0.875rem; font-weight: 500;
  font-family: inherit; cursor: pointer;
  transition: all 0.15s; margin-top: 0.5rem;
}
.alt-btn:hover { border-color: var(--primary); color: var(--primary); background: #fafafe; }

.error-msg {
  background: rgba(239,68,68,.06); border: 1px solid rgba(239,68,68,.2);
  border-radius: 8px; padding: 0.625rem 0.875rem; color: var(--error); font-size: 0.875rem; margin-bottom: 1.25rem;
}
.back-link { display: inline-block; margin-top: 1rem; font-size: 0.8125rem; color: #6b7280; cursor: pointer; }
.back-link:hover { color: var(--primary); }
.avatar {
  width: 40px; height: 40px; border-radius: 50%; background: var(--primary); color: #fff;
  display: flex; align-items: center; justify-content: center; font-weight: 600; font-size: 1rem; margin-bottom: 1rem;
}
.powered-by { text-align: center; margin-top: 2rem; font-size: 0.75rem; color: #9ca3af; }
.spinner {
  width: 24px; height: 24px; border: 3px solid #e5e7eb; border-top-color: var(--primary);
  border-radius: 50%; animation: spin 0.6s linear infinite; margin: 2rem auto;
}
@keyframes spin { to { transform: rotate(360deg); } }

.divider {
  display: flex; align-items: center; gap: 0.75rem; margin: 1.25rem 0;
  color: #9ca3af; font-size: 0.75rem;
}
.divider::before, .divider::after {
  content: ''; flex: 1; height: 1px; background: #e5e7eb;
}

.check-email-icon { font-size: 2.5rem; margin-bottom: 1rem; text-align: center; }
.info-box {
  background: #f0f2ff; border-radius: 10px; padding: 1rem;
  margin-bottom: 1.25rem; font-size: 0.8125rem; color: #4b5563; line-height: 1.5;
}

.sso-btn {
  width: 100%; padding: 0.625rem 1rem;
  border: 1px solid #e5e7eb; border-radius: 10px;
  background: #fff; color: var(--text);
  font-size: 0.875rem; font-weight: 500;
  font-family: inherit; cursor: pointer;
  transition: all 0.15s; margin-top: 0.5rem;
  display: flex; align-items: center; gap: 0.5rem; justify-content: center;
}
.sso-btn:hover { border-color: var(--primary); background: #fafafe; }
.sso-icon { font-size: 1.1rem; }
.sso-google:hover { border-color: #4285f4; }
.sso-entraid:hover { border-color: #00a4ef; }
.sso-gitlab:hover { border-color: #fc6d26; }
.sso-apple:hover { border-color: #000; }
</style>
