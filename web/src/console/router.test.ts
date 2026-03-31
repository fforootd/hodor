import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@/api/client', () => ({
  resetTraceContext: vi.fn(),
}))

import router from './router'

describe('console router user creation routes', () => {
  beforeEach(async () => {
    await router.push('/')
  })

  it('resolves /users/new to the canonical human user create route', () => {
    const resolved = router.resolve('/users/new')
    const lastMatch = resolved.matched[resolved.matched.length - 1]
    const props = typeof lastMatch?.props?.default === 'function' ? lastMatch.props.default : null

    expect(resolved.name).toBe('user-create')
    expect(props).not.toBeNull()
    expect(props?.(resolved)).toEqual({ schemaType: 'human_user' })
  })

  it('resolves query-based user types on /users/new', () => {
    const resolved = router.resolve('/users/new?type=service_user')
    const lastMatch = resolved.matched[resolved.matched.length - 1]
    const props = typeof lastMatch?.props?.default === 'function' ? lastMatch.props.default : null

    expect(resolved.name).toBe('user-create')
    expect(props?.(resolved)).toEqual({ schemaType: 'service_user' })
  })

  it('redirects legacy schema-shaped user routes to the canonical create path', async () => {
    await router.push('/s/ai_agent/new')

    expect(router.currentRoute.value.fullPath).toBe('/users/new?type=ai_agent')
    expect(router.currentRoute.value.name).toBe('user-create')
  })

  it('resolves the API & Protocols route', () => {
    const resolved = router.resolve('/api-protocols')

    expect(resolved.name).toBe('api-protocols')
    expect(resolved.path).toBe('/api-protocols')
  })
})
