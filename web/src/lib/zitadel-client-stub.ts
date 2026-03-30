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
export interface FgaReadTuplesResponse { tuples: any[] }
export interface FgaWriteTuplesResponse { [k: string]: any }
export interface FgaDeleteTuplesResponse { [k: string]: any }
export interface FgaListObjectsResponse { objects: string[] }
export interface FgaExpandResponse { tree: any }
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
export const fgaModelGraph = () => wrap(api.get<any>('/v1/fga/model/graph'))
export const fgaCheck = (opts?: Opts) => wrap(api.post<any>('/v1/fga/check', opts?.body))
export const fgaReadTuples = (opts?: Opts) => wrap(api.get<any>(`/v1/fga/tuples${qs(opts?.query)}`))
export const fgaWriteTuples = (opts?: Opts) => wrap(api.post<any>('/v1/fga/tuples', opts?.body))
export const fgaDeleteTuples = (opts?: Opts) => wrap(api.delete<any>('/v1/fga/tuples', opts?.body))
export const fgaListObjects = (opts?: Opts) => wrap(api.post<any>('/v1/fga/list-objects', opts?.body))
export const fgaExpand = (opts?: Opts) => wrap(api.post<any>('/v1/fga/expand', opts?.body))
export const fgaBatchTest = (opts?: Opts) => wrap(api.post<any>('/v1/fga/batch-test', opts?.body))

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
