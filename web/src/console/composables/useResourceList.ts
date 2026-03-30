/**
 * Generic composable for resource list views.
 *
 * Provides loading state, error handling, search filtering,
 * and fetch wrapper — eliminating boilerplate across list views.
 */
import { ref, computed, type Ref } from 'vue'
import { toast } from 'vue-sonner'

export interface UseResourceListOptions<T> {
  /** Function that fetches the resource list from the API */
  fetchFn: () => Promise<T[]>
  /** Human-readable resource name for error messages */
  resourceName: string
  /** Fields to search against when filtering. Defaults to ['name', 'id'] */
  searchFields?: (keyof T | string)[]
}

export function useResourceList<T extends Record<string, any>>(options: UseResourceListOptions<T>) {
  const items = ref<T[]>([]) as Ref<T[]>
  const loading = ref(false)
  const error = ref('')
  const searchQuery = ref('')

  const searchFields = options.searchFields || ['name', 'id']

  const filteredItems = computed(() => {
    if (!searchQuery.value.trim()) return items.value
    const q = searchQuery.value.toLowerCase()
    return items.value.filter(item =>
      searchFields.some(field => {
        const val = item[field as keyof T]
        return typeof val === 'string' && val.toLowerCase().includes(q)
      })
    )
  })

  async function fetch() {
    loading.value = true
    error.value = ''
    try {
      items.value = await options.fetchFn()
    } catch (e: any) {
      error.value = e?.message || `Failed to load ${options.resourceName}`
      toast.error(`Failed to load ${options.resourceName}`, { description: e?.message })
    } finally {
      loading.value = false
    }
  }

  return {
    items,
    loading,
    error,
    searchQuery,
    filteredItems,
    fetch,
  }
}
