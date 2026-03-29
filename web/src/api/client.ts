/** Shared fetch wrapper with auth handling and trace propagation */
import { toast } from 'vue-sonner'
import { getDeviceFingerprint } from '../lib/telemetry'

// Runtime base path: injected by the Go server via <script>window.__ZITADEL_BASE_PATH__="..."</script>
// This allows the same build to work at any sub-path (e.g., /auth, /zitadel).
const BASE_URL = (window as any).__ZITADEL_BASE_PATH__ || ''

/** Structured API error with HTTP status code. */
export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(message: string, status: number, code?: string) {
    super(message)
    this.name = 'ApiError'
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
function credentialsMode(): RequestCredentials {
  if (!BASE_URL) return 'same-origin'
  try {
    const apiOrigin = new URL(BASE_URL, window.location.origin).origin
    return apiOrigin !== window.location.origin ? 'include' : 'same-origin'
  } catch {
    return 'same-origin'
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
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
}

async function request<T>(path: string, opts: RequestInit = {}): Promise<T> {
  // Generate a unique span_id for this request.
  const spanId = generateHex(16)
  const traceparent = `00-${currentTraceId}-${spanId}-01`

  // Include device fingerprint if available (non-blocking — uses cached value).
  const fingerprint = getDeviceFingerprint()

  const resp = await fetch(`${BASE_URL}${path}`, {
    ...opts,
    headers: {
      'Content-Type': 'application/json',
      'Traceparent': traceparent,
      ...(fingerprint ? { 'X-Fingerprint': fingerprint } : {}),
      ...opts.headers,
    },
    credentials: credentialsMode(),
  })

  if (!resp.ok) {
    const body = await resp.json().catch(() => ({ error: resp.statusText }))
    const message = body.error || `HTTP ${resp.status}`
    const code = body.code || undefined

    // Handle 401 globally — session expired or invalid token.
    if (resp.status === 401) {
      handleUnauthorized()
    }

    throw new ApiError(message, resp.status, code)
  }

  // Handle empty responses (e.g. DELETE returns no body)
  const text = await resp.text()
  if (!text) return undefined as T
  return JSON.parse(text)
}

export const api = {
  get: <T>(path: string, headers?: Record<string, string>) =>
    request<T>(path, { headers }),
  post: <T>(path: string, body: unknown, headers?: Record<string, string>) =>
    request<T>(path, { method: 'POST', body: JSON.stringify(body), headers }),
  put: <T>(path: string, body: unknown, headers?: Record<string, string>) =>
    request<T>(path, { method: 'PUT', body: JSON.stringify(body), headers }),
  patch: <T>(path: string, body: unknown, headers?: Record<string, string>) =>
    request<T>(path, { method: 'PATCH', body: JSON.stringify(body), headers }),
  delete: <T>(path: string, body?: unknown, headers?: Record<string, string>) =>
    request<T>(path, { method: 'DELETE', ...(body ? { body: JSON.stringify(body) } : {}), headers }),
}

