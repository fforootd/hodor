import { api } from './client'

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

export interface LoginStartResponse {
  login_session_id: string
  identity_id: number
  display_name: string
  auth_methods: string[]
  next_step: string
}

export interface LoginPasswordResponse {
  next_step?: string
  error?: string
}

export interface LoginCompleteResponse {
  session_id: number
  redirect_uri: string
}

export const brandingApi = {
  get: (domain?: string) =>
    api.get<Branding>(`/v1/branding${domain ? `?domain=${domain}` : ''}`),
}

export const loginApi = {
  start: (identifier: string) =>
    api.post<LoginStartResponse>('/v1/login/start', { identifier }),

  password: (loginSessionId: string, password: string) =>
    api.post<LoginPasswordResponse>('/v1/login/password', {
      login_session_id: loginSessionId,
      password,
    }),

  complete: (loginSessionId: string) =>
    api.post<LoginCompleteResponse>('/v1/login/complete', {
      login_session_id: loginSessionId,
    }),

  magicLink: (email: string) =>
    api.post<{ status: string; purpose: string; message: string }>(
      '/v1/auth/magic-link', { email }
    ),
}

// --- Flow API (schema-driven) ---

export interface UINode {
  type: string           // heading, description, input, submit, button, divider, sso_button, avatar, link, icon, info, spinner
  name?: string          // form field name
  input_type?: string    // text, password, email
  label?: string
  text?: string
  placeholder?: string
  autocomplete?: string
  required?: boolean
  action?: string        // identifier, password, magic_link, sso, passkey, back, resend_magic_link
  provider_id?: string
  provider_name?: string
  template?: string      // google, entraid, etc.
  initial?: string       // avatar initial
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
  identity?: FlowIdentity
}

export interface FlowCompleteResponse {
  flow_id: string
  step: string
  session_id: number
  redirect_uri: string
}

export const flowApi = {
  create: () =>
    api.post<FlowStep>('/v1/login/flows', {}),

  submit: (flowId: string, action: string, data?: Record<string, string>) =>
    api.post<FlowStep | FlowCompleteResponse>(`/v1/login/flows/${flowId}/submit`, { action, ...data }),

  get: (flowId: string) =>
    api.get<FlowStep>(`/v1/login/flows/${flowId}`),
}

