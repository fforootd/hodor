/**
 * Local stub for @zitadel/client-js
 *
 * This package doesn't exist on npm yet — it's a forward reference to a
 * planned generated SDK. This shim provides all the exported symbols that
 * resources.ts and sdk.ts expect, backed by the existing api.get/post client.
 *
 * Once the real SDK is published, delete this file and remove the Vite alias.
 */
import { api } from '@/api/client'

// ─── Types ──────────────────────────────────────────────────
// These are passthrough type stubs — the shapes match what the rest views expect.

export interface IdentityResponse { id: string; [k: string]: any }
export interface SchemaResponse { id: string; [k: string]: any }
export interface SessionResponse { id: string; [k: string]: any }
export interface CatalogTemplateDetailResponse { id: string; [k: string]: any }
export interface CatalogInstallResponse { [k: string]: any }
export interface CatalogRefreshResponse { [k: string]: any }
export interface CountsResponse { [k: string]: any }
export interface FgaCheckResponse { allowed: boolean; [k: string]: any }
export interface FgaBatchTestResponse { results: any[] }
export interface FgaModelGraphResponse { nodes: any[]; edges: any[] }
export interface FgaModelResponse { model: string; [k: string]: any }
export interface FgaRelationshipCondition { name: string; context?: Record<string, unknown> }
export interface FgaTupleKey { user: string; relation: string; object: string; condition?: FgaRelationshipCondition | null }
export interface FgaContextualTuples { tuple_keys?: FgaTupleKey[] }
export interface FgaStoreResponse { store_id: string; name: string; instance_id: string }
export interface FgaStoreCheckRequest { tuple_key: FgaTupleKey; authorization_model_id?: string; contextual_tuples?: FgaContextualTuples; context?: unknown }
export interface FgaStoreCheckResponse { allowed: boolean }
export interface FgaBatchCheckItem { tuple_key: FgaTupleKey; correlation_id?: string }
export interface FgaStoreBatchCheckRequest { checks?: FgaBatchCheckItem[]; authorization_model_id?: string; contextual_tuples?: FgaContextualTuples; context?: unknown }
export interface FgaStoreBatchCheckResponse { results?: Array<{ allowed: boolean; correlation_id?: string }> }
export interface FgaTupleFilter { user?: string; relation?: string; object?: string }
export interface FgaStoreReadRequest { tuple_key?: FgaTupleFilter; page_size?: number; continuation_token?: string }
export interface FgaStoreReadResponse { tuples?: Array<{ key: FgaTupleKey; timestamp?: string }>; continuation_token?: string }
export interface FgaTupleKeySet { tuple_keys?: FgaTupleKey[] }
export interface FgaStoreWriteRequest { writes?: FgaTupleKeySet; deletes?: FgaTupleKeySet; authorization_model_id?: string }
export interface FgaStoreWriteResponse { [k: string]: any }
export interface FgaUserFilter { type: string; relation?: string }
export interface FgaStoreListUsersRequest { object: string; relation: string; user_filters?: FgaUserFilter[]; authorization_model_id?: string; contextual_tuples?: FgaContextualTuples }
export interface FgaStoreListUsersResponse { users?: string[] }
export interface FgaStoreReadChangesResponse { changes?: Array<{ tuple_key: FgaTupleKey; operation: string; timestamp: string }>; continuation_token?: string }
export interface FgaAuthorizationModelWriteRequest { schema_version: string; type_definitions?: Array<Record<string, unknown>>; conditions?: Record<string, unknown> }
export interface FgaAuthorizationModelWriteResponse { authorization_model_id: string }
export interface FgaAuthorizationModelMetadata { authorization_model_id: string; schema_version: string; type_definitions: Array<Record<string, unknown>>; conditions?: Record<string, unknown>; created_at: string }
export interface FgaAuthorizationModelsListResponse { authorization_models?: FgaAuthorizationModelMetadata[] }
export interface FgaReadTuplesResponse { tuples: any[] }
export interface FgaWriteTuplesResponse { [k: string]: any }
export interface FgaDeleteTuplesResponse { [k: string]: any }
export interface FgaListObjectsResponse { objects: string[] }
export interface FgaExpandRequest { object: string; relation: string; authorization_model_id?: string; contextual_tuples?: FgaContextualTuples }
export interface FgaExpandResponse { tree: any }
export interface FgaListObjectsRequest { user: string; relation: string; type: string; authorization_model_id?: string; contextual_tuples?: FgaContextualTuples }
export interface SearchResponse { results: any[]; total: number }
export interface PromoteSchemaResponse { [k: string]: any }
export interface DiffSchemaResponse { [k: string]: any }
export interface PreviewSchemaResponse { [k: string]: any }
export interface SchemaIdentityCountResponse { count: number }
export interface MagicLinkResponse { status: string; [k: string]: any }
export interface ProviderResponse { id: string; [k: string]: any }
export interface ImportResult { [k: string]: any }
export interface ListResponse { items: any[]; total: number }

