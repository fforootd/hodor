/**
 * Shared reactive instance context — display only.
 *
 * Instance scoping is now URL-path-based (/v1/instances/:id/...).
 * This composable only tracks the current instance ID and domain
 * for sidebar/breadcrumb display, synced from route params.
 */
import { ref, readonly } from 'vue'

const _instanceId = ref<string | null>(null)
const _domain = ref<string>('')

export function useInstanceContext() {
  /** Update display state (called by InstanceLayout when route param changes). */
  function setInstance(id: string | null, domain?: string) {
    _instanceId.value = id
    _domain.value = domain || ''
  }

  function clearInstance() {
    setInstance(null)
  }

  return {
    currentInstanceId: readonly(_instanceId),
    currentInstanceDomain: readonly(_domain),
    setInstance,
    clearInstance,
  }
}
