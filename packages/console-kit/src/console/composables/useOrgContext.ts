/**
 * Shared reactive org context.
 *
 * All list views watch `currentOrgId` and reload their data when it changes.
 * App.vue calls `setOrg()` when the user switches org.
 */
import { ref, readonly } from 'vue'

// Module-level singleton so re-imports share the same ref.
function getStoredOrg(): string | null {
  try { return localStorage.getItem('zitadel_org') } catch { return null }
}
const _orgId = ref<string | null>(getStoredOrg())

export function useOrgContext() {
  function setOrg(id: string | null) {
    _orgId.value = id
    try {
      if (id) {
        localStorage.setItem('zitadel_org', id)
      } else {
        localStorage.removeItem('zitadel_org')
      }
    } catch { /* test env */ }
  }

  return {
    currentOrgId: readonly(_orgId),
    setOrg,
  }
}
