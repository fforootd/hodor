<template>
  <div class="login-shell">
    <div class="login-card">
      <div v-if="branding.logo_url" class="logo">
        <img :src="branding.logo_url" :alt="branding.org_name">
      </div>
      <div v-else class="logo-text">{{ branding.org_name || 'ZITADEL' }}</div>

      <!-- Identifier step -->
      <template v-if="step === 'identifier'">
        <h1>{{ branding.heading }}</h1>
        <p class="subtitle">{{ branding.description }}</p>
        <div v-if="error" class="error-msg">{{ error }}</div>
        <form @submit.prevent="onIdentifierSubmit">
          <div class="form-group">
            <label for="identifier">Email or username</label>
            <input
              id="identifier"
              v-model="identifier"
              type="text"
              placeholder="you@example.com"
              autocomplete="username"
              autofocus
            >
          </div>
          <button type="submit" :disabled="loading">
            {{ loading ? 'Checking...' : 'Continue' }}
          </button>
        </form>
      </template>

      <!-- Password step (with magic link option) -->
      <template v-if="step === 'password'">
        <div class="avatar">{{ displayInitial }}</div>
        <h1>{{ displayName }}</h1>
        <p class="subtitle">Choose how to sign in</p>
        <div v-if="error" class="error-msg">{{ error }}</div>
        <form @submit.prevent="onPasswordSubmit">
          <div class="form-group">
            <label for="password">Password</label>
            <input
              id="password"
              v-model="password"
              type="password"
              placeholder="••••••••"
              autocomplete="current-password"
              autofocus
            >
          </div>
          <button type="submit" :disabled="loading">
            {{ loading ? 'Signing in...' : 'Sign in with password' }}
          </button>
        </form>
        <div class="divider"><span>or</span></div>
        <button class="magic-link-btn" @click="onSendMagicLink" :disabled="loading">
          {{ loading ? 'Sending...' : '✉ Send me a sign-in link' }}
        </button>
        <template v-if="ssoProviders.length">
          <button
            v-for="p in ssoProviders"
            :key="p.id"
            class="sso-btn"
            :class="'sso-' + p.template"
            @click="startSSO(p.id)"
          >
            <span class="sso-icon">{{ ssoIcon(p.template) }}</span>
            Continue with {{ p.name }}
          </button>
        </template>
        <a class="back-link" @click="goBack">← Use a different account</a>
      </template>

      <!-- Magic link sent step -->
      <template v-if="step === 'magic-link-sent'">
        <div class="check-email-icon">✉</div>
        <h1>Check your email</h1>
        <p class="subtitle">We sent a sign-in link to <strong>{{ identifier }}</strong></p>
        <div class="check-email-info">
          <p>Click the link in your email to sign in. The link expires in 15 minutes.</p>
          <p class="hint">Didn't get the email? Check your spam folder or try again.</p>
        </div>
        <button class="magic-link-btn" @click="onSendMagicLink" :disabled="loading">
          {{ loading ? 'Sending...' : 'Resend link' }}
        </button>
        <a class="back-link" @click="goBack">← Back to sign in</a>
      </template>

      <!-- Complete step -->
      <template v-if="step === 'complete'">
        <h1>Welcome!</h1>
        <p class="subtitle">Redirecting you now...</p>
        <div class="spinner"></div>
      </template>
    </div>

    <div v-if="!branding.hide_zitadel_branding" class="powered-by">
      Powered by ZITADEL
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { api } from '@/api/client'
import { brandingApi, loginApi, type Branding } from '@/api/branding'

const DEFAULT_BRANDING: Branding = {
  org_id: '', org_name: 'ZITADEL', logo_url: '',
  heading: 'Welcome back', description: 'Sign in to your account',
  colors: { primary: '#6366f1', background: '#f0f2ff', surface: '#fff', text: '#1a1a2e', error: '#ef4444' },
  font_family: 'Inter, system-ui, sans-serif', hide_zitadel_branding: false,
}

