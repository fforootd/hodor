import { describe, expect, it } from 'vitest'
import { ApiError } from '@/api/client'
import { nextLoginInitRetryDelay, shouldRetryLoginInit, toLoginErrorDetail } from './init-state'

describe('login init state helpers', () => {
  it('maps ApiError into login error detail', () => {
    const detail = toLoginErrorDetail(
      new ApiError(
        'Zitadel is still starting. Try again in a moment.',
        503,
        'service_starting',
        true,
        'startup',
      ),
    )

    expect(detail).toEqual({
      code: 'service_starting',
      message: 'Zitadel is still starting. Try again in a moment.',
      retryable: true,
      kind: 'startup',
      status: 503,
    })
  })

  it('treats unknown errors as retryable transport failures', () => {
    expect(toLoginErrorDetail(new Error('boom'))).toEqual({
      code: 'service_unavailable',
      message: 'Zitadel is temporarily unavailable. Try again in a moment.',
      retryable: true,
      kind: 'transport',
    })
  })

  it('retries only startup and transport errors within the retry budget', () => {
    expect(
      shouldRetryLoginInit(
        {
          code: 'service_starting',
          message: 'starting',
          retryable: true,
          kind: 'startup',
        },
        0,
      ),
    ).toBe(true)

    expect(
      shouldRetryLoginInit(
        {
          code: 'flow_config_invalid',
          message: 'bad config',
          retryable: false,
          kind: 'configuration',
        },
        0,
      ),
    ).toBe(false)

    expect(
      shouldRetryLoginInit(
        {
          code: 'service_unavailable',
          message: 'down',
          retryable: true,
          kind: 'transport',
        },
        99,
      ),
    ).toBe(false)
  })

  it('returns the configured retry delays', () => {
    expect(nextLoginInitRetryDelay(0)).toBe(250)
    expect(nextLoginInitRetryDelay(4)).toBe(3000)
    expect(nextLoginInitRetryDelay(5)).toBeNull()
  })
})
