/** Shared fetch wrapper with auth handling and trace propagation */
import { toast } from 'vue-sonner'
import { getDeviceFingerprint } from '../lib/telemetry'

// Runtime base path: injected by the Go server via <script>window.__ZITADEL_BASE_PATH__="..."</script>
// This allows the same build to work at any sub-path (e.g., /auth, /zitadel).
const BASE_URL = (window as any).__ZITADEL_BASE_PATH__ || ''

export type ApiErrorKind = 'startup' | 'transport' | 'configuration' | 'flow' | 'internal'

interface ParsedApiError {
  message: string
  code: string
  retryable: boolean
  kind: ApiErrorKind
}

/** Structured API error with HTTP status code. */
export class ApiError extends Error {
  readonly status: number
  readonly code: string
  readonly retryable: boolean
  readonly kind: ApiErrorKind

  constructor(
    message: string,
    status: number,
    code?: string,
    retryable = false,
    kind: ApiErrorKind = 'internal',
  ) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code || `HTTP_${status}`
    this.retryable = retryable
    this.kind = kind
  }

  get isUnauthorized(): boolean {
    return this.status === 401
  }

  get isForbidden(): boolean {
    return this.status === 403
  }
}

function isApiErrorKind(value: unknown): value is ApiErrorKind {
  return (
    value === 'startup' ||
    value === 'transport' ||
    value === 'configuration' ||
    value === 'flow' ||
    value === 'internal'
  )
}

export function parseApiErrorPayload(
  body: any,
  status: number,
  statusText: string,
): ParsedApiError {
  const nested = body?.error
  if (nested && typeof nested === 'object' && !Array.isArray(nested)) {
    return {
      message: nested.message || `HTTP ${status}`,
      code: nested.code || `HTTP_${status}`,
      retryable: Boolean(nested.retryable),
      kind: isApiErrorKind(nested.kind) ? nested.kind : 'internal',
    }
  }

  return {
    message: body?.error || `HTTP ${status || statusText || 0}`,
    code: String(body?.code || `HTTP_${status || 0}`),
    retryable: false,
    kind: 'internal',
  }
}

async function readErrorPayload(resp: Response): Promise<any> {
  const text = await resp.text().catch(() => '')
  if (!text) {
    return { error: resp.statusText }
  }

  try {
    return JSON.parse(text)
  } catch {
    return { error: text }
  }
}

/** Track whether we've already shown a 401 toast to avoid duplicates. */
let is401Redirecting = false

/**
 * Handle 401 Unauthorized responses globally.
 * Shows a toast notification and redirects to the login page.
 */
function handleUnauthorized() {
  if (is401Redirecting) return
  is401Redirecting = true

  toast.error('Session expired', {
    description: 'Your session has expired or is invalid. Redirecting to login…',
    duration: 4000,
  })

  // Redirect after a brief delay so the user sees the toast.
  setTimeout(() => {
    const loginUrl = `${BASE_URL}/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`
    window.location.href = loginUrl
  }, 1500)
}

// Dynamic credentials mode: 'include' for cross-origin (WC embedding), 'same-origin' for same-origin.
export function getApiBaseUrl(): string {
  return BASE_URL
}

export function credentialsMode(baseUrl = BASE_URL): RequestCredentials {
  if (!baseUrl) return 'same-origin'
  try {
    const apiOrigin = new URL(baseUrl, window.location.origin).origin
    return apiOrigin !== window.location.origin ? 'include' : 'same-origin'
  } catch {
    return 'same-origin'
  }
}

export function getCurrentOrgHeader(): string | null {
  try {
    const orgId = localStorage.getItem('zitadel_org')
    return orgId && orgId.trim() ? orgId.trim() : null
  } catch {
    return null
  }
}

// ─── Request Context ───────────────────────────────────────
// Each page navigation generates a new request_id (transmitted via W3C
// Traceparent header). The server extracts the trace-id portion and stores
// it as request_id per ADR-023. All API calls within one navigation share
// the same request_id for correlation.