type Step = 'loading' | 'identifier' | 'password' | 'magic-link-sent' | 'complete'

const step = ref<Step>('loading')
const branding = ref<Branding>(DEFAULT_BRANDING)
const error = ref('')
const loading = ref(false)
const identifier = ref('')
const password = ref('')
const loginSessionId = ref('')
const displayName = ref('')
const ssoProviders = ref<any[]>([])

const displayInitial = computed(() =>
  (displayName.value || identifier.value || '?')[0].toUpperCase()
)

const ssoIcons: Record<string, string> = {
  google: '🔵',
  entraid: '🟦',
  gitlab: '🦊',
  apple: '🍎',
  custom: '🔑',
}
function ssoIcon(template: string) { return ssoIcons[template] || '🔑' }

function startSSO(providerId: string) {
  window.location.href = `/v1/auth/sso/${providerId}/start`
}

onMounted(async () => {
  try {
    branding.value = await brandingApi.get()
  } catch { /* use defaults */ }
  // Load auth settings to get SSO providers.
  try {
    const settings = await api.get<any>('/v1/auth/settings')
    const sso = settings?.auth_methods?.sso
    if (sso?.enabled && sso?.providers?.length) {
      ssoProviders.value = sso.providers
    }
  } catch { /* no settings */ }
  step.value = 'identifier'
})

async function onIdentifierSubmit() {
  if (!identifier.value.trim()) { error.value = 'Please enter your email or username'; return }
  loading.value = true; error.value = ''
  try {
    const resp = await loginApi.start(identifier.value.trim())
    loginSessionId.value = resp.login_session_id
    displayName.value = resp.display_name
    step.value = 'password'
  } catch (e: any) {
    error.value = e.message || 'Unable to find account'
  } finally { loading.value = false }
}

async function onPasswordSubmit() {
  if (!password.value) { error.value = 'Please enter your password'; return }
  loading.value = true; error.value = ''
  try {
    const resp = await loginApi.password(loginSessionId.value, password.value)
    if (resp.error) { error.value = resp.error === 'invalid_password' ? 'Invalid password. Please try again.' : resp.error; return }
    step.value = 'complete'
    const complete = await loginApi.complete(loginSessionId.value)
    window.location.href = complete.redirect_uri || '/console'
  } catch { error.value = 'Sign in failed. Please try again.' }
  finally { loading.value = false }
}

async function onSendMagicLink() {
  loading.value = true; error.value = ''
  try {
    await loginApi.magicLink(identifier.value.trim())
    step.value = 'magic-link-sent'
  } catch (e: any) {
    error.value = e.message || 'Failed to send magic link'
  } finally { loading.value = false }
}

function goBack() {
  step.value = 'identifier'; error.value = ''; password.value = ''
  loginSessionId.value = ''; displayName.value = ''
}
</script>

