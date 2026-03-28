/** Shared fetch wrapper with auth handling */
import { toast } from 'vue-sonner'

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

async function request<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const resp = await fetch(`${BASE_URL}${path}`, {
    ...opts,
    headers: {
      'Content-Type': 'application/json',
      ...opts.headers,
    },
    credentials: 'same-origin',
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
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'POST', body: JSON.stringify(body) }),
  put: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'PUT', body: JSON.stringify(body) }),
  patch: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'PATCH', body: JSON.stringify(body) }),
  delete: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'DELETE', ...(body ? { body: JSON.stringify(body) } : {}) }),
}
