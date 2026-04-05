import type { LocationQuery, LocationQueryRaw } from 'vue-router'

export type TraceRouteFilterMode = 'generic' | 'actor'

export interface EventRouteFilters {
  actor: string
  aggregateId: string
  eventId: string
  fingerprint: string
  sessionId: string
}

export interface TraceRouteFilter {
  mode: TraceRouteFilterMode
  value: string
}

export function getSessionUserFilter(query: LocationQuery): string {
  return firstQueryValue(query.user_id) || firstQueryValue(query.user)
}

export function getEventRouteFilters(query: LocationQuery): EventRouteFilters {
  return {
    actor: firstQueryValue(query.actor),
    aggregateId: firstQueryValue(query.aggregate_id),
    eventId: firstQueryValue(query.id),
    fingerprint: firstQueryValue(query.fingerprint),
    sessionId: firstQueryValue(query.session_id),
  }
}

export function getTraceRouteFilter(query: LocationQuery): TraceRouteFilter | null {
  const actorId = firstQueryValue(query.actor_id)
  if (actorId) {
    return { mode: 'actor', value: actorId }
  }

  const id = firstQueryValue(query.id)
  if (id) {
    return { mode: 'generic', value: id }
  }

  return null
}

export function buildTraceRouteQuery(
  currentQuery: LocationQuery,
  value: string,
  mode: TraceRouteFilterMode,
): LocationQueryRaw {
  const nextQuery: Record<string, string> = {}

  for (const [key, rawValue] of Object.entries(currentQuery)) {
    if (key === 'actor_id' || key === 'id') continue
    const normalized = firstQueryValue(rawValue)
    if (normalized) {
      nextQuery[key] = normalized
    }
  }

  const trimmed = value.trim()
  if (!trimmed) {
    return nextQuery
  }

  nextQuery[mode === 'actor' ? 'actor_id' : 'id'] = trimmed
  return nextQuery
}

export function buildTraceWhereClause(value: string, mode: TraceRouteFilterMode): string {
  const escaped = escapeSqlLiteral(value)
  if (mode === 'actor') {
    return `actor_id = '${escaped}'`
  }

  return `(request_id = '${escaped}' OR session_id = '${escaped}' OR actor_id = '${escaped}' OR flow_id = '${escaped}' OR fingerprint = '${escaped}' OR client_id = '${escaped}')`
}

export function escapeSqlLiteral(value: string): string {
  return value.replace(/'/g, "''")
}

function firstQueryValue(value: LocationQuery[string] | string | string[] | undefined): string {
  if (Array.isArray(value)) {
    return typeof value[0] === 'string' ? value[0] : ''
  }

  return typeof value === 'string' ? value : ''
}
