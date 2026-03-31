import type { RouteLocationRaw } from 'vue-router'

export const USER_SCHEMA_TYPES = ['human_user', 'service_user', 'ai_agent'] as const

export type UserSchemaType = (typeof USER_SCHEMA_TYPES)[number]

const USER_SCHEMA_TYPE_SET = new Set<string>(USER_SCHEMA_TYPES)

const USER_SCHEMA_LABELS: Record<UserSchemaType, string> = {
  human_user: 'User',
  service_user: 'Service Account',
  ai_agent: 'AI Agent',
}

export function isUserSchemaType(value: string): value is UserSchemaType {
  return USER_SCHEMA_TYPE_SET.has(value)
}

export function normalizeUserSchemaType(value: unknown): UserSchemaType {
  const raw = Array.isArray(value) ? value[0] : value
  if (typeof raw === 'string' && isUserSchemaType(raw)) {
    return raw
  }
  return 'human_user'
}

export function getUserSchemaLabel(schemaType: string): string {
  if (isUserSchemaType(schemaType)) {
    return USER_SCHEMA_LABELS[schemaType]
  }
  return schemaType
}

export function buildUserCreateRoute(schemaType: string): RouteLocationRaw {
  const normalized = normalizeUserSchemaType(schemaType)
  if (normalized === 'human_user') {
    return '/users/new'
  }
  return {
    path: '/users/new',
    query: { type: normalized },
  }
}

export function buildUserDetailRoute(userId: string): string {
  return `/users/${encodeURIComponent(userId)}`
}

export function buildUserListRoute(): string {
  return '/users'
}
