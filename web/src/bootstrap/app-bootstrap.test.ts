import { describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/client'
import {
  shouldRetryAppBootstrap,
  toAppBootstrapErrorDetail,
  useAppBootstrap,
} from './app-bootstrap'

describe('app bootstrap helpers', () => {
  it('maps readyz 503 responses into startup errors', () => {
    expect(
      toAppBootstrapErrorDetail({
        status: 503,
        error: 'starting',
      }),
    ).toEqual({
      code: 'service_starting',
      message: 'Zitadel is still starting. Try again in a moment.',
      retryable: true,
      kind: 'startup',
      status: 503,
    })
  })

  it('does not retry authorization failures', () => {
    expect(
      shouldRetryAppBootstrap(
        {
          code: 'HTTP_401',
          message: 'unauthorized',
          retryable: false,
          kind: 'internal',
          status: 401,
        },
        0,
      ),
    ).toBe(false)
  })

  it('retries startup failures and becomes ready when the service recovers', async () => {
    const task = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(
        new ApiError(
          'Zitadel is still starting. Try again in a moment.',
          503,
          'service_starting',
          true,
          'startup',
        ),
      )
      .mockResolvedValueOnce()

    const bootstrap = useAppBootstrap(task, {
      waitForReady: async () => {},
    })

    await expect(bootstrap.run()).resolves.toBe(true)
    expect(task).toHaveBeenCalledTimes(2)
    expect(bootstrap.state.value).toBe('ready')
    expect(bootstrap.error.value).toBeNull()
  })

  it('stops on fatal configuration errors', async () => {
    const task = vi.fn<() => Promise<void>>().mockRejectedValue(
      new ApiError(
        'Login is not configured correctly.',
        500,
        'flow_config_invalid',
        false,
        'configuration',
      ),
    )
    const onFatal = vi.fn()
    const bootstrap = useAppBootstrap(task, { onFatal })

    await expect(bootstrap.run()).resolves.toBe(false)
    expect(bootstrap.state.value).toBe('fatal')
    expect(onFatal).toHaveBeenCalledTimes(1)
  })
})
