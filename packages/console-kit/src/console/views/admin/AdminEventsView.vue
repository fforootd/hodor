<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Deployment Events</h1>
      <p class="text-sm text-muted-foreground">Operator view — deployment-wide event log across all instances.</p>
    </div>

    <Card>
      <CardContent class="pt-6">
        <div class="flex flex-col items-center justify-center py-12 text-center">
          <Activity class="size-10 text-muted-foreground mb-4" />
          <h3 class="text-lg font-medium">Cross-instance events</h3>
          <p class="text-sm text-muted-foreground mt-1 max-w-md">
            Deployment-wide event log with filtering by instance, event type, and time range will be available here.
          </p>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { Card, CardContent } from '@/components/ui/card'
import { Activity } from 'lucide-vue-next'
</script>
