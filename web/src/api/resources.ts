/**
 * API resources — now backed by the generated @zitadel/client-js SDK.
 *
 * This file provides backward-compatible exports so existing views
 * continue to work without changes. New code should import directly
 * from '@zitadel/client-js' or from './sdk'.
 */

// Initialize the SDK client (must run before any SDK calls).
import './sdk'

// Re-export generated types under their legacy names.
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
// User API (type-specific, replaces generic entityApi)
// ------------------------------------------------------------------
export const userApi = {
  list: () => unwrapItems(listUsers()),
  get: (id: string) => unwrap(getUser({ path: { id } })),
  create: (data: Record<string, unknown>) => unwrap(createUser({ body: data as any })),
  update: (id: string, data: Record<string, unknown>) => unwrap(updateUser({ path: { id }, body: data as any })),
  delete: (id: string) => unwrap(deleteUser({ path: { id } })),
  setPassword: (id: string, password: string) => unwrap(setUserPasswordFn({ path: { id }, body: { password } })),
}

// Backward-compatible alias — views still import entityApi.
export const entityApi = userApi

// ------------------------------------------------------------------
// Schema API
// ------------------------------------------------------------------
export const schemaApi = {
  list: () => unwrapItems(listSchemas()),
  listByType: (type: string) => unwrapItems(listSchemas({ query: { type } })),
  get: (id: string) => unwrap(getSchema({ path: { id } })),
  update: (id: string, schema: Record<string, unknown>, message?: string) =>
    unwrap(updateSchema({ path: { id }, body: { schema, message: message || '' } })),
  promote: (id: string) =>
    unwrap(promoteSchemaFn({ path: { id } })),
  diff: (id: string, compareId: string) =>
    unwrap(diffSchemaFn({ path: { id }, query: { compare: compareId } })),
  preview: (id: string, entityId: string) =>
    unwrap(previewSchemaFn({ path: { id }, body: { entity_id: entityId } })),
  entityCount: (id: string) =>
    unwrap(schemaIdentityCount({ path: { id } })).then((r: any) => r.count),
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
  get: (id: string) => unwrap(getCatalogEntry({ path: { id } })),
  install: (id: string, variables: Record<string, any>) =>
    unwrap(installFromCatalogFn({ path: { id }, body: { variables } })),
  refresh: () => unwrap(refreshCatalogFn()),
}

// ------------------------------------------------------------------
// Session API
// ------------------------------------------------------------------
export const sessionApi = {
  list: () => unwrapItems(listSessions()),
  revoke: (id: string) => unwrap(revokeSession({ path: { id } })),
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
  session_id?: string
  trace_id?: string
  span_id?: string
  parent_span_id?: string
  payload: Record<string, unknown>
  metadata?: Record<string, unknown>
  created_at: string
}

export const eventApi = {
  list: (params?: { type?: string; limit?: number; session_id?: string }) => {
    const query: Record<string, string> = {}
    if (params?.type) query.types = params.type
    if (params?.limit) query.limit = String(params.limit)
    if (params?.session_id) query.session_id = params.session_id
    return unwrapItems(listEvents({ query: query as any }))
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
  link: string
}

export const searchApi = {
  search: (q: string, limit = 10) => unwrap(search({ query: { q, limit } })),
}

// ------------------------------------------------------------------
// Magic Link API
// ------------------------------------------------------------------
export const magicLinkApi = {
  send: (email: string) => unwrap(sendMagicLinkFn({ body: { email } })),
}

// ------------------------------------------------------------------
// Meta Schema API
// ------------------------------------------------------------------
export const metaSchemaApi = {
  get: () => unwrap(getMetaSchemaFn()),
}

// ------------------------------------------------------------------
// Organization API
// ------------------------------------------------------------------
export interface Org {
  id: number
  identifier: string
  display_name: string
}

export const orgApi = {
  list: () => unwrapItems(listOrgs()),
}

// ------------------------------------------------------------------
// Counts API
// ------------------------------------------------------------------
export const countsApi = {
  get: () => unwrap(entityCounts()),
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
  list: () => unwrapItems(listProviders()),
  templates: () => api.get<{ templates: ProviderTemplate[] }>('/v1/providers/templates').then(r => r.templates || []),
  create: (data: Record<string, unknown>) => unwrap(createProviderFn({ body: data as any })),
  update: (id: string, data: Record<string, unknown>) => unwrap(updateProviderFn({ path: { id }, body: data as any })),
  delete: (id: string) => unwrap(deleteProviderFn({ path: { id } })),
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
  getModel: () => unwrap(fgaGetModel()),
  getModelGraph: () => unwrap(fgaModelGraph()),
  check: (user: string, relation: string, object: string) =>
    unwrap(fgaCheckFn({ body: { user, relation, object } })),
  readTuples: (params?: { user?: string; relation?: string; object?: string }) => {
    const query: Record<string, string> = {}
    if (params?.user) query.user = params.user
    if (params?.relation) query.relation = params.relation
    if (params?.object) query.object = params.object
    return unwrap(fgaReadTuples({ query: query as any })).then((r: any) => r.tuples || [])
  },
  writeTuples: (tuples: FGATuple[]) =>
    unwrap(fgaWriteTuplesFn({ body: { tuples } as any })),
  deleteTuples: (tuples: FGATuple[]) =>
    unwrap(fgaDeleteTuplesFn({ body: { tuples } as any })),
  listObjects: (user: string, relation: string, type: string) =>
    unwrap(fgaListObjectsFn({ body: { user, relation, type } })),
  expand: (relation: string, object: string) =>
    unwrap(fgaExpandFn({ body: { relation, object } })),
  batchTest: (assertions: { user: string; relation: string; object: string; expected: boolean }[]) =>
    unwrap(fgaBatchTestFn({ body: { assertions } as any })),
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
