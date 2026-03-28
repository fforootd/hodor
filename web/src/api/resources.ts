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
  entity_id: string
  identity_id?: string
  identifier?: string
  user_agent?: string
  ip_address?: string
  created_at: string
  last_active?: string
  expires_at?: string
  revoked_at?: string
  state?: string
  auth_method?: string
  mfa_verified?: boolean
  geo?: {
    country?: string
    city?: string
    lat?: number
    lng?: number
  }
}

export interface Event {
  id: string
  event_type: string
  actor_id: string
  actor_type?: string
  aggregate_id: string
  aggregate_type: string
  session_id?: string
  trace_id?: string
  span_id?: string
  parent_span_id?: string
  payload: Record<string, unknown>
  metadata?: Record<string, unknown>
  created_at: string
}

// API returns list responses wrapped in { items: T[], next_cursor?, total? }
interface ListResponse<T> {
  items: T[]
  next_cursor?: string
  total?: number
}

export const entityApi = {
  list: () => api.get<ListResponse<Identity>>('/v1/entities').then(r => r.items || []),
  get: (id: string) => api.get<Identity>(`/v1/entities/${id}`),
  create: (data: Partial<Identity>) => api.post<Identity>('/v1/entities', data),
  update: (id: string, data: Partial<Identity>) =>
    api.patch<Identity>(`/v1/entities/${id}`, data),
  delete: (id: string) => api.delete<void>(`/v1/entities/${id}`),
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
  entityCount: (id: string) =>
    api.get<{ count: number }>(`/v1/schemas/${id}/identity-count`).then(r => r.count),
  previewUpgrade: (type: string, newSchema: Record<string, any>, sampleSize = 10) =>
    api.post<UpgradeReport>(`/v1/schemas/${encodeURIComponent(type)}/preview-upgrade`, {
      new_schema: newSchema, sample_size: sampleSize,
    }),
}

// --- Catalog (ADR-015) ---

export interface CatalogTemplate {
  id: string
  name: string
  type: string        // action, provider, login_flow, authorization
  version: string
  description: string
  tags: string[]
  source: string      // embedded | remote
}

export interface CatalogVariable {
  type: string        // string, integer, boolean
  description?: string
  default?: any
}

export interface CatalogTemplateDetail {
  template: CatalogTemplate
  variables: Record<string, CatalogVariable>
  payload: Record<string, any>
}

export interface UpgradeFieldChange {
  path: string
  change: string      // field_added, field_removed, type_changed, required_added, required_removed
  description: string
  severity: string    // info, warning, breaking
  affected_estimate?: number
}

export interface UpgradeEntityResult {
  id: string
  display_name: string
  status: string      // valid, warning, breaking
  changes?: { path: string; issue: string; current_value: any; suggestion?: string }[]
}

export interface UpgradeReport {
  schema_type: string
  total_entities: number
  sampled: number
  impact: { valid: number; warnings: number; breaking: number }
  field_changes: UpgradeFieldChange[]
  sample_entities: UpgradeEntityResult[]
}

export const catalogApi = {
  list: (type?: string, tag?: string) => {
    const qs = new URLSearchParams()
    if (type) qs.set('type', type)
    if (tag) qs.set('tags', tag)
    const q = qs.toString()
    return api.get<{ templates: CatalogTemplate[]; total: number }>(
      `/v1/catalog${q ? '?' + q : ''}`
    ).then(r => r.templates || [])
  },
  get: (id: string) =>
    api.get<CatalogTemplateDetail>(`/v1/catalog/${encodeURIComponent(id)}`),
  install: (id: string, variables: Record<string, any>) =>
    api.post<{ id: string; template_id: string; status: string }>(
      `/v1/catalog/${encodeURIComponent(id)}/install`, { variables }
    ),
  refresh: () =>
    api.post<{ status: string; new: number }>('/v1/catalog/refresh', {}),
}

export const sessionApi = {
  list: () => api.get<ListResponse<Session>>('/v1/sessions').then(r => r.items || []),
  revoke: (id: string) => api.post<void>(`/v1/sessions/${id}/revoke`, {}),
}

