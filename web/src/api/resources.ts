/**
 * API resources — now backed by the generated @zitadel/client-js SDK.
 *
 * This file provides backward-compatible exports so existing views
 * continue to work without changes. New code should import directly
 * from '@zitadel/client-js' or from './sdk'.
 */

// Initialize the SDK client (must run before any SDK calls).
import './sdk'

export type {
  IdentityResponse as Identity,
  SchemaResponse as Schema,
  SessionResponse as Session,
} from '@zitadel/client-js'

// Types the SDK generates with the same name.
export type {
  CatalogTemplateDetailResponse as CatalogTemplateDetail,
  CatalogInstallResponse,
  CatalogRefreshResponse,
  CountsResponse,
  FgaCheckResponse as FGACheckResult,
  FgaBatchTestResponse as FGABatchTestResult,
  FgaModelGraphResponse,
  FgaModelResponse,
  FgaReadTuplesResponse,
  FgaWriteTuplesResponse,
  FgaDeleteTuplesResponse,
  FgaListObjectsResponse,
  FgaExpandResponse,
  SearchResponse,
  PromoteSchemaResponse,
  DiffSchemaResponse,
  PreviewSchemaResponse,
  SchemaIdentityCountResponse,
  MagicLinkResponse,
  ProviderResponse,
  ImportResult,
  ListResponse,
} from '@zitadel/client-js'

// Internal type imports for explicit return annotations.
import type {
  IdentityResponse,
  SchemaResponse,
  SessionResponse,
  CatalogTemplateDetailResponse,
  CatalogInstallResponse,
  CatalogRefreshResponse,
  CountsResponse,
  FgaCheckResponse,
  FgaBatchTestResponse,
  FgaModelGraphResponse,
  FgaModelResponse,
  FgaListObjectsResponse,
  FgaExpandResponse,
  SearchResponse,
  PromoteSchemaResponse,
  DiffSchemaResponse,
  PreviewSchemaResponse,
  MagicLinkResponse,
} from '@zitadel/client-js'

// Import SDK service functions.
import {
  listUsers,
  createUser,
  getUser,
  updateUser,
  deleteUser,
  listSchemas,
  getSchema,
  updateSchema,
  promoteSchema as promoteSchemaFn,
  diffSchema as diffSchemaFn,
  previewSchema as previewSchemaFn,
  schemaIdentityCount,
  setUserPassword as setUserPasswordFn,
  listSessions,
  revokeSession,
  listEvents,
  entityCounts,
  search,
  sendMagicLink as sendMagicLinkFn,
  listCatalog as listCatalogFn,
  getCatalogEntry,
  installFromCatalog as installFromCatalogFn,
  refreshCatalog as refreshCatalogFn,
  listProviders,
  createProvider as createProviderFn,
  updateProvider as updateProviderFn,
  deleteProvider as deleteProviderFn,
  listProviderTemplates as listProviderTemplatesFn,
  fgaGetModel,
  fgaModelGraph,
  fgaCheck as fgaCheckFn,
  fgaReadTuples,
  fgaWriteTuples as fgaWriteTuplesFn,
  fgaDeleteTuples as fgaDeleteTuplesFn,
  fgaListObjects as fgaListObjectsFn,
  fgaExpand as fgaExpandFn,
  fgaBatchTest as fgaBatchTestFn,
  listOrgs,
  getMetaSchema as getMetaSchemaFn,
} from '@zitadel/client-js'

// Legacy api import — still needed for ad-hoc calls (analytics, passwords, etc.)
import { api } from './client'

// ------------------------------------------------------------------
// Helper to unwrap hey-api responses: { data, error } → data
// ------------------------------------------------------------------
// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function unwrap<T>(promise: Promise<any>): Promise<T> {
  const res = await promise
  if (res.error !== undefined) throw res.error
  return res.data as T
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function unwrapItems<T>(promise: Promise<any>): Promise<T[]> {
  const res = await promise
  if (res.error !== undefined) throw res.error
  return (res.data?.items as T[]) || []
}

