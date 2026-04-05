<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">System Config</h1>
      <p class="text-sm text-muted-foreground">Operator view — deployment-level settings and feature defaults.</p>
    </div>

    <Card>
      <CardContent class="pt-6">
        <div class="flex flex-col items-center justify-center py-12 text-center">
          <Wrench class="size-10 text-muted-foreground mb-4" />
          <h3 class="text-lg font-medium">System configuration</h3>
          <p class="text-sm text-muted-foreground mt-1 max-w-md">
            Deployment-level settings, feature flag defaults, and system health monitoring will be available here.
          </p>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { Card, CardContent } from '@/components/ui/card'
import { Wrench } from 'lucide-vue-next'
</script>
