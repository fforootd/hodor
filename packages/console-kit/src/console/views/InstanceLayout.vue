<template>
  <router-view />
</template>

<script setup lang="ts">
/**
 * Layout wrapper for instance-scoped routes.
 *
 * Syncs the :instanceId route param to the display context
 * (for sidebar/breadcrumbs). API scoping happens via URL rewriting
 * in the fetch layer — no headers or localStorage needed.
 */
import { watch, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useInstanceContext } from '@/console/composables/useInstanceContext'

const route = useRoute()
const { setInstance } = useInstanceContext()

function syncDisplay() {
  const id = route.params.instanceId as string
  if (id) {
    setInstance(id, id)
  }
}

onMounted(syncDisplay)
watch(() => route.params.instanceId, syncDisplay)
</script>
