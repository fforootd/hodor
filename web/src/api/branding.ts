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
