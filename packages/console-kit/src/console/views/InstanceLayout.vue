<template>
  <router-view />
</template>

<script setup lang="ts">
/**
 * Layout wrapper for instance-scoped routes.
 *
 * Syncs the :instanceId route param to the display context
 * (for sidebar/breadcrumbs). Fetches instance metadata to resolve
 * the primary domain for display. API scoping happens via URL
 * rewriting in the fetch layer — no headers or localStorage needed.
 */
import { watch, onMounted, onBeforeUnmount, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useInstanceContext } from '@/console/composables/useInstanceContext'
import { instanceApi } from '@/api/resources'

const route = useRoute()
const { setInstance, clearInstance } = useInstanceContext()
const alive = ref(true)

async function syncDisplay() {
  const id = route.params.instanceId as string
  if (!id || !alive.value) return

  setInstance(id, id)
  try {
    const inst = await instanceApi.get(id)
    // Only update if still mounted and still on the same instance
    if (alive.value && route.params.instanceId === id && inst) {
      setInstance(id, inst.primary_domain || id)
    }
  } catch {
    // Keep UUID as fallback
  }
}

onMounted(syncDisplay)
watch(() => route.params.instanceId, syncDisplay)

onBeforeUnmount(() => {
  alive.value = false
  clearInstance()
})
</script>
