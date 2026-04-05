/** Shared fetch wrapper with auth handling and trace propagation */
import { toast } from 'vue-sonner'
import { getDeviceFingerprint } from '../lib/telemetry'

// ─── Configurable API Client ──────��───────────────────────
// Call configureApi() at app startup to set the base URL,
// org/instance header providers, and 401 handling.
// This allows the same client code to work in both the
// standalone console and the cloud portal.

let _getBaseUrl: () => string = () => ''
let _onUnauthorized: (() => void) | null = null

// Legacy stubs — kept for backward compat with code that imports these.
// Instance scoping is now URL-path-based; org filtering is query-param-based.
/** @deprecated Instance context is now URL-path-based. This is a no-op. */
export function setInstanceContext(_instanceId: string | null) {}
/** @deprecated Instance context is now URL-path-based. Always returns null. */
export function getInstanceContext(): string | null { return null }

export interface ApiClientConfig {
  baseUrl: string | (() => string)
  onUnauthorized?: () => void
}

export function configureApi(config: ApiClientConfig) {
  const bu = config.baseUrl
  _getBaseUrl = typeof bu === 'function' ? bu : () => bu
  if (config.onUnauthorized) _onUnauthorized = config.onUnauthorized
}

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

  if (_onUnauthorized) {
    _onUnauthorized()
    return
  }

  // Default behavior: toast + redirect to login.
  toast.error('Session expired', {
    description: 'Your session has expired or is invalid. Redirecting to login…',
    duration: 4000,
  })

  setTimeout(() => {
    const loginUrl = `${_getBaseUrl()}/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`
    window.location.href = loginUrl
  }, 1500)
}

// Dynamic credentials mode: 'include' for cross-origin (WC embedding), 'same-origin' for same-origin.
export function getApiBaseUrl(): string {
  return _getBaseUrl()
}

export function credentialsMode(baseUrl = _getBaseUrl()): RequestCredentials {
  if (!baseUrl) return 'same-origin'
  try {
    const apiOrigin = new URL(baseUrl, window.location.origin).origin
    return apiOrigin !== window.location.origin ? 'include' : 'same-origin'
  } catch {
    return 'same-origin'
  }
}

/** @deprecated Org filtering is now query-param-based. */
export function getCurrentOrgHeader(): string | null { return null }
/** @deprecated Instance scoping is now URL-path-based. */
export function getCurrentInstanceHeader(): string | null { return null }

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
  baseUrl = _getBaseUrl(),
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

  // Rewrite /v1/... to /v1/instances/:id/... when inside an instance scope.
  // Skip rewriting for root-level endpoints (console bootstrap, admin, auth, instances CRUD).
  let resolvedPath = path
  const instanceMatch = window.location.pathname.match(/\/console\/instances\/([^/]+)/)
  if (
    instanceMatch &&
    path.startsWith('/v1/') &&
    !path.startsWith('/v1/instances') &&
    !path.startsWith('/v1/console/') &&
    !path.startsWith('/v1/admin/') &&
    !path.startsWith('/v1/auth/')
  ) {
    resolvedPath = `/v1/instances/${instanceMatch[1]}${path.slice(3)}`
  }

  try {
    return await fetch(`${baseUrl}${resolvedPath}`, {
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
  baseUrl = _getBaseUrl(),
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
  baseUrl = _getBaseUrl(),
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
