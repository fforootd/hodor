/**
 * Generic composable for resource detail views.
 *
 * Provides load, save, delete (with confirmation), and
 * edit-mode management for any single-resource detail view.
 */
import { ref, computed, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'

export interface UseResourceDetailOptions<T> {
  /** Function that fetches the resource by ID */
  fetchFn: (id: string) => Promise<T>
  /** Function that updates the resource */
  updateFn?: (id: string, data: Partial<T>) => Promise<T>
  /** Function that deletes the resource */
  deleteFn?: (id: string) => Promise<void>
  /** Route to navigate to after deletion */
  listRoute: string
  /** Human-readable resource name for messages */
  resourceName: string
}

export function useResourceDetail<T extends Record<string, any>>(options: UseResourceDetailOptions<T>) {
  const route = useRoute()
  const router = useRouter()

  const item = ref<T | null>(null) as Ref<T | null>
  const loading = ref(false)
  const saving = ref(false)
  const deleting = ref(false)
  const error = ref('')
  const showDeleteConfirm = ref(false)

  const resourceId = computed(() => route.params.id as string)

  async function load() {
    loading.value = true
    error.value = ''
    try {
      item.value = await options.fetchFn(resourceId.value)
    } catch (e: any) {
      error.value = e?.message || `Failed to load ${options.resourceName}`
      toast.error(`Failed to load ${options.resourceName}`, { description: e?.message })
    } finally {
      loading.value = false
    }
  }

  async function save(data: Partial<T>) {
    if (!options.updateFn) return
    saving.value = true
    error.value = ''
    try {
      item.value = await options.updateFn(resourceId.value, data)
      toast.success(`${options.resourceName} updated`)
    } catch (e: any) {
      error.value = e?.message || `Failed to update ${options.resourceName}`
      toast.error(`Failed to update ${options.resourceName}`, { description: e?.message })
    } finally {
      saving.value = false
    }
  }

  async function confirmDelete() {
    if (!options.deleteFn) return
    deleting.value = true
    try {
      await options.deleteFn(resourceId.value)
      toast.success(`${options.resourceName} deleted`)
      router.push(options.listRoute)
    } catch (e: any) {
      toast.error(`Failed to delete ${options.resourceName}`, { description: e?.message })
      showDeleteConfirm.value = false
    } finally {
      deleting.value = false
    }
  }

  return {
    item,
    loading,
    saving,
    deleting,
    error,
    showDeleteConfirm,
    resourceId,
    load,
    save,
    confirmDelete,
  }
}
