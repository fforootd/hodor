import { api, requestJSON, requestText } from './client'
import { generateTraceparent, getFlowId, getDeviceFingerprint } from '@/lib/telemetry'

export interface Branding {
  org_id: string
  org_name: string
  logo_url: string
  heading: string
  description: string
  colors: Record<string, string>
  font_family: string
  hide_zitadel_branding: boolean
}

// --- Flow API (schema-driven) — ADR-019 ---

export interface UINode {
  type: string // heading, description, input, submit, button, divider, sso_button, avatar, link, icon, info, spinner, error, hidden, registration_link, group
  name?: string // form field name
  input_type?: string // text, password, email
  label?: string
  text?: string
  placeholder?: string
  autocomplete?: string
  required?: boolean
  action?: string // identifier, password, magic_link, sso, back, register, register_submit, resend_magic_link, passkey
  provider_id?: string
  provider_name?: string
  template?: string // google, entraid, etc.
  initial?: string // avatar initial
  value?: string // pre-filled value
  disabled?: boolean
  errors?: string[] // per-field validation errors
  attributes?: Record<string, string>
  children?: UINode[] // nested nodes (group containers)
  min_length?: number
  max_length?: number
  pattern?: string
}

export interface ConsentItem {
  id: string
  label: string // supports markdown links
  required: boolean
}

export interface FlowBranding {
  heading: string
  description: string
  logo_url: string
  org_name: string
  colors: Record<string, string>
  font_family: string
  font_url: string
  texts: Record<string, string>
  custom_css: string
  hide_zitadel_branding: boolean
  // Layout & visual
  layout: string // "centered" | "split" | "muted" | "card_image" | "minimal"
  dark_mode: string // "light" | "dark" | "auto"
  cover_image: string // URL for split/card_image layouts
  logo_dark: string // Alt logo for dark mode
  favicon: string
  border_radius: string // "sm" | "md" | "lg" | "xl" | "full"
  // Legal
  terms_url: string
  privacy_url: string
  // Social
  social_position: string // "top" | "bottom"
  // Consent
  consent: ConsentItem[]
}

export interface FlowError {
  code: string
  message: string
}

export interface FlowMessage {
  type: string // "info" | "warning" | "success"
  text: string
}

export interface FlowIdentity {
  display_name: string
  avatar_initial: string
}

export interface FlowStep {
  flow_id: string
  step: string
  nodes: UINode[]
  branding: FlowBranding
  captcha_required?: boolean
  captcha_verified?: boolean
  identity?: FlowIdentity
  errors?: FlowError[]
  messages?: FlowMessage[]
  css?: string
}

export interface FlowCompleteResponse {
  flow_id: string
  step: string
  session_id: string
  redirect_uri: string
}

export const brandingApi = {
  get: (domain?: string) => api.get<Branding>(`/v1/branding${domain ? `?domain=${domain}` : ''}`),
}

/** Build flow correlation headers for Traceparent, X-Flow-ID, and X-Fingerprint propagation. */
function flowHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    Traceparent: generateTraceparent(),
  }
  const flowId = getFlowId()
  if (flowId) {
    headers['X-Flow-Id'] = flowId
  }
  const fingerprint = getDeviceFingerprint()
  if (fingerprint) {
    headers['X-Fingerprint'] = fingerprint
  }
  return headers
}

export const flowApi = {
  create: (baseUrl = '', redirectUri?: string, state?: string, authRequestId?: string) => {
    const body: Record<string, string> = {}
    if (redirectUri) body.redirect_uri = redirectUri
    if (state) body.state = state
    if (authRequestId) body.auth_request_id = authRequestId
    const fingerprint = getDeviceFingerprint()
    if (fingerprint) body.fingerprint = fingerprint
    return requestJSON<FlowStep | FlowCompleteResponse>(
      '/v1/login/flows',
      {
        method: 'POST',
        body: JSON.stringify(body),
        headers: flowHeaders(),
      },
      baseUrl,
    )
  },

  submit: (baseUrl = '', flowId: string, action: string, data?: Record<string, string>) =>
    requestJSON<FlowStep | FlowCompleteResponse>(
      `/v1/login/flows/${flowId}/submit`,
      {
        method: 'POST',
        body: JSON.stringify({ action, ...data }),
        headers: flowHeaders(),
      },
      baseUrl,
    ),

  get: (baseUrl = '', flowId: string) =>
    requestJSON<FlowStep>(`/v1/login/flows/${flowId}`, { headers: flowHeaders() }, baseUrl),

  captchaChallenge: (baseUrl = '', flowId: string) =>
    requestJSON<Record<string, unknown>>(
      `/v1/login/flows/${flowId}/captcha/challenge`,
      { headers: flowHeaders() },
      baseUrl,
    ),

  ready: async (baseUrl = '') => {
    const text = await requestText('/readyz', { headers: { Accept: 'text/plain' } }, baseUrl)
    return text.trim() === 'ready'
  },
}
