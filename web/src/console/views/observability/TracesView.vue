<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Traces</h1>
        <p class="text-sm text-muted-foreground">End-to-end causal chains via trace_id and span_id.</p>
      </div>
    </div>

    <Card class="py-12">
      <CardContent class="text-center text-muted-foreground">
        <Workflow class="mx-auto size-10 mb-3 opacity-40" />
        <p class="font-medium">Traces Explorer</p>
        <p class="text-sm mt-1">Coming soon: View distributed traces.</p>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { Card, CardContent } from '@/components/ui/card'
import { Workflow } from 'lucide-vue-next'
</script>
