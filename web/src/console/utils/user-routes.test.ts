import { describe, expect, it } from 'vitest'
import { buildUserCreateRoute, getUserSchemaLabel, normalizeUserSchemaType } from './user-routes'

describe('user route helpers', () => {
  it('defaults unknown create types to human users', () => {
    expect(normalizeUserSchemaType(undefined)).toBe('human_user')
    expect(normalizeUserSchemaType('not_a_user')).toBe('human_user')
  })

  it('keeps human user creation on the clean canonical route', () => {
    expect(buildUserCreateRoute('human_user')).toBe('/users/new')
  })

  it('uses query params for non-human user create routes', () => {
    expect(buildUserCreateRoute('service_user')).toEqual({
      path: '/users/new',
      query: { type: 'service_user' },
    })
    expect(buildUserCreateRoute('ai_agent')).toEqual({
      path: '/users/new',
      query: { type: 'ai_agent' },
    })
  })

  it('maps schema types to customer-facing labels', () => {
    expect(getUserSchemaLabel('human_user')).toBe('User')
    expect(getUserSchemaLabel('service_user')).toBe('Service Account')
    expect(getUserSchemaLabel('ai_agent')).toBe('AI Agent')
  })
})
