<template>
  <Card>
    <CardHeader class="pb-3">
      <CardTitle class="text-sm">System Information</CardTitle>
    </CardHeader>
    <CardContent>
      <dl class="grid grid-cols-[100px_1fr] gap-x-4 gap-y-2 text-sm">
        <dt class="text-muted-foreground">ID</dt>
        <dd class="font-mono text-xs break-all">{{ id }}</dd>
        <template v-if="orgId">
          <dt class="text-muted-foreground">Org</dt>
          <dd>{{ orgId }}</dd>
        </template>
        <template v-if="schemaId">
          <dt class="text-muted-foreground">Schema</dt>
          <dd>{{ schemaId }}</dd>
        </template>
        <slot />
        <template v-if="createdAt">
          <dt class="text-muted-foreground">Created</dt>
          <dd>{{ formatDateTime(createdAt) }}</dd>
        </template>
        <template v-if="updatedAt">
          <dt class="text-muted-foreground">Updated</dt>
          <dd>{{ formatDateTime(updatedAt) }}</dd>
        </template>
      </dl>
    </CardContent>
  </Card>
</template>

<script setup lang="ts">
import { formatDateTime } from '@/console/utils/format'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

defineProps<{
  id: string
  orgId?: string
  schemaId?: string
  createdAt?: string
  updatedAt?: string
}>()
</script>
