<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Billing</h1>
      <p class="text-sm text-muted-foreground">Manage your subscription and view usage.</p>
    </div>

    <!-- Placeholder -->
    <Card>
      <CardContent class="pt-6">
        <div class="flex flex-col items-center justify-center py-12 text-center">
          <CreditCard class="size-10 text-muted-foreground mb-4" />
          <h3 class="text-lg font-medium">Billing coming soon</h3>
          <p class="text-sm text-muted-foreground mt-1 max-w-md">
            Subscription management, usage tracking, and invoicing will be available here.
          </p>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { Card, CardContent } from '@/components/ui/card'
import { CreditCard } from 'lucide-vue-next'
</script>
