/** Shared fetch wrapper with auth handling */

// Runtime base path: injected by the Go server via <script>window.__ZITADEL_BASE_PATH__="..."</script>
// This allows the same build to work at any sub-path (e.g., /auth, /zitadel).
const BASE_URL = (window as any).__ZITADEL_BASE_PATH__ || ''

interface ApiError {
  error: string
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
    const body = await resp.json().catch(() => ({ error: resp.statusText })) as ApiError
    throw new Error(body.error || `HTTP ${resp.status}`)
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
  delete: <T>(path: string) =>
    request<T>(path, { method: 'DELETE' }),
}
