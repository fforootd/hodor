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
  FgaStoreResponse as FGAStore,
  FgaStoreCheckRequest,
  FgaStoreCheckResponse,
  FgaStoreBatchCheckRequest,
  FgaStoreBatchCheckResponse,
  FgaStoreReadRequest,
  FgaStoreReadResponse,
  FgaStoreWriteRequest,
  FgaStoreWriteResponse,
  FgaStoreListUsersRequest,
  FgaStoreListUsersResponse,
  FgaStoreReadChangesResponse,
  FgaAuthorizationModelWriteRequest,
  FgaAuthorizationModelWriteResponse,
  FgaAuthorizationModelMetadata,
  FgaAuthorizationModelsListResponse,
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
  FgaStoreResponse,
  FgaStoreCheckRequest,
  FgaStoreCheckResponse,
  FgaStoreBatchCheckRequest,
  FgaStoreBatchCheckResponse,
  FgaStoreReadRequest,
  FgaStoreReadResponse,
  FgaStoreWriteRequest,
  FgaStoreWriteResponse,
  FgaStoreListUsersRequest,
  FgaStoreListUsersResponse,
  FgaStoreReadChangesResponse,
  FgaAuthorizationModelWriteRequest,
  FgaAuthorizationModelWriteResponse,
  FgaAuthorizationModelMetadata,
  FgaAuthorizationModelsListResponse,
  FgaExpandRequest,
  FgaListObjectsRequest,
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
  listApps as listAppsFn,
  createApp as createAppFn,
  getApp as getAppFn,
  updateApp as updateAppFn,
  deleteApp as deleteAppFn,
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
  fgaWriteModel as fgaWriteModelFn,
  fgaModelGraph,
  fgaCheck as fgaCheckFn,
  fgaDiscoverStore as fgaDiscoverStoreFn,
  fgaStoreCheck as fgaStoreCheckFn,
  fgaStoreBatchCheck as fgaStoreBatchCheckFn,
  fgaStoreRead as fgaStoreReadFn,
  fgaStoreWrite as fgaStoreWriteFn,
  fgaStoreExpand as fgaStoreExpandFn,
  fgaStoreListObjects as fgaStoreListObjectsFn,
  fgaStoreListUsers as fgaStoreListUsersFn,
  fgaStoreReadChanges as fgaStoreReadChangesFn,
  fgaStoreListAuthorizationModels as fgaStoreListAuthorizationModelsFn,
  fgaStoreWriteAuthorizationModel as fgaStoreWriteAuthorizationModelFn,
  fgaStoreGetAuthorizationModel as fgaStoreGetAuthorizationModelFn,
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
// User API — canonical users family for human_user, service_user, and ai_agent
// ------------------------------------------------------------------
export const userApi = {
  list: (params?: { org_id?: string; schema_type?: string; state?: string; limit?: number }): Promise<IdentityResponse[]> =>
    unwrapItems<IdentityResponse>(listUsers({ query: params as any })),
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

export interface CatalogListResponse {
  templates: CatalogTemplate[]
  total: number
  can_refresh: boolean
}

export const catalogApi = {
  list: (type?: string, tag?: string) => {
    const query: Record<string, string> = {}
    if (type) query.type = type
    if (tag) query.tags = tag
    return api.get<CatalogListResponse>(
      `/v1/catalog${Object.keys(query).length ? '?' + new URLSearchParams(query).toString() : ''}`
    )
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
  list: (params?: { user_id?: string }): Promise<SessionResponse[]> => unwrapItems<SessionResponse>(listSessions({ query: params as any })),
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
  list: (params?: { type?: string; limit?: number; session_id?: string; fingerprint?: string; aggregate_id?: string }): Promise<Event[]> => {
    const query: Record<string, string> = {}
    if (params?.type) query.types = params.type
    if (params?.limit) query.limit = String(params.limit)
    if (params?.session_id) query.session_id = params.session_id
    if (params?.fingerprint) query.fingerprint = params.fingerprint
    if (params?.aggregate_id) query.aggregate_id = params.aggregate_id
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
  name: string
  state?: string
  schema_id?: string
  schema_type?: string
  metadata?: Record<string, unknown>
  data?: Record<string, unknown>
  created_at?: string
  updated_at?: string
}

export const orgApi = {
  list: (): Promise<Org[]> => unwrapItems<Org>(listOrgs()).then(items => items.map(normalizeOrg)),
  get: (id: string): Promise<Org> => api.get<Org>(`/v1/orgs/${encodeURIComponent(id)}`).then(normalizeOrg),
  create: (data: { name?: string; schema_id?: string; metadata?: Record<string, unknown>; data?: Record<string, unknown> }): Promise<Org> =>
    api.post<Org>('/v1/orgs', data).then(normalizeOrg),
  update: (id: string, data: Partial<{ name: string; state: string; metadata: Record<string, unknown>; data: Record<string, unknown> }>): Promise<Org> =>
    api.patch<Org>(`/v1/orgs/${encodeURIComponent(id)}`, data).then(normalizeOrg),
  delete: (id: string): Promise<void> =>
    api.delete(`/v1/orgs/${encodeURIComponent(id)}`),
}

export interface OrgMember {
  user_id: string
  display_name?: string
  role: string
  added_at: string
}

export const orgMembersApi = {
  list: (orgId: string): Promise<OrgMember[]> =>
    api.get<{ items: OrgMember[] }>(`/v1/orgs/${encodeURIComponent(orgId)}/members`).then(r => r.items ?? []),
  add: (orgId: string, userId: string, role = 'member'): Promise<OrgMember> =>
    api.post<OrgMember>(`/v1/orgs/${encodeURIComponent(orgId)}/members`, { user_id: userId, role }),
  remove: (orgId: string, userId: string): Promise<void> =>
    api.delete(`/v1/orgs/${encodeURIComponent(orgId)}/members/${encodeURIComponent(userId)}`),
}

// ------------------------------------------------------------------
// Counts API
// ------------------------------------------------------------------
export const countsApi = {
  get: (): Promise<CountsResponse> => unwrap<CountsResponse>(entityCounts()),
}

// ------------------------------------------------------------------
// App API — canonical applications family
// ------------------------------------------------------------------
export interface App {
  id: string
  org_id: string
  name: string
  description?: string
  app_type: string
  client_id: string
  client_secret?: string
  redirect_uris: string[]
  post_logout_redirect_uris?: string[]
  grant_types: string[]
  response_types: string[]
  logo_uri?: string
  state: string
  schema_id?: string
  schema_type?: string
  metadata?: Record<string, unknown>
  data?: Record<string, unknown>
  created_at: string
  updated_at: string
}

export const appApi = {
  list: (params?: { org_id?: string; schema_type?: string; state?: string; limit?: number }): Promise<App[]> =>
    unwrapItems<App>(listAppsFn({ query: params as any })).then(items => items.map(normalizeApp)),
  get: (id: string): Promise<App> =>
    unwrap<App>(getAppFn({ path: { id } })).then(normalizeApp),
  create: (data: Partial<App> & { name?: string; data?: Record<string, unknown> }): Promise<App> =>
    unwrap<App>(createAppFn({ body: data as any })).then(normalizeApp),
  update: (id: string, data: Partial<App>): Promise<App> =>
    unwrap<App>(updateAppFn({ path: { id }, body: data as any })).then(normalizeApp),
  delete: (id: string): Promise<void> =>
    unwrap<void>(deleteAppFn({ path: { id } })),
}

// Normalize JSON-encoded array fields that come back as strings from the generic list handler.
function normalizeApp(app: any): App {
  if (typeof app.redirect_uris === 'string') {
    try { app.redirect_uris = JSON.parse(app.redirect_uris) } catch { app.redirect_uris = [] }
  }
  if (typeof app.grant_types === 'string') {
    try { app.grant_types = JSON.parse(app.grant_types) } catch { app.grant_types = [] }
  }
  if (typeof app.response_types === 'string') {
    try { app.response_types = JSON.parse(app.response_types) } catch { app.response_types = [] }
  }
  if (typeof app.metadata === 'string') {
    try { app.metadata = JSON.parse(app.metadata) } catch { app.metadata = {} }
  }
  if (typeof app.data === 'string') {
    try { app.data = JSON.parse(app.data) } catch { app.data = {} }
  }
  if (typeof app.post_logout_redirect_uris === 'string') {
    try { app.post_logout_redirect_uris = JSON.parse(app.post_logout_redirect_uris) } catch { app.post_logout_redirect_uris = [] }
  }
  return app as App
}

function normalizeOrg(org: any): Org {
  if (typeof org.metadata === 'string') {
    try { org.metadata = JSON.parse(org.metadata) } catch { org.metadata = {} }
  }
  if (typeof org.data === 'string') {
    try { org.data = JSON.parse(org.data) } catch { org.data = {} }
  }
  return org as Org
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
  discoverStore: (): Promise<FgaStoreResponse> =>
    unwrap<FgaStoreResponse>(fgaDiscoverStoreFn()),
  getModel: (): Promise<FgaModelResponse> => unwrap<FgaModelResponse>(fgaGetModel()),
  writeModel: (body: FgaAuthorizationModelWriteRequest): Promise<FgaAuthorizationModelWriteResponse> =>
    unwrap<FgaAuthorizationModelWriteResponse>(fgaWriteModelFn({ body })),
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
  checkStore: (storeId: string, body: FgaStoreCheckRequest): Promise<FgaStoreCheckResponse> =>
    unwrap<FgaStoreCheckResponse>(fgaStoreCheckFn({ path: { store_id: storeId }, body })),
  batchCheckStore: (storeId: string, body: FgaStoreBatchCheckRequest): Promise<FgaStoreBatchCheckResponse> =>
    unwrap<FgaStoreBatchCheckResponse>(fgaStoreBatchCheckFn({ path: { store_id: storeId }, body })),
  readStore: (storeId: string, body: FgaStoreReadRequest): Promise<FgaStoreReadResponse> =>
    unwrap<FgaStoreReadResponse>(fgaStoreReadFn({ path: { store_id: storeId }, body })),
  writeStore: (storeId: string, body: FgaStoreWriteRequest): Promise<FgaStoreWriteResponse> =>
    unwrap<FgaStoreWriteResponse>(fgaStoreWriteFn({ path: { store_id: storeId }, body })),
  expandStore: (storeId: string, relation: string, object: string, extra?: Partial<FgaExpandRequest>): Promise<FgaExpandResponse> =>
    unwrap<FgaExpandResponse>(fgaStoreExpandFn({ path: { store_id: storeId }, body: { relation, object, ...extra } })),
  listObjectsStore: (storeId: string, user: string, relation: string, type: string, extra?: Partial<FgaListObjectsRequest>): Promise<FgaListObjectsResponse> =>
    unwrap<FgaListObjectsResponse>(fgaStoreListObjectsFn({ path: { store_id: storeId }, body: { user, relation, type, ...extra } })),
  listUsersStore: (storeId: string, body: FgaStoreListUsersRequest): Promise<FgaStoreListUsersResponse> =>
    unwrap<FgaStoreListUsersResponse>(fgaStoreListUsersFn({ path: { store_id: storeId }, body })),
  readChangesStore: (storeId: string, params?: { type?: string; page_size?: number; continuation_token?: string }): Promise<FgaStoreReadChangesResponse> =>
    unwrap<FgaStoreReadChangesResponse>(fgaStoreReadChangesFn({ path: { store_id: storeId }, query: params })),
  listAuthorizationModelsStore: (storeId: string): Promise<FgaAuthorizationModelsListResponse> =>
    unwrap<FgaAuthorizationModelsListResponse>(fgaStoreListAuthorizationModelsFn({ path: { store_id: storeId } })),
  getAuthorizationModelStore: (storeId: string, modelId: string): Promise<FgaAuthorizationModelMetadata> =>
    unwrap<FgaAuthorizationModelMetadata>(fgaStoreGetAuthorizationModelFn({ path: { store_id: storeId, model_id: modelId } })),
  writeAuthorizationModelStore: (storeId: string, body: FgaAuthorizationModelWriteRequest): Promise<FgaAuthorizationModelWriteResponse> =>
    unwrap<FgaAuthorizationModelWriteResponse>(fgaStoreWriteAuthorizationModelFn({ path: { store_id: storeId }, body })),
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
  schema_id?: string
  schema_type?: string
  metadata?: Record<string, unknown>
  data?: Record<string, unknown>
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
    return api.get<{ items: Group[] }>(`/v1/groups${qs}`).then(r => (r.items || []).map(normalizeGroup))
  },
  get: (id: string): Promise<Group> =>
    api.get<Group>(`/v1/groups/${encodeURIComponent(id)}`).then(normalizeGroup),
  create: (data: { name?: string; schema_id?: string; description?: string; metadata?: Record<string, unknown>; data?: Record<string, unknown> }): Promise<Group> =>
    api.post<Group>('/v1/groups', data).then(normalizeGroup),
  update: (id: string, data: Partial<{ name: string; description: string; state: string; metadata: Record<string, unknown>; data: Record<string, unknown> }>): Promise<Group> =>
    api.patch<Group>(`/v1/groups/${encodeURIComponent(id)}`, data).then(normalizeGroup),
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
  schema_id?: string
  schema_type?: string
  metadata?: Record<string, unknown>
  data?: Record<string, unknown>
  member_count: number
  created_at: string
  updated_at: string
}

export const projectApi = {
  list: (orgId?: string): Promise<Project[]> => {
    const qs = orgId ? `?org_id=${encodeURIComponent(orgId)}` : ''
    return api.get<{ items: Project[] }>(`/v1/projects${qs}`).then(r => (r.items || []).map(normalizeProject))
  },
  get: (id: string): Promise<Project> =>
    api.get<Project>(`/v1/projects/${encodeURIComponent(id)}`).then(normalizeProject),
  create: (data: { name?: string; schema_id?: string; description?: string; metadata?: Record<string, unknown>; data?: Record<string, unknown> }): Promise<Project> =>
    api.post<Project>('/v1/projects', data).then(normalizeProject),
  update: (id: string, data: Partial<{ name: string; description: string; state: string; metadata: Record<string, unknown>; data: Record<string, unknown> }>): Promise<Project> =>
    api.patch<Project>(`/v1/projects/${encodeURIComponent(id)}`, data).then(normalizeProject),
  delete: (id: string): Promise<void> =>
    api.delete(`/v1/projects/${encodeURIComponent(id)}`),
  listMembers: (id: string): Promise<Member[]> =>
    api.get<{ items: Member[] }>(`/v1/projects/${encodeURIComponent(id)}/members`).then(r => r.items || []),
  addMember: (id: string, userId: string, role = 'member'): Promise<Member> =>
    api.post<Member>(`/v1/projects/${encodeURIComponent(id)}/members`, { user_id: userId, role }),
  removeMember: (id: string, userId: string): Promise<void> =>
    api.delete(`/v1/projects/${encodeURIComponent(id)}/members/${encodeURIComponent(userId)}`),
}

function normalizeGroup(group: any): Group {
  if (typeof group.metadata === 'string') {
    try { group.metadata = JSON.parse(group.metadata) } catch { group.metadata = {} }
  }
  if (typeof group.data === 'string') {
    try { group.data = JSON.parse(group.data) } catch { group.data = {} }
  }
  return group as Group
}

function normalizeProject(project: any): Project {
  if (typeof project.metadata === 'string') {
    try { project.metadata = JSON.parse(project.metadata) } catch { project.metadata = {} }
  }
  if (typeof project.data === 'string') {
    try { project.data = JSON.parse(project.data) } catch { project.data = {} }
  }
  return project as Project
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
// Notifications API
// ------------------------------------------------------------------
export interface NotificationSettingsEnvelope {
  type: string
  scope: string
  scope_id: string
  data?: Record<string, unknown>
  effective?: Record<string, unknown>
  inherited?: boolean
}

export interface NotificationPreset {
  id: string
  label: string
  medium: string
  driver: string
  description: string
  config: Record<string, unknown>
}

export interface NotificationRender {
  medium: string
  channel_id?: string
  template_key: string
  locale: string
  subject?: string
  text_body: string
  html_body?: string
  metadata?: Record<string, string>
}

export interface NotificationPreviewRequest {
  org_id?: string
  medium: string
  template_key: string
  locale?: string
  payload?: Record<string, unknown>
}

export interface NotificationTestRequest extends NotificationPreviewRequest {
  channel_id?: string
  recipient: string
}

export const notificationApi = {
  getSettings(scope = 'instance', scopeId = ''): Promise<NotificationSettingsEnvelope> {
    const query = new URLSearchParams({ scope, raw: 'true' })
    if (scopeId) query.set('scope_id', scopeId)
    return api.get<NotificationSettingsEnvelope>(`/v1/settings/notification?${query.toString()}`)
  },
  getEffectiveSettings(scope = 'instance', scopeId = ''): Promise<NotificationSettingsEnvelope> {
    const query = new URLSearchParams({ scope })
    if (scopeId) query.set('scope_id', scopeId)
    return api.get<NotificationSettingsEnvelope>(`/v1/settings/notification?${query.toString()}`)
  },
  saveSettings(
    data: Record<string, unknown>,
    scope = 'instance',
    scopeId = '',
  ): Promise<NotificationSettingsEnvelope> {
    const query = new URLSearchParams({ scope })
    if (scopeId) query.set('scope_id', scopeId)
    return api.put<NotificationSettingsEnvelope>(`/v1/settings/notification?${query.toString()}`, data)
  },
  getTemplates(scope = 'instance', scopeId = ''): Promise<NotificationSettingsEnvelope> {
    const query = new URLSearchParams({ scope, raw: 'true' })
    if (scopeId) query.set('scope_id', scopeId)
    return api.get<NotificationSettingsEnvelope>(`/v1/settings/notification_templates?${query.toString()}`)
  },
  saveTemplates(
    data: Record<string, unknown>,
    scope = 'instance',
    scopeId = '',
  ): Promise<NotificationSettingsEnvelope> {
    const query = new URLSearchParams({ scope })
    if (scopeId) query.set('scope_id', scopeId)
    return api.put<NotificationSettingsEnvelope>(`/v1/settings/notification_templates?${query.toString()}`, data)
  },
  listPresets(): Promise<{ presets: NotificationPreset[] }> {
    return api.get<{ presets: NotificationPreset[] }>('/v1/notifications/presets')
  },
  preview(body: NotificationPreviewRequest): Promise<NotificationRender> {
    return api.post<NotificationRender>('/v1/notifications/preview', body)
  },
  sendTest(body: NotificationTestRequest): Promise<NotificationRender> {
    return api.post<NotificationRender>('/v1/notifications/test', body)
  },
}
