<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Staff Grants</h1>
      <p class="text-sm text-muted-foreground">Time-limited support access to child instances.</p>
    </div>

    <Card>
      <CardContent class="py-12">
        <div class="text-center space-y-3">
          <div class="mx-auto flex size-12 items-center justify-center rounded-full bg-muted">
            <ShieldAlert class="size-6 text-muted-foreground" />
          </div>
          <div>
            <p class="text-sm font-medium">Coming soon</p>
            <p class="mt-1 text-sm text-muted-foreground max-w-md mx-auto">
              Staff access grants allow time-limited, role-scoped support access to child instances
              with full audit trail. See ADR-036 for the design.
            </p>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { Card, CardContent } from '@/components/ui/card'
import { ShieldAlert } from 'lucide-vue-next'
</script>
