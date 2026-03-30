import { ref, type Ref } from 'vue'
import {
  ApiError,
  parseApiErrorPayload,
  type ApiErrorKind,
} from '@/api/client'
import { flowApi } from '@/api/branding'

export type AppBootstrapState = 'initializing' | 'waiting_for_server' | 'ready' | 'fatal'
export type AppBootstrapErrorKind = ApiErrorKind

export interface AppBootstrapErrorDetail {
  code: string
  message: string
  retryable: boolean
  kind: AppBootstrapErrorKind
  status?: number
}

interface UseAppBootstrapOptions {
  waitForReady?: AppBootstrapWaiter
  onFatal?: (detail: AppBootstrapErrorDetail) => void
}

export type AppBootstrapWaiter = (
  delayMs: number,
  isDisposed: () => boolean,
) => Promise<void>

const APP_BOOTSTRAP_RETRY_DELAYS = [250, 500, 1000, 2000, 3000]
const DEFAULT_BOOTSTRAP_ERROR: AppBootstrapErrorDetail = {
  code: 'service_unavailable',
  message: 'Zitadel is temporarily unavailable. Try again in a moment.',
  retryable: true,
  kind: 'transport',
}

export function nextAppBootstrapRetryDelay(attempt: number): number | null {
  return APP_BOOTSTRAP_RETRY_DELAYS[attempt] ?? null
}

export function shouldRetryAppBootstrap(
  detail: AppBootstrapErrorDetail,
  attempt: number,
): boolean {
  if (detail.status === 401 || detail.status === 403) return false
  if (detail.kind === 'configuration' || detail.kind === 'flow') return false
  if (!detail.retryable && !isRetryableStatus(detail.status)) return false
  return nextAppBootstrapRetryDelay(attempt) !== null
}

export function toAppBootstrapErrorDetail(err: unknown): AppBootstrapErrorDetail {
  if (err instanceof ApiError) {
    return normalizeBootstrapError({
      code: err.code,
      message: err.message,
      retryable: err.retryable,
      kind: err.kind,
      status: err.status || undefined,
    })
  }

  if (err && typeof err === 'object' && looksLikeStructuredBootstrapError(err)) {
    const status = extractStatus(err)
    const statusText = extractStatusText(err)
    const parsed = parseApiErrorPayload(err, status ?? 0, statusText)
    return normalizeBootstrapError({
      code: parsed.code,
      message: parsed.message,
      retryable: parsed.retryable,
      kind: parsed.kind,
      status,
    })
  }

  return { ...DEFAULT_BOOTSTRAP_ERROR }
}

export function createReadyzWaiter(baseUrl = ''): AppBootstrapWaiter {
  return async (delayMs, isDisposed) => {
    const startedAt = Date.now()
    while (!isDisposed() && Date.now() - startedAt < delayMs) {
      const ready = await flowApi.ready(baseUrl).catch(() => false)
      if (ready) return
      await sleep(Math.min(250, delayMs))
    }
  }
}

export function useAppBootstrap(
  bootstrapTask: () => Promise<void>,
  options: UseAppBootstrapOptions = {},
): {
  state: Ref<AppBootstrapState>
  error: Ref<AppBootstrapErrorDetail | null>
  retryDelayMs: Ref<number>
  run: () => Promise<boolean>
  retry: () => Promise<boolean>
  dispose: () => void
} {
  const state = ref<AppBootstrapState>('initializing')
  const error = ref<AppBootstrapErrorDetail | null>(null)
  const retryDelayMs = ref(0)
  let disposed = false

  async function run() {
    let attempt = 0

    while (!disposed) {
      state.value = attempt === 0 ? 'initializing' : 'waiting_for_server'
      retryDelayMs.value = 0

      try {
        await bootstrapTask()
        state.value = 'ready'
        error.value = null
        retryDelayMs.value = 0
        return true
      } catch (err) {
        const detail = toAppBootstrapErrorDetail(err)
        error.value = detail

        if (!shouldRetryAppBootstrap(detail, attempt)) {
          state.value = 'fatal'
          options.onFatal?.(detail)
          return false
        }

        const delay = nextAppBootstrapRetryDelay(attempt)
        if (delay == null) {
          state.value = 'fatal'
          options.onFatal?.(detail)
          return false
        }

        state.value = 'waiting_for_server'
        retryDelayMs.value = delay

        if (options.waitForReady) {
          await options.waitForReady(delay, () => disposed)
        } else {
          await sleep(delay)
        }

        attempt += 1
      }
    }

    return false
  }

  async function retry() {
    error.value = null
    state.value = 'initializing'
    retryDelayMs.value = 0
    return run()
  }

  function dispose() {
    disposed = true
  }

  return {
    state,
    error,
    retryDelayMs,
    run,
    retry,
    dispose,
  }
}

function normalizeBootstrapError(
  detail: AppBootstrapErrorDetail,
): AppBootstrapErrorDetail {
  const status = detail.status || undefined
  const isReadyzFailure = status === 503
  const message =
    isReadyzFailure &&
    (!detail.message ||
      detail.message === 'starting' ||
      detail.message.startsWith('HTTP 503'))
      ? 'Zitadel is still starting. Try again in a moment.'
      : detail.message || DEFAULT_BOOTSTRAP_ERROR.message

  return {
    code: isReadyzFailure ? 'service_starting' : detail.code || DEFAULT_BOOTSTRAP_ERROR.code,
    message,
    retryable: detail.retryable || isRetryableStatus(status),
    kind:
      isReadyzFailure && detail.kind === 'internal'
        ? 'startup'
        : detail.kind,
    status,
  }
}

function extractStatus(err: object): number | undefined {
  const candidate = (err as { status?: unknown }).status
  if (typeof candidate === 'number') return candidate

  const responseStatus = (err as { response?: { status?: unknown } }).response?.status
  return typeof responseStatus === 'number' ? responseStatus : undefined
}

function extractStatusText(err: object): string {
  const direct = (err as { statusText?: unknown }).statusText
  if (typeof direct === 'string') return direct

  const nested = (err as { response?: { statusText?: unknown } }).response?.statusText
  return typeof nested === 'string' ? nested : ''
}

function looksLikeStructuredBootstrapError(err: object): boolean {
  const candidate = err as {
    status?: unknown
    response?: { status?: unknown }
    error?: unknown
    code?: unknown
  }
  return (
    typeof candidate.status === 'number' ||
    typeof candidate.response?.status === 'number' ||
    candidate.error !== undefined ||
    candidate.code !== undefined
  )
}

function isRetryableStatus(status?: number): boolean {
  return typeof status === 'number' && status >= 500
}

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms))
}