let currentTraceId = generateHex(32)

/** Reset the request_id on navigation. Call from router.afterEach(). */
export function resetTraceContext() {
  currentTraceId = generateHex(32)
}

function generateHex(length: number): string {
  const bytes = new Uint8Array(length / 2)
  crypto.getRandomValues(bytes)
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

async function fetchWithContext(
  path: string,
  opts: RequestInit = {},
  baseUrl = BASE_URL,
): Promise<Response> {
  // Generate a unique span_id for this request.
  const spanId = generateHex(16)
  const traceparent = `00-${currentTraceId}-${spanId}-01`

  // Include device fingerprint if available (non-blocking — uses cached value).
  const fingerprint = getDeviceFingerprint()

  const headers = new Headers(opts.headers || {})
  if (!(opts.body instanceof FormData) && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json')
  }
  if (!headers.has('X-Org-Id')) {
    const orgId = getCurrentOrgHeader()
    if (orgId) {
      headers.set('X-Org-Id', orgId)
    }
  }

  try {
    return await fetch(`${baseUrl}${path}`, {
      ...opts,
      headers: {
        Traceparent: traceparent,
        ...(fingerprint ? { 'X-Fingerprint': fingerprint } : {}),
        ...Object.fromEntries(headers.entries()),
      },
      credentials: credentialsMode(baseUrl),
    })
  } catch {
    throw new ApiError(
      'Login is temporarily unavailable. Try again in a moment.',
      0,
      'service_unavailable',
      true,
      'transport',
    )
  }
}

export async function requestJSON<T>(
  path: string,
  opts: RequestInit = {},
  baseUrl = BASE_URL,
): Promise<T> {
  const resp = await fetchWithContext(path, opts, baseUrl)

  if (!resp.ok) {
    const body = await readErrorPayload(resp)
    const parsed = parseApiErrorPayload(body, resp.status, resp.statusText)

    // Handle 401 globally — session expired or invalid token.
    if (resp.status === 401) {
      handleUnauthorized()
    }

    throw new ApiError(parsed.message, resp.status, parsed.code, parsed.retryable, parsed.kind)
  }

  // Handle empty responses (e.g. DELETE returns no body)
  const text = await resp.text()
  if (!text) return undefined as T
  return JSON.parse(text)
}

export async function requestText(
  path: string,
  opts: RequestInit = {},
  baseUrl = BASE_URL,
): Promise<string> {
  const resp = await fetchWithContext(path, opts, baseUrl)

  if (!resp.ok) {
    const body = await readErrorPayload(resp)
    const parsed = parseApiErrorPayload(body, resp.status, resp.statusText)
    throw new ApiError(parsed.message, resp.status, parsed.code, parsed.retryable, parsed.kind)
  }

  return resp.text()
}

export const api = {
  get: <T>(path: string, headers?: Record<string, string>) => requestJSON<T>(path, { headers }),
  post: <T>(path: string, body: unknown, headers?: Record<string, string>) =>
    requestJSON<T>(path, { method: 'POST', body: JSON.stringify(body), headers }),
  put: <T>(path: string, body: unknown, headers?: Record<string, string>) =>
    requestJSON<T>(path, { method: 'PUT', body: JSON.stringify(body), headers }),
  patch: <T>(path: string, body: unknown, headers?: Record<string, string>) =>
    requestJSON<T>(path, { method: 'PATCH', body: JSON.stringify(body), headers }),
  delete: <T>(path: string, body?: unknown, headers?: Record<string, string>) =>
    requestJSON<T>(path, {
      method: 'DELETE',
      ...(body ? { body: JSON.stringify(body) } : {}),
      headers,
    }),
  postForm: <T>(path: string, body: FormData, headers?: Record<string, string>) =>
    requestJSON<T>(path, { method: 'POST', body, headers }),
}
