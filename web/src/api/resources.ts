import { api } from './client'

export interface Identity {
  id: string
  org_id: string
  identifier: string
  display_name: string
  state: string
  schema_id?: string
  schema_name?: string
  profile: Record<string, unknown>
  metadata: Record<string, unknown>
  data?: Record<string, unknown> | string
  capabilities: string[]
  created_at: string
  updated_at: string
}

export interface Schema {
  id: string
  name?: string
  type: string
  description?: string
  org_id: number
  schema: Record<string, unknown>
  version: number
  is_default: boolean
  message: string
  created_by: string
  created_at: string
}

export interface Session {
  id: string
  identity_id: string
  identifier?: string
  user_agent: string
  ip_address: string
  created_at: string
  expires_at: string
  revoked_at?: string
}

export interface Event {
  id: string
  event_type: string
  actor_id: string
  aggregate_id: string
  aggregate_type: string
  payload: Record<string, unknown>
  created_at: string
}

// API returns list responses wrapped in { items: T[], next_cursor?, total? }
interface ListResponse<T> {
  items: T[]
  next_cursor?: string
  total?: number
}

export const identityApi = {
  list: () => api.get<ListResponse<Identity>>('/v1/identities').then(r => r.items || []),
  get: (id: string) => api.get<Identity>(`/v1/identities/${id}`),
  create: (data: Partial<Identity>) => api.post<Identity>('/v1/identities', data),
  update: (id: string, data: Partial<Identity>) =>
    api.patch<Identity>(`/v1/identities/${id}`, data),
  delete: (id: string) => api.delete<void>(`/v1/identities/${id}`),
}

export const schemaApi = {
  list: () => api.get<ListResponse<Schema>>('/v1/schemas').then(r => r.items || []),
  listByType: (type: string) => api.get<ListResponse<Schema>>(`/v1/schemas?type=${encodeURIComponent(type)}`).then(r => r.items || []),
  get: (id: string) => api.get<Schema>(`/v1/schemas/${id}`),
  update: (id: string, schema: Record<string, unknown>, message?: string) =>
    api.patch<Schema>(`/v1/schemas/${id}`, { schema, message: message || '' }),
  promote: (id: string) =>
    api.post<{ status: string; version: number; affected_entities: number }>(`/v1/schemas/${id}/promote`, {}),
  diff: (id: string, compareId: string) =>
    api.get<{ left: any; right: any; changes: any[] }>(`/v1/schemas/${id}/diff?compare=${compareId}`),
  preview: (id: string, entityId: string) =>
    api.post<{ entity: string; current_claims: Record<string, any>; draft_claims: Record<string, any>; changes: any[] }>(
      `/v1/schemas/${id}/preview`, { entity_id: entityId }
    ),
  identityCount: (id: string) =>
    api.get<{ count: number }>(`/v1/schemas/${id}/identity-count`).then(r => r.count),
}

export const sessionApi = {
  list: () => api.get<ListResponse<Session>>('/v1/sessions').then(r => r.items || []),
  revoke: (id: string) => api.post<void>(`/v1/sessions/${id}/revoke`, {}),
}

export const eventApi = {
  list: (params?: { type?: string; limit?: number }) => {
    const qs = new URLSearchParams()
    if (params?.type) qs.set('type', params.type)
    if (params?.limit) qs.set('limit', String(params.limit))
    return api.get<ListResponse<Event>>(`/v1/events?${qs}`).then(r => r.items || [])
  },
}

export interface SearchResult {
  resource_type: string
  id: string
  title: string
  subtitle: string
  link: string
}

export const searchApi = {
  search: (q: string, limit = 10) =>
    api.get<{ results: SearchResult[]; query: string; count: number }>(
      `/v1/search?q=${encodeURIComponent(q)}&limit=${limit}`
    ),
}

export const magicLinkApi = {
  send: (email: string) =>
    api.post<{ status: string; purpose: string; message: string }>(
      '/v1/auth/magic-link', { email }
    ),
}
