import { toast } from 'vue-sonner'

export type MutationVerb =
  | 'create'
  | 'update'
  | 'delete'
  | 'save'
  | 'add'
  | 'remove'
  | 'invite'
  | 'refresh'
  | 'install'
  | 'toggle'

const mutationSuccessLabel: Record<MutationVerb, string> = {
  create: 'created',
  update: 'updated',
  delete: 'deleted',
  save: 'saved',
  add: 'added',
  remove: 'removed',
  invite: 'invited',
  refresh: 'refreshed',
  install: 'installed',
  toggle: 'updated',
}

function lowerFirst(value: string): string {
  if (!value) return value
  return value.charAt(0).toLowerCase() + value.slice(1)
}

export function getErrorMessage(error: unknown, fallback = 'Something went wrong'): string {
  if (typeof error === 'string' && error.trim()) return error.trim()
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.trim()) return message.trim()
  }
  return fallback
}

export function notifySuccess(title: string, description?: string) {
  toast.success(title, description ? { description } : undefined)
}

export function notifyError(title: string, error?: unknown, fallbackDescription?: string) {
  const description = error
    ? getErrorMessage(error, fallbackDescription || 'Something went wrong')
    : fallbackDescription

  toast.error(title, description ? { description } : undefined)
}

export function notifyMutationSuccess(
  resource: string,
  action: MutationVerb,
  description?: string,
) {
  notifySuccess(`${resource} ${mutationSuccessLabel[action]}`, description)
}

export function notifyMutationError(
  resource: string,
  action: MutationVerb,
  error?: unknown,
  fallbackDescription?: string,
) {
  notifyError(`Failed to ${action} ${lowerFirst(resource)}`, error, fallbackDescription)
}
