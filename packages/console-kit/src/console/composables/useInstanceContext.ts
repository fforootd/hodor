/**
 * Shared reactive instance context — display only.
 *
 * Instance scoping is now URL-path-based (/v1/instances/:id/...).
 * This composable only tracks the current instance ID and domain
 * for sidebar/breadcrumb display, synced from route params.
 */
import { ref } from 'vue'

const _instanceId = ref<string | null>(null)
const _domain = ref('')

export function useInstanceContext() {
  /** Update display state (called by InstanceLayout when route param changes). */
  function setInstance(id: string | null, domain?: string) {
    _instanceId.value = id
    _domain.value = domain || ''
  }

  function clearInstance() {
    _instanceId.value = null
    _domain.value = ''
  }

  return {
    currentInstanceId: _instanceId,
    currentInstanceDomain: _domain,
    setInstance,
    clearInstance,
  }
}
