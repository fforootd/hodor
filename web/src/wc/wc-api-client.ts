/**
 * Standalone API client for Zitadel web components.
 *
 * Unlike `api/client.ts` which depends on vue-sonner (toast) and
 * vue-router (redirect), this client is framework-agnostic and
 * suitable for use in web components embedded in customer apps.
 *
 * Errors are thrown as ApiError instances — the WC layer catches
 * them and dispatches native CustomEvents.
 */

import { credentialsMode } from './host-utils'

/** Structured API error with HTTP status code. */
export class WCApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(message: string, status: number, code?: string) {
    super(message)
    this.name = 'WCApiError'
    this.status = status
    this.code = code || `HTTP_${status}`
  }

  get isUnauthorized(): boolean {
    return this.status === 401
  }

  get isForbidden(): boolean {
    return this.status === 403
  }
}

/**
 * Create a WC-scoped API client bound to a specific base URL.
 *
 * Each web component instance creates its own client with its
 * api-base-url prop, so multiple WCs pointing at different
 * backends can coexist on the same page.
 */
export function createWCApiClient(baseUrl: string) {
  const creds = credentialsMode(baseUrl)

  async function request<T>(path: string, opts: RequestInit = {}): Promise<T> {
    const resp = await fetch(`${baseUrl}${path}`, {
      ...opts,
      headers: {
        'Content-Type': 'application/json',
        ...opts.headers,
      },
      credentials: creds,
    })

    if (!resp.ok) {
      const body = await resp.json().catch(() => ({ error: resp.statusText }))
      const message = body.error || `HTTP ${resp.status}`
      const code = body.code || undefined
      throw new WCApiError(message, resp.status, code)
    }

    // Handle empty responses (e.g. DELETE returns no body)
    const text = await resp.text()
    if (!text) return undefined as T
    return JSON.parse(text)
  }

  return {
    get: <T>(path: string) => request<T>(path),

    post: <T>(path: string, body: unknown) =>
      request<T>(path, { method: 'POST', body: JSON.stringify(body) }),

    put: <T>(path: string, body: unknown) =>
      request<T>(path, { method: 'PUT', body: JSON.stringify(body) }),

    patch: <T>(path: string, body: unknown) =>
      request<T>(path, { method: 'PATCH', body: JSON.stringify(body) }),

    delete: <T>(path: string, body?: unknown) =>
      request<T>(path, {
        method: 'DELETE',
        ...(body ? { body: JSON.stringify(body) } : {}),
      }),
  }
}

/** Type for the WC API client returned by createWCApiClient. */
export type WCApiClient = ReturnType<typeof createWCApiClient>
