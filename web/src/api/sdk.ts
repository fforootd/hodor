/**
 * SDK client configuration for the Vue console.
 *
 * Bridges the generated @zitadel/client-js SDK with the existing
 * fetch wrapper (auth, errors, toast, base URL).
 */
import { client } from '@zitadel/client-js'
import { toast } from 'vue-sonner'

export { ApiError } from './client'

// Runtime base path from the Go server.
const BASE_URL = (window as any).__ZITADEL_BASE_PATH__ || ''

/** Track 401 redirect to avoid duplicates. */
let is401Redirecting = false

function handleUnauthorized() {
  if (is401Redirecting) return
  is401Redirecting = true
  toast.error('Session expired', {
    description: 'Your session has expired or is invalid. Redirecting to login…',
    duration: 4000,
  })
  setTimeout(() => {
    const loginUrl = `${BASE_URL}/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`
    window.location.href = loginUrl
  }, 1500)
}

/**
 * Custom fetch that prepends base URL, adds credentials,
 * and handles 401 globally.
 */
const consoleFetch: typeof fetch = async (input, init) => {
  const url = typeof input === 'string' && input.startsWith('/')
    ? `${BASE_URL}${input}`
    : input

  const resp = await globalThis.fetch(url, {
    ...init,
    credentials: 'same-origin',
  })

  if (resp.status === 401) {
    handleUnauthorized()
  }

  return resp
}

// Configure the SDK's global client to use our console fetch.
client.setConfig({
  baseUrl: '',
  fetch: consoleFetch,
})

export { client }