// ─── Client ─────────────────────────────────────────────────
// Minimal hey-api compatible client stub.

export const client = {
  setConfig(_cfg: { baseUrl?: string; fetch?: typeof fetch }) {
    // no-op — the real config lives in api/client.ts
  },
}

// ─── SDK-style wrapper ── returns { data, error } ───────────
// Each function returns a Promise<{ data, error }> matching the hey-api pattern.

function wrap<T>(promise: Promise<T>) {
  return promise.then(data => ({ data, error: undefined })).catch(error => ({ data: undefined, error }))
}

type Opts = { path?: Record<string, string>; query?: Record<string, any>; body?: any }

// Users
export const listUsers = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/users${qs(opts?.query)}`))
export const createUser = (opts?: Opts) => wrap(api.post<any>('/v1/users', opts?.body))
export const getUser = (opts?: Opts) => wrap(api.get<any>(`/v1/users/${opts?.path?.id}`))
export const updateUser = (opts?: Opts) => wrap(api.patch<any>(`/v1/users/${opts?.path?.id}`, opts?.body))
export const deleteUser = (opts?: Opts) => wrap(api.delete<any>(`/v1/users/${opts?.path?.id}`))
export const setUserPassword = (opts?: Opts) => wrap(api.post<any>(`/v1/users/${opts?.path?.id}/password`, opts?.body))

// Applications
export const listApps = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/apps${qs(opts?.query)}`))
export const createApp = (opts?: Opts) => wrap(api.post<any>('/v1/apps', opts?.body))
export const getApp = (opts?: Opts) => wrap(api.get<any>(`/v1/apps/${opts?.path?.id}`))
export const updateApp = (opts?: Opts) => wrap(api.patch<any>(`/v1/apps/${opts?.path?.id}`, opts?.body))
export const deleteApp = (opts?: Opts) => wrap(api.delete<any>(`/v1/apps/${opts?.path?.id}`))

// Schemas
export const listSchemas = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/schemas${qs(opts?.query)}`))
export const getSchema = (opts?: Opts) => wrap(api.get<any>(`/v1/schemas/${opts?.path?.id}`))
export const updateSchema = (opts?: Opts) => wrap(api.patch<any>(`/v1/schemas/${opts?.path?.id}`, opts?.body))
export const promoteSchema = (opts?: Opts) => wrap(api.post<any>(`/v1/schemas/${opts?.path?.id}/promote`, {}))
export const diffSchema = (opts?: Opts) => wrap(api.get<any>(`/v1/schemas/${opts?.path?.id}/diff${qs(opts?.query)}`))
export const previewSchema = (opts?: Opts) => wrap(api.post<any>(`/v1/schemas/${opts?.path?.id}/preview`, opts?.body))
export const schemaIdentityCount = (opts?: Opts) => wrap(api.get<any>(`/v1/schemas/${opts?.path?.id}/identity-count`))

// Sessions
export const listSessions = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/sessions${qs(opts?.query)}`))
export const revokeSession = (opts?: Opts) => wrap(api.post<any>(`/v1/sessions/${opts?.path?.id}/revoke`, {}))