// ------------------------------------------------------------------
// User API — unified CRUD for all user types (human_user, service_user, ai_agent)
// ------------------------------------------------------------------
export const userApi = {
  list: (): Promise<IdentityResponse[]> => unwrapItems<IdentityResponse>(listUsers()),
  get: (id: string): Promise<IdentityResponse> => unwrap<IdentityResponse>(getUser({ path: { id } })),
  create: (data: Record<string, unknown>): Promise<IdentityResponse> => unwrap<IdentityResponse>(createUser({ body: data as any })),
  update: (id: string, data: Record<string, unknown>): Promise<IdentityResponse> => unwrap<IdentityResponse>(updateUser({ path: { id }, body: data as any })),
  delete: (id: string): Promise<void> => unwrap<void>(deleteUser({ path: { id } })),
  setPassword: (id: string, password: string): Promise<void> => unwrap<void>(setUserPasswordFn({ path: { id }, body: { password } })),
}

// ------------------------------------------------------------------
// Schema API
// ------------------------------------------------------------------
export const schemaApi = {
  list: (): Promise<SchemaResponse[]> => unwrapItems<SchemaResponse>(listSchemas()),
  listByType: (type: string): Promise<SchemaResponse[]> => unwrapItems<SchemaResponse>(listSchemas({ query: { type } })),
  get: (id: string): Promise<SchemaResponse> => unwrap<SchemaResponse>(getSchema({ path: { id } })),
  update: (id: string, schema: Record<string, unknown>, message?: string) =>
    unwrap<SchemaResponse>(updateSchema({ path: { id }, body: { schema, message: message || '' } })),
  promote: (id: string): Promise<PromoteSchemaResponse> =>
    unwrap<PromoteSchemaResponse>(promoteSchemaFn({ path: { id } })),
  diff: (id: string, compareId: string): Promise<DiffSchemaResponse> =>
    unwrap<DiffSchemaResponse>(diffSchemaFn({ path: { id }, query: { compare: compareId } })),
  preview: (id: string, entityId: string): Promise<PreviewSchemaResponse> =>
    unwrap<PreviewSchemaResponse>(previewSchemaFn({ path: { id }, body: { entity_id: entityId } })),
  entityCount: (id: string) =>
    unwrap<any>(schemaIdentityCount({ path: { id } })).then((r: any) => r.count),
  // previewUpgrade stays on raw api — not in the generated SDK yet
  previewUpgrade: (type: string, newSchema: Record<string, any>, sampleSize = 10) =>
    api.post<UpgradeReport>(`/v1/schemas/${encodeURIComponent(type)}/preview-upgrade`, {
      new_schema: newSchema, sample_size: sampleSize,
    }),
}

// ------------------------------------------------------------------
// Catalog API
// ------------------------------------------------------------------
export interface CatalogTemplate {
  id: string
  name: string
  type: string
  version: string
  description: string
  tags: string[]
  source: string
}

export interface CatalogVariable {
  type: string
  description?: string
  default?: any
}

export const catalogApi = {
  list: (type?: string, tag?: string) => {
    const query: Record<string, string> = {}
    if (type) query.type = type
    if (tag) query.tags = tag
    return api.get<{ templates: CatalogTemplate[]; total: number }>(
      `/v1/catalog${Object.keys(query).length ? '?' + new URLSearchParams(query).toString() : ''}`
    ).then(r => r.templates || [])
  },
  get: (id: string): Promise<CatalogTemplateDetailResponse> => unwrap<CatalogTemplateDetailResponse>(getCatalogEntry({ path: { id } })),
  install: (id: string, variables: Record<string, any>): Promise<CatalogInstallResponse> =>
    unwrap<CatalogInstallResponse>(installFromCatalogFn({ path: { id }, body: { variables } })),
  refresh: (): Promise<CatalogRefreshResponse> => unwrap<CatalogRefreshResponse>(refreshCatalogFn()),
}

// ------------------------------------------------------------------
// Session API
// ------------------------------------------------------------------
export const sessionApi = {
  list: (): Promise<SessionResponse[]> => unwrapItems<SessionResponse>(listSessions()),
  revoke: (id: string): Promise<void> => unwrap<void>(revokeSession({ path: { id } })),
}

