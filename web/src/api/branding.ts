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

// --- Flow API (schema-driven) — ADR-019 ---

export interface UINode {
  type: string           // heading, description, input, submit, button, divider, sso_button, avatar, link, icon, info, spinner, error, hidden, registration_link, group
  name?: string          // form field name
  input_type?: string    // text, password, email
  label?: string
  text?: string
  placeholder?: string
  autocomplete?: string
  required?: boolean
  action?: string        // identifier, password, magic_link, sso, back, register, register_submit, resend_magic_link, passkey
  provider_id?: string
  provider_name?: string
  template?: string      // google, entraid, etc.
  initial?: string       // avatar initial
  value?: string         // pre-filled value
  disabled?: boolean
  errors?: string[]      // per-field validation errors
  attributes?: Record<string, string>
  children?: UINode[]    // nested nodes (group containers)
  min_length?: number
  max_length?: number
  pattern?: string
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

export interface FlowError {
  code: string
  message: string
}

export interface FlowMessage {
  type: string  // "info" | "warning" | "success"
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
  identity?: FlowIdentity
  errors?: FlowError[]
  messages?: FlowMessage[]
  css?: string
}

export interface FlowCompleteResponse {
  flow_id: string
  step: string
  session_id: number
  redirect_uri: string
}

export const brandingApi = {
  get: (domain?: string) =>
    api.get<Branding>(`/v1/branding${domain ? `?domain=${domain}` : ''}`),
}

export const flowApi = {
  create: (redirectUri?: string, state?: string) => {
    const body: Record<string, string> = {}
    if (redirectUri) body.redirect_uri = redirectUri
    if (state) body.state = state
    return api.post<FlowStep>('/v1/login/flows', body)
  },

  submit: (flowId: string, action: string, data?: Record<string, string>) =>
    api.post<FlowStep | FlowCompleteResponse>(`/v1/login/flows/${flowId}/submit`, { action, ...data }),

  get: (flowId: string) =>
    api.get<FlowStep>(`/v1/login/flows/${flowId}`),
}
