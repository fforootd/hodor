import { ApiError, type ApiErrorKind } from '@/api/client'

export type LoginInitState = 'initializing' | 'waiting_for_server' | 'ready' | 'fatal'
export type LoginErrorKind = ApiErrorKind

export interface LoginErrorDetail {
  code: string
  message: string
  retryable: boolean
  kind: LoginErrorKind
  status?: number
}

const LOGIN_INIT_RETRY_DELAYS = [250, 500, 1000, 2000, 3000]

export function toLoginErrorDetail(err: unknown): LoginErrorDetail {
  if (err instanceof ApiError) {
    return {
      code: err.code,
      message: err.message,
      retryable: err.retryable,
      kind: err.kind,
      status: err.status || undefined,
    }
  }

  return {
    code: 'service_unavailable',
    message: 'Login is temporarily unavailable. Try again in a moment.',
    retryable: true,
    kind: 'transport',
  }
}

export function nextLoginInitRetryDelay(attempt: number): number | null {
  return LOGIN_INIT_RETRY_DELAYS[attempt] ?? null
}

export function shouldRetryLoginInit(detail: LoginErrorDetail, attempt: number): boolean {
  if (!detail.retryable) return false
  if (detail.kind !== 'startup' && detail.kind !== 'transport') return false
  return nextLoginInitRetryDelay(attempt) !== null
}