// ------------------------------------------------------------------
// Event API
// ------------------------------------------------------------------
export interface Event {
  id: string
  event_type: string
  actor_id: string
  actor_type?: string
  aggregate_id: string
  aggregate_type: string
  request_id?: string
  session_id?: string
  flow_id?: string
  fingerprint?: string
  client_id?: string
  token_id?: string
  delegation_type?: string
  sdk_name?: string
  sdk_version?: string
  payload: Record<string, unknown>
  metadata?: Record<string, unknown>
  created_at: string
}

export const eventApi = {
  list: (params?: { type?: string; limit?: number; session_id?: string; fingerprint?: string }): Promise<Event[]> => {
    const query: Record<string, string> = {}
    if (params?.type) query.types = params.type
    if (params?.limit) query.limit = String(params.limit)
    if (params?.session_id) query.session_id = params.session_id
    if (params?.fingerprint) query.fingerprint = params.fingerprint
    return unwrapItems<Event>(listEvents({ query: query as any }))
  },
}

// ------------------------------------------------------------------
// Search API
// ------------------------------------------------------------------
export interface SearchResult {
  resource_type: string
  id: string
  title: string
  subtitle: string
}

export const searchApi = {
  search: (q: string, limit = 10): Promise<SearchResponse> => unwrap<SearchResponse>(search({ query: { q, limit } })),
}

// ------------------------------------------------------------------
// Magic Link API
// ------------------------------------------------------------------
export const magicLinkApi = {
  send: (email: string): Promise<MagicLinkResponse> => unwrap<MagicLinkResponse>(sendMagicLinkFn({ body: { email } })),
}

// ------------------------------------------------------------------
// Meta Schema API
// ------------------------------------------------------------------
export const metaSchemaApi = {
  get: (): Promise<any> => unwrap<any>(getMetaSchemaFn()),
}

// ------------------------------------------------------------------
// Organization API
// ------------------------------------------------------------------
export interface Org {
  id: string
  instance_id?: string
  name: string
  state?: string
  metadata?: Record<string, unknown>
  created_at?: string
  updated_at?: string
}

export const orgApi = {
  list: (): Promise<Org[]> => unwrapItems<Org>(listOrgs()),
  get: (id: string): Promise<Org> => api.get<Org>(`/v1/orgs/${encodeURIComponent(id)}`),
  create: (data: { name: string; metadata?: Record<string, unknown> }): Promise<Org> =>
    api.post<Org>('/v1/orgs', data),
  update: (id: string, data: Partial<{ name: string; state: string; metadata: Record<string, unknown> }>): Promise<Org> =>
    api.patch<Org>(`/v1/orgs/${encodeURIComponent(id)}`, data),
  delete: (id: string): Promise<void> =>
    api.delete(`/v1/orgs/${encodeURIComponent(id)}`),
}

// ------------------------------------------------------------------
// Counts API
// ------------------------------------------------------------------
export const countsApi = {
  get: (): Promise<CountsResponse> => unwrap<CountsResponse>(entityCounts()),
}

// ------------------------------------------------------------------
// Provider API
// ------------------------------------------------------------------
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
  list: (): Promise<Provider[]> => unwrapItems<Provider>(listProviders()),
  templates: () => api.get<{ templates: ProviderTemplate[] }>('/v1/providers/templates').then(r => r.templates || []),
  create: (data: Record<string, unknown>) => unwrap<any>(createProviderFn({ body: data as any })),
  update: (id: string, data: Record<string, unknown>) => unwrap<any>(updateProviderFn({ path: { id }, body: data as any })),
  delete: (id: string): Promise<void> => unwrap<void>(deleteProviderFn({ path: { id } })),
}

// ------------------------------------------------------------------
// Analytics API (no SDK — custom SQL queries)
// ------------------------------------------------------------------
export const analyticsApi = {
  query: (body: Record<string, unknown>) =>
    api.post<Record<string, any>>('/v1/analytics/query', body),
  schema: () =>
    api.get<Record<string, any>>('/v1/analytics/schema'),
  tables: () =>
    api.get<Record<string, any>>('/v1/analytics/tables'),
}

