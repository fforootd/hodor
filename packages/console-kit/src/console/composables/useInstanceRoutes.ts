/**
 * Instance-aware route resolution.
 *
 * When an instance is selected, resolveRoute('/users') returns
 * '/instances/<id>/users'. In flat mode (no instance), returns '/users'.
 */
import { computed } from 'vue'
import { useRoute } from 'vue-router'

export function useInstanceRoutes() {
  const route = useRoute()

  const instanceId = computed(() => route.params.instanceId as string | undefined)

  const instancePrefix = computed(() =>
    instanceId.value ? `/instances/${instanceId.value}` : '',
  )

  function resolveRoute(path: string): string {
    if (!instanceId.value) return path
    // Don't prefix already-prefixed paths
    if (path.startsWith('/instances/')) return path
    // Don't prefix root-level paths
    if (path.startsWith('/admin/') || path === '/team' || path === '/billing') return path
    return `/instances/${instanceId.value}${path.startsWith('/') ? path : '/' + path}`
  }

  return {
    instanceId,
    instancePrefix,
    resolveRoute,
  }
}
