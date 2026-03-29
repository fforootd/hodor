/**
 * Configurable Zitadel API client.
 *
 * Provides a factory for setting up the SDK with custom base URL,
 * authentication, and interceptors.
 */

export interface ClientConfig {
  /** Base URL of the Zitadel instance (e.g., 'https://your-instance.zitadel.cloud'). */
  baseUrl: string

  /**
   * Custom fetch function. Defaults to the global `fetch`.
   * Use this to inject auth headers, error handling, or logging.
   */
  fetch?: typeof fetch

  /**
   * Bearer token for authentication (PAT or session token).
   * If provided, an Authorization header is automatically added.
   */
  token?: string | (() => string | Promise<string>)
}

/**
 * Create a configured Zitadel client.
 *
 * @example
 * ```ts
 * const zitadel = createClient({
 *   baseUrl: '/api',
 *   token: 'pat_...',
 * })
 * ```
 */
export function createClient(config: ClientConfig) {
  const baseFetch = config.fetch ?? globalThis.fetch

  const customFetch: typeof fetch = async (input, init) => {
    const headers = new Headers(init?.headers)

    // Inject auth token if configured.
    if (config.token) {
      const token =
        typeof config.token === 'function' ? await config.token() : config.token
      headers.set('Authorization', `Bearer ${token}`)
    }

    // Ensure JSON content type for requests with body.
    if (init?.body && !headers.has('Content-Type')) {
      headers.set('Content-Type', 'application/json')
    }

    const url =
      typeof input === 'string' && input.startsWith('/')
        ? `${config.baseUrl}${input}`
        : input

    return baseFetch(url, { ...init, headers })
  }

  return {
    fetch: customFetch,
    baseUrl: config.baseUrl,
  }
}