// ------------------------------------------------------------------
// FGA API
// ------------------------------------------------------------------
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
  getModel: (): Promise<FgaModelResponse> => unwrap<FgaModelResponse>(fgaGetModel()),
  getModelGraph: (): Promise<FgaModelGraphResponse> => unwrap<FgaModelGraphResponse>(fgaModelGraph()),
  check: (user: string, relation: string, object: string): Promise<FgaCheckResponse> =>
    unwrap<FgaCheckResponse>(fgaCheckFn({ body: { user, relation, object } })),
  readTuples: (params?: { user?: string; relation?: string; object?: string }) => {
    const query: Record<string, string> = {}
    if (params?.user) query.user = params.user
    if (params?.relation) query.relation = params.relation
    if (params?.object) query.object = params.object
    return unwrap<any>(fgaReadTuples({ query: query as any })).then((r: any) => r.tuples || [])
  },
  writeTuples: (tuples: FGATuple[]) =>
    unwrap<any>(fgaWriteTuplesFn({ body: { tuples } as any })),
  deleteTuples: (tuples: FGATuple[]) =>
    unwrap<any>(fgaDeleteTuplesFn({ body: { tuples } as any })),
  listObjects: (user: string, relation: string, type: string): Promise<FgaListObjectsResponse> =>
    unwrap<FgaListObjectsResponse>(fgaListObjectsFn({ body: { user, relation, type } })),
  expand: (relation: string, object: string): Promise<FgaExpandResponse> =>
    unwrap<FgaExpandResponse>(fgaExpandFn({ body: { relation, object } })),
  batchTest: (assertions: { user: string; relation: string; object: string; expected: boolean }[]): Promise<FgaBatchTestResponse> =>
    unwrap<FgaBatchTestResponse>(fgaBatchTestFn({ body: { assertions } as any })),
}

// ------------------------------------------------------------------
// Upgrade report types (used by SchemaUpgradePreview)
// ------------------------------------------------------------------
export interface UpgradeFieldChange {
  path: string
  change: string
  description: string
  severity: string
  affected_estimate?: number
}