// Events
export const listEvents = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/events${qs(opts?.query)}`))

// Counts
export const entityCounts = () => wrap(api.get<any>('/v1/counts'))

// Search
export const search = (opts?: Opts) => wrap(api.get<any>(`/v1/search${qs(opts?.query)}`))

// Magic Link
export const sendMagicLink = (opts?: Opts) => wrap(api.post<any>('/v1/auth/magic-link', opts?.body))

// Catalog
export const listCatalog = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/catalog${qs(opts?.query)}`))
export const getCatalogEntry = (opts?: Opts) => wrap(api.get<any>(`/v1/catalog/${opts?.path?.id}`))
export const installFromCatalog = (opts?: Opts) => wrap(api.post<any>(`/v1/catalog/${opts?.path?.id}/install`, opts?.body))
export const refreshCatalog = () => wrap(api.post<any>('/v1/catalog/refresh', {}))

// Providers
export const listProviders = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/providers${qs(opts?.query)}`))
export const createProvider = (opts?: Opts) => wrap(api.post<any>('/v1/providers', opts?.body))
export const updateProvider = (opts?: Opts) => wrap(api.patch<any>(`/v1/providers/${opts?.path?.id}`, opts?.body))
export const deleteProvider = (opts?: Opts) => wrap(api.delete<any>(`/v1/providers/${opts?.path?.id}`))
export const listProviderTemplates = () => wrap(api.get<{ templates: any[] }>('/v1/providers/templates'))

// FGA
export const fgaGetModel = () => wrap(api.get<any>('/v1/fga/model'))
export const fgaWriteModel = (opts?: Opts) => wrap(api.post<any>('/v1/fga/model', opts?.body))
export const fgaModelGraph = () => wrap(api.get<any>('/v1/fga/model/graph'))
export const fgaCheck = (opts?: Opts) => wrap(api.post<any>('/v1/fga/check', opts?.body))
export const fgaDiscoverStore = () => wrap(api.get<any>('/v1/fga/store'))
export const fgaStoreCheck = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/check`, opts?.body))
export const fgaStoreBatchCheck = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/batch-check`, opts?.body))
export const fgaStoreRead = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/read`, opts?.body))
export const fgaStoreWrite = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/write`, opts?.body))
export const fgaStoreExpand = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/expand`, opts?.body))
export const fgaStoreListObjects = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/list-objects`, opts?.body))
export const fgaStoreListUsers = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/list-users`, opts?.body))
export const fgaStoreReadChanges = (opts?: Opts) => wrap(api.get<any>(`/v1/fga/stores/${opts?.path?.store_id}/changes${qs(opts?.query)}`))
export const fgaStoreListAuthorizationModels = (opts?: Opts) => wrap(api.get<any>(`/v1/fga/stores/${opts?.path?.store_id}/authorization-models`))
export const fgaStoreWriteAuthorizationModel = (opts?: Opts) => wrap(api.post<any>(`/v1/fga/stores/${opts?.path?.store_id}/authorization-models`, opts?.body))
export const fgaStoreGetAuthorizationModel = (opts?: Opts) => wrap(api.get<any>(`/v1/fga/stores/${opts?.path?.store_id}/authorization-models/${opts?.path?.model_id}`))
export const fgaReadTuples = (opts?: Opts) => wrap(api.get<any>(`/v1/fga/tuples${qs(opts?.query)}`))
export const fgaWriteTuples = (opts?: Opts) => wrap(api.post<any>('/v1/fga/tuples', opts?.body))
export const fgaDeleteTuples = (opts?: Opts) => wrap(api.delete<any>('/v1/fga/tuples', opts?.body))
export const fgaListObjects = (opts?: Opts) => wrap(api.post<any>('/v1/fga/list-objects', opts?.body))
export const fgaExpand = (opts?: Opts) => wrap(api.post<any>('/v1/fga/expand', opts?.body))
export const fgaBatchTest = (opts?: Opts) => wrap(api.post<any>('/v1/fga/test', opts?.body))

// Orgs
export const listOrgs = (opts?: Opts) => wrap(api.get<{ items: any[] }>(`/v1/orgs${qs(opts?.query)}`))

// Meta Schema
export const getMetaSchema = () => wrap(api.get<any>('/v1/schemas/$meta'))

// ─── Helpers ────────────────────────────────────────────────

function qs(params?: Record<string, any>): string {
  if (!params || Object.keys(params).length === 0) return ''
  const sp = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null && v !== '') sp.set(k, String(v))
  }
  const s = sp.toString()
  return s ? `?${s}` : ''
}
