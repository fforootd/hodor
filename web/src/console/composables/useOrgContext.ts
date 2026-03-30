/**
 * Shared reactive org context.
 *
 * All list views watch `currentOrgId` and reload their data when it changes.
 * App.vue calls `setOrg()` when the user switches org.
 */
import { ref, readonly } from 'vue'

// Module-level singleton so re-imports share the same ref.
const _orgId = ref<string | null>(localStorage.getItem('zitadel_org'))

export function useOrgContext() {
  function setOrg(id: string | null) {
    _orgId.value = id
    if (id) {
      localStorage.setItem('zitadel_org', id)
    } else {
      localStorage.removeItem('zitadel_org')
    }
  }

  return {
    currentOrgId: readonly(_orgId),
    setOrg,
  }
}
