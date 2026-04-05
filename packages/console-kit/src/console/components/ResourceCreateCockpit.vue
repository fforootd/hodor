<template>
  <div class="space-y-6 pb-10">
    <section class="sticky top-0 z-10 rounded-3xl border bg-background/95 p-6 shadow-sm backdrop-blur">
      <div class="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
        <div class="flex items-start gap-4">
          <Button variant="ghost" size="icon" as-child class="mt-1 shrink-0">
            <RouterLink :to="backRoute" :aria-label="`Back to ${singularTitle}`">
              <ArrowLeft class="size-4" />
            </RouterLink>
          </Button>

          <div class="min-w-0 space-y-3">
            <div class="space-y-1">
              <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                {{ eyebrow }}
              </p>
              <h1 class="truncate text-3xl font-semibold tracking-tight">Create {{ singularTitle }}</h1>
              <p class="max-w-2xl text-sm text-muted-foreground">
                {{ description }}
              </p>
            </div>

            <div v-if="badges.length" class="flex flex-wrap items-center gap-2">
              <Badge v-for="(badge, badgeIndex) in badges" :key="badgeIndex" :variant="badge.variant || 'secondary'" class="text-xs">
                {{ badge.label }}
              </Badge>
            </div>
          </div>
        </div>

        <div class="flex flex-wrap gap-2 xl:justify-end">
          <Button variant="outline" as-child>
            <RouterLink :to="backRoute">Cancel</RouterLink>
          </Button>
          <Button :disabled="submitting || !jsonValid" @click="$emit('submit')">
            {{ submitting ? 'Creating…' : `Create ${singularTitle}` }}
          </Button>
        </div>
      </div>
    </section>

    <FormError :error="error" />

    <Tabs v-model="activeTab" class="space-y-6">
      <TabsList class="grid w-full gap-2 rounded-2xl bg-muted/50 p-1" :class="tabsListClass">
        <TabsTrigger value="details">Details</TabsTrigger>
        <TabsTrigger
          v-for="tab in extraTabs"
          :key="tab.value"
          :value="tab.value"
        >
          {{ tab.label }}
        </TabsTrigger>
        <TabsTrigger value="review">Review</TabsTrigger>
        <TabsTrigger value="api">API</TabsTrigger>
      </TabsList>

      <TabsContent value="details" class="space-y-6">
        <div class="grid gap-6 xl:grid-cols-[1.35fr_minmax(0,0.95fr)]">
          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Details</CardTitle>
              <p class="text-sm text-muted-foreground">
                {{ detailsDescription }}
              </p>
            </CardHeader>
            <CardContent class="space-y-5">
              <slot name="details" />
            </CardContent>
          </Card>

          <div class="space-y-6">
            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <CardTitle class="text-sm">Operator Summary</CardTitle>
              </CardHeader>
              <CardContent class="space-y-4">
                <div
                  v-for="(card, cardIndex) in summaryCards"
                  :key="cardIndex"
                  class="rounded-2xl border bg-muted/20 px-4 py-3"
                >
                  <p class="text-[11px] uppercase tracking-wider text-muted-foreground">{{ card.label }}</p>
                  <p class="mt-1 text-sm font-medium">{{ card.value }}</p>
                </div>
              </CardContent>
            </Card>
          </div>
        </div>
      </TabsContent>

      <TabsContent
        v-for="tab in extraTabs"
        :key="tab.value"
        :value="tab.value"
        class="space-y-6"
      >
        <slot :name="`tab-${tab.value}`" />
      </TabsContent>

      <TabsContent value="review" class="space-y-6">
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)]">
          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Review</CardTitle>
              <p class="text-sm text-muted-foreground">
                Confirm the resource values before creating it.
              </p>
            </CardHeader>
            <CardContent class="space-y-5">
              <section class="space-y-3">
                <h2 class="text-sm font-semibold">Resource values</h2>
                <div v-if="reviewRows.length" class="space-y-2">
                  <div
                    v-for="(row, rowIndex) in reviewRows"
                    :key="rowIndex"
                    class="flex items-center justify-between gap-4 rounded-2xl border bg-muted/20 px-4 py-3 text-sm"
                  >
                    <span class="text-muted-foreground">{{ row.label }}</span>
                    <span class="text-right font-medium">{{ row.value }}</span>
                  </div>
                </div>
                <p v-else class="text-sm text-muted-foreground">No values captured yet.</p>
              </section>
            </CardContent>
          </Card>

          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Create {{ singularTitle }}</CardTitle>
            </CardHeader>
            <CardContent class="space-y-4">
              <div
                v-for="(card, cardIndex) in reviewSummaryCards"
                :key="cardIndex"
                class="rounded-2xl border bg-muted/20 px-4 py-3"
              >
                <p class="text-[11px] uppercase tracking-wider text-muted-foreground">{{ card.label }}</p>
                <p class="mt-1 text-sm font-medium">{{ card.value }}</p>
              </div>
              <Button class="w-full" :disabled="submitting || !jsonValid" @click="$emit('submit')">
                {{ submitting ? 'Creating…' : `Create ${singularTitle}` }}
              </Button>
            </CardContent>
          </Card>
        </div>
      </TabsContent>

      <TabsContent value="api" class="space-y-6">
        <ResourceDeveloperTools
          v-model:json-content="jsonContent"
          :curl-snippets="curlSnippets"
          :json-error="jsonError"
          :schema="schema"
          @json-valid="$emit('json-valid', $event)"
          @json-error="$emit('json-error', $event)"
        />
      </TabsContent>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import type { CurlSnippet, SummaryFact } from '@/console/utils/schema-resource'
import ResourceDeveloperTools from '@/console/components/ResourceDeveloperTools.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { FormError } from '@/components/ui/form-error'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ArrowLeft } from 'lucide-vue-next'

interface BadgeItem {
  label: string
  variant?: 'default' | 'secondary' | 'destructive' | 'outline'
}

interface CreateTab {
  label: string
  value: string
}

const props = withDefaults(defineProps<{
  backRoute: string
  badges?: BadgeItem[]
  curlSnippets: CurlSnippet[]
  description: string
  detailsDescription?: string
  error: string
  eyebrow?: string
  extraTabs?: CreateTab[]
  jsonError?: string
  jsonValid: boolean
  reviewRows: SummaryFact[]
  reviewSummaryCards: SummaryFact[]
  schema: Record<string, any> | null
  singularTitle: string
  submitting: boolean
  summaryCards: SummaryFact[]
}>(), {
  badges: () => [],
  detailsDescription: 'Capture the resource values that matter operationally first.',
  eyebrow: 'Resource creation',
  extraTabs: () => [],
  jsonError: '',
})

const activeTab = defineModel<string>('activeTab', { default: 'details' })
const jsonContent = defineModel<string>('jsonContent', { default: '{}' })

defineEmits<{
  submit: []
  'json-valid': [parsed: Record<string, any>]
  'json-error': [message: string]
}>()

const tabsListClass = computed(() => {
  const count = props.extraTabs.length + 3
  if (count <= 3) return 'grid-cols-3'
  if (count === 4) return 'md:grid-cols-4 grid-cols-2'
  return 'md:grid-cols-5 grid-cols-2'
})
</script>