<style scoped>
.login-shell {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  min-height: 100vh; padding: 2rem;
  font-family: Inter, system-ui, sans-serif;
  background: linear-gradient(135deg, #f0f2ff 0%, #fafbff 50%, #f5f3ff 100%);
  -webkit-font-smoothing: antialiased;
}
.login-card {
  background: #fff; border-radius: 16px; padding: 2.5rem;
  width: 100%; max-width: 400px;
  box-shadow: 0 10px 25px rgba(0,0,0,.08), 0 4px 10px rgba(0,0,0,.04);
  border: 1px solid rgba(0,0,0,.06);
}
.logo img { max-height: 32px; margin-bottom: 1.5rem; }
.logo-text { font-size: 1.25rem; font-weight: 800; letter-spacing: -0.03em; color: #1a1a2e; margin-bottom: 1.5rem; }
h1 { font-size: 1.5rem; font-weight: 700; color: #1a1a2e; margin: 0 0 0.25rem; }
.subtitle { color: #6b7280; font-size: 0.875rem; margin: 0 0 2rem; }
.form-group { margin-bottom: 1.25rem; }
label { display: block; font-size: 0.8125rem; font-weight: 500; color: #1a1a2e; margin-bottom: 0.375rem; }
input {
  width: 100%; padding: 0.625rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit; transition: border-color 0.2s, box-shadow 0.2s; box-sizing: border-box;
}
input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 3px rgba(99,102,241,.1); }
input::placeholder { color: #9ca3af; }
button[type="submit"] {
  width: 100%; padding: 0.625rem 1.25rem; background: #1a1a2e; color: #fff;
  border: none; border-radius: 8px; font-size: 0.875rem; font-weight: 600;
  font-family: inherit; cursor: pointer; transition: background 0.15s, transform 0.1s; margin-top: 0.5rem;
}
button:hover { background: #2d2d4e; }
button:active { transform: scale(0.98); }
button:disabled { opacity: 0.6; cursor: not-allowed; }
.error-msg {
  background: rgba(239,68,68,.06); border: 1px solid rgba(239,68,68,.2);
  border-radius: 8px; padding: 0.625rem 0.875rem; color: #ef4444; font-size: 0.875rem; margin-bottom: 1.25rem;
}
.back-link { display: inline-block; margin-top: 1rem; font-size: 0.8125rem; color: #6b7280; cursor: pointer; }
.back-link:hover { color: #6366f1; }
.avatar {
  width: 40px; height: 40px; border-radius: 50%; background: #6366f1; color: #fff;
  display: flex; align-items: center; justify-content: center; font-weight: 600; font-size: 1rem; margin-bottom: 1rem;
}
.powered-by { text-align: center; margin-top: 2rem; font-size: 0.75rem; color: #9ca3af; }
.spinner {
  width: 24px; height: 24px; border: 3px solid #e5e7eb; border-top-color: #6366f1;
  border-radius: 50%; animation: spin 0.6s linear infinite; margin: 2rem auto;
}
@keyframes spin { to { transform: rotate(360deg); } }

/* Divider */
.divider {
  display: flex; align-items: center; gap: 0.75rem; margin: 1.25rem 0;
  color: #9ca3af; font-size: 0.75rem;
}
.divider::before, .divider::after {
  content: ''; flex: 1; height: 1px; background: #e5e7eb;
}

/* Magic link button */
.magic-link-btn {
  width: 100%; padding: 0.625rem 1.25rem;
  background: #fff; color: #1a1a2e;
  border: 1px solid #e5e7eb; border-radius: 8px;
  font-size: 0.875rem; font-weight: 500;
  font-family: inherit; cursor: pointer;
  transition: all 0.15s;
}
.magic-link-btn:hover { border-color: #6366f1; color: #6366f1; background: #fafafe; }
.magic-link-btn:disabled { opacity: 0.6; cursor: not-allowed; }

/* Check email */
.check-email-icon {
  font-size: 2.5rem; margin-bottom: 1rem; text-align: center;
}
.check-email-info {
  background: #f0f2ff; border-radius: 10px; padding: 1rem;
  margin-bottom: 1.25rem; font-size: 0.8125rem; color: #4b5563; line-height: 1.5;
}
.check-email-info p { margin: 0 0 0.5rem; }
.check-email-info p:last-child { margin: 0; }
.hint { color: #9ca3af; font-size: 0.75rem; }

/* SSO provider buttons */
.sso-btn {
  width: 100%; padding: 0.625rem 1rem;
  border: 1px solid #e5e7eb; border-radius: 10px;
  background: #fff; color: #1a1a2e;
  font-size: 0.875rem; font-weight: 500;
  font-family: inherit; cursor: pointer;
  transition: all 0.15s; margin-top: 0.5rem;
  display: flex; align-items: center; gap: 0.5rem;
  justify-content: center;
}
.sso-btn:hover { border-color: #6366f1; background: #fafafe; }
.sso-icon { font-size: 1.1rem; }
.sso-google:hover { border-color: #4285f4; }
.sso-entraid:hover { border-color: #00a4ef; }
.sso-gitlab:hover { border-color: #fc6d26; }
.sso-apple:hover { border-color: #000; }
</style>