export const eventApi = {
  list: (params?: { type?: string; limit?: number; session_id?: string }) => {
    const qs = new URLSearchParams()
    if (params?.type) qs.set('type', params.type)
    if (params?.limit) qs.set('limit', String(params.limit))
    if (params?.session_id) qs.set('session_id', params.session_id)
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

// Meta schema (catalog + groups)
export const metaSchemaApi = {
  get: () => api.get<Record<string, any>>('/v1/schemas/$meta'),
}

// Organization management
export interface Org {
  id: number
  identifier: string
  display_name: string
}

export const orgApi = {
  list: () => api.get<ListResponse<Org>>('/v1/orgs').then(r => r.items || []),
}

// Batch counts for sidebar badges
export const countsApi = {
  get: () => api.get<Record<string, number>>('/v1/counts'),
}

// Provider management
export interface Provider {
  id: string
  name: string
  type: string
  template?: string
  enabled: boolean
  config: Record<string, unknown>
  created_at: string
}

export interface ProviderTemplate {
  template: string
  name: string
  type: string
  fields: Record<string, any>[]
}

export const providerApi = {
  list: () => api.get<ListResponse<Provider>>('/v1/providers').then(r => r.items || []),
  templates: () => api.get<{ templates: ProviderTemplate[] }>('/v1/providers/templates').then(r => r.templates || []),
  create: (data: Record<string, unknown>) => api.post<Provider>('/v1/providers', data),
  update: (id: string, data: Record<string, unknown>) => api.patch<Provider>(`/v1/providers/${id}`, data),
  delete: (id: string) => api.delete<void>(`/v1/providers/${id}`),
}

// Analytics
export const analyticsApi = {
  query: (body: Record<string, unknown>) =>
    api.post<Record<string, any>>('/v1/analytics/query', body),
  schema: () =>
    api.get<Record<string, any>>('/v1/analytics/schema'),
  tables: () =>
    api.get<Record<string, any>>('/v1/analytics/tables'),
}

// FGA (Fine-Grained Authorization) management
export interface FGATuple {
  user: string
  relation: string
  object: string
}

export interface FGAModelNode {
  id: string
  relations: string[]
  permissions: string[]
}

export interface FGAModelEdge {
  from: string
  to: string
  relation: string
  kind: string
}

export interface FGACheckResult {
  allowed: boolean
  user: string
  relation: string
  object: string
}

export interface FGATestResult {
  user: string
  relation: string
  object: string
  expected: boolean
  actual: boolean
  pass: boolean
  error?: string
}

export const fgaApi = {
  getModel: () =>
    api.get<{ schema_version: string; types: { type: string; relations: string[] }[] }>('/v1/fga/model'),
  getModelGraph: () =>
    api.get<{ nodes: FGAModelNode[]; edges: FGAModelEdge[] }>('/v1/fga/model/graph'),
  check: (user: string, relation: string, object: string) =>
    api.post<FGACheckResult>('/v1/fga/check', { user, relation, object }),
  readTuples: (params?: { user?: string; relation?: string; object?: string }) => {
    const qs = new URLSearchParams()
    if (params?.user) qs.set('user', params.user)
    if (params?.relation) qs.set('relation', params.relation)
    if (params?.object) qs.set('object', params.object)
    return api.get<{ tuples: FGATuple[] }>(`/v1/fga/tuples?${qs}`).then(r => r.tuples || [])
  },
  writeTuples: (tuples: FGATuple[]) =>
    api.post<{ status: string; written: number }>('/v1/fga/tuples', { tuples }),
  deleteTuples: (tuples: FGATuple[]) =>
    api.delete<{ status: string; deleted: number }>('/v1/fga/tuples', { tuples }),
  listObjects: (user: string, relation: string, type: string) =>
    api.post<{ objects: string[] }>('/v1/fga/list-objects', { user, relation, type }),
  expand: (relation: string, object: string) =>
    api.post<{ tree: any }>('/v1/fga/expand', { relation, object }),
  batchTest: (assertions: { user: string; relation: string; object: string; expected: boolean }[]) =>
    api.post<{ results: FGATestResult[]; total: number; passed: number; failed: number }>('/v1/fga/test', { assertions }),
}
