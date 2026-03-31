import { describe, expect, it } from 'vitest'

import {
  buildTraceRouteQuery,
  buildTraceWhereClause,
  getEventRouteFilters,
  getSessionUserFilter,
  getTraceRouteFilter,
} from './route-filters'

describe('console route filters', () => {
  it('prefers canonical user_id for session drilldowns while keeping user as a fallback', () => {
    expect(getSessionUserFilter({ user_id: 'user-123' })).toBe('user-123')
    expect(getSessionUserFilter({ user: 'legacy-user' })).toBe('legacy-user')
  })

  it('reads aggregate_id for event drilldowns while preserving older filters', () => {
    expect(getEventRouteFilters({
      aggregate_id: 'user-123',
      actor: 'admin-1',
      fingerprint: 'fp-1',
      id: 'evt-1',
      session_id: 'sess-1',
    })).toEqual({
      actor: 'admin-1',
      aggregateId: 'user-123',
      eventId: 'evt-1',
      fingerprint: 'fp-1',
      sessionId: 'sess-1',
    })
  })

  it('treats actor_id as the explicit traces drilldown and keeps id for generic lookup', () => {
    expect(getTraceRouteFilter({ actor_id: 'user-123' })).toEqual({
      mode: 'actor',
      value: 'user-123',
    })
    expect(getTraceRouteFilter({ id: 'req-123' })).toEqual({
      mode: 'generic',
      value: 'req-123',
    })
  })

  it('builds canonical trace queries and where clauses for actor drilldowns', () => {
    expect(buildTraceRouteQuery({ page: '2', id: 'old' }, 'user-123', 'actor')).toEqual({
      actor_id: 'user-123',
      page: '2',
    })
    expect(buildTraceWhereClause("user-'123", 'actor')).toBe("actor_id = 'user-''123'")
  })
})