export interface UpgradeEntityResult {
  id: string
  display_name: string
  status: string
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

// ------------------------------------------------------------------
// Group API (ADR-020: sealed primitive)
// ------------------------------------------------------------------
export interface Group {
  id: string
  org_id: string
  name: string
  description: string
  state: string
  metadata?: Record<string, unknown>
  member_count: number
  created_at: string
  updated_at: string
}

export interface Member {
  user_id: string
  display_name?: string
  role: string
  added_at: string
}

export const groupApi = {
  list: (orgId?: string): Promise<Group[]> => {
    const qs = orgId ? `?org_id=${encodeURIComponent(orgId)}` : ''
    return api.get<{ items: Group[] }>(`/v1/groups${qs}`).then(r => r.items || [])
  },
  get: (id: string): Promise<Group> =>
    api.get<Group>(`/v1/groups/${encodeURIComponent(id)}`),
  create: (data: { name: string; description?: string; metadata?: Record<string, unknown> }): Promise<Group> =>
    api.post<Group>('/v1/groups', data),
  update: (id: string, data: Partial<{ name: string; description: string; state: string; metadata: Record<string, unknown> }>): Promise<Group> =>
    api.patch<Group>(`/v1/groups/${encodeURIComponent(id)}`, data),
  delete: (id: string): Promise<void> =>
    api.delete(`/v1/groups/${encodeURIComponent(id)}`),
  listMembers: (id: string): Promise<Member[]> =>
    api.get<{ items: Member[] }>(`/v1/groups/${encodeURIComponent(id)}/members`).then(r => r.items || []),
  addMember: (id: string, userId: string, role = 'member'): Promise<Member> =>
    api.post<Member>(`/v1/groups/${encodeURIComponent(id)}/members`, { user_id: userId, role }),
  removeMember: (id: string, userId: string): Promise<void> =>
    api.delete(`/v1/groups/${encodeURIComponent(id)}/members/${encodeURIComponent(userId)}`),
}

// ------------------------------------------------------------------
// Project API (ADR-020: sealed primitive)
// ------------------------------------------------------------------
export interface Project {
  id: string
  org_id: string
  name: string
  description: string
  state: string
  metadata?: Record<string, unknown>
  member_count: number
  created_at: string
  updated_at: string
}

export const projectApi = {
  list: (orgId?: string): Promise<Project[]> => {
    const qs = orgId ? `?org_id=${encodeURIComponent(orgId)}` : ''
    return api.get<{ items: Project[] }>(`/v1/projects${qs}`).then(r => r.items || [])
  },
  get: (id: string): Promise<Project> =>
    api.get<Project>(`/v1/projects/${encodeURIComponent(id)}`),
  create: (data: { name: string; description?: string; metadata?: Record<string, unknown> }): Promise<Project> =>
    api.post<Project>('/v1/projects', data),
  update: (id: string, data: Partial<{ name: string; description: string; state: string; metadata: Record<string, unknown> }>): Promise<Project> =>
    api.patch<Project>(`/v1/projects/${encodeURIComponent(id)}`, data),
  delete: (id: string): Promise<void> =>
    api.delete(`/v1/projects/${encodeURIComponent(id)}`),
  listMembers: (id: string): Promise<Member[]> =>
    api.get<{ items: Member[] }>(`/v1/projects/${encodeURIComponent(id)}/members`).then(r => r.items || []),
  addMember: (id: string, userId: string, role = 'member'): Promise<Member> =>
    api.post<Member>(`/v1/projects/${encodeURIComponent(id)}/members`, { user_id: userId, role }),
  removeMember: (id: string, userId: string): Promise<void> =>
    api.delete(`/v1/projects/${encodeURIComponent(id)}/members/${encodeURIComponent(userId)}`),
}

// ------------------------------------------------------------------
// Module API (ADR-020: marketplace modules)
// ------------------------------------------------------------------
export interface Module {
  name: string
  description: string
  enabled: boolean
}

export const moduleApi = {
  list: (): Promise<Module[]> =>
    api.get<{ items: Module[] }>('/v1/modules').then(r => r.items || []),
  enable: (name: string): Promise<void> =>
    api.post<void>(`/v1/modules/${encodeURIComponent(name)}/enable`, {}),
  disable: (name: string): Promise<void> =>
    api.post<void>(`/v1/modules/${encodeURIComponent(name)}/disable`, {}),
}

// ------------------------------------------------------------------
// Instance API (ADR-021: multi-tenancy)
// ------------------------------------------------------------------
import { ref, computed } from 'vue'

export interface Instance {
  id: string
  name: string
  domain: string
  is_root: boolean
  state: string
  created_at: string
  updated_at: string
}

export interface InstanceDetail {
  instance: Instance
  user_count: number
  org_count: number
}

/** Reactive state — the currently selected instance (null = root). */
export const currentInstance = ref<string | null>(null)

/** Returns the API path prefix for the current instance context. */
export const instanceApiPrefix = computed(() => {
  if (currentInstance.value) {
    return `/v1/instances/${currentInstance.value}`
  }
  return '/v1'
})

export function switchInstance(instanceId: string | null) {
  currentInstance.value = instanceId
  if (typeof window !== 'undefined' && window.localStorage) {
    if (instanceId) {
      window.localStorage.setItem('zitadel_instance', instanceId)
    } else {
      window.localStorage.removeItem('zitadel_instance')
    }
  }
}

// Restore instance from localStorage on load.
try {
  if (typeof window !== 'undefined' && window.localStorage) {
    const savedInstance = window.localStorage.getItem('zitadel_instance')
    if (savedInstance) {
      currentInstance.value = savedInstance
    }
  }
} catch (e) {
  // Ignore localStorage errors in non-browser environments.
}

export const instanceApi = {
  list: (): Promise<Instance[]> =>
    api.get<{ items: Instance[] }>('/v1/instances').then(r => r.items || []),
  get: (id: string): Promise<InstanceDetail> =>
    api.get<InstanceDetail>(`/v1/instances/${encodeURIComponent(id)}`),
  create: (data: { name: string; domain?: string }): Promise<Instance> =>
    api.post<Instance>('/v1/instances', data),
  update: (id: string, data: Partial<{ name: string; domain: string; state: string }>): Promise<void> =>
    api.patch<void>(`/v1/instances/${encodeURIComponent(id)}`, data),
  delete: (id: string): Promise<void> =>
    api.delete(`/v1/instances/${encodeURIComponent(id)}`),
}
