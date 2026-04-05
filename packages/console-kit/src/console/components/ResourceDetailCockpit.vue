<template>
  <div class="space-y-6 pb-10">
    <div
      v-if="loadError && !resource"
      class="rounded-2xl border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive"
    >
      {{ loadError }}
    </div>

    <div
      v-else-if="loading && !resource"
      class="flex h-64 items-center justify-center rounded-3xl border bg-card text-sm text-muted-foreground"
    >
      Loading {{ singularTitle.toLowerCase() }}…
    </div>

    <template v-else-if="resource">
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
                <h1 class="truncate text-3xl font-semibold tracking-tight">{{ displayTitle }}</h1>
                <p v-if="subtitle" class="truncate text-sm text-muted-foreground">{{ subtitle }}</p>
              </div>

              <div v-if="badges.length" class="flex flex-wrap items-center gap-2">
                <Badge v-for="(badge, badgeIndex) in badges" :key="badgeIndex" :variant="badge.variant || 'secondary'" class="text-xs">
                  {{ badge.label }}
                </Badge>
              </div>
            </div>
          </div>

          <div class="flex flex-wrap gap-2 xl:justify-end">
            <slot name="header-actions" />
            <Button variant="destructive" :disabled="deleting" @click="showDeleteConfirm = true">
              {{ deleting ? 'Deleting…' : `Delete ${singularTitle}` }}
            </Button>
          </div>
        </div>
      </section>

      <Tabs v-model="activeTab" class="space-y-6">
        <TabsList class="grid w-full gap-2 rounded-2xl bg-muted/50 p-1" :class="tabsListClass">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger
            v-for="tab in extraTabs"
            :key="tab.value"
            :value="tab.value"
          >
            {{ tab.label }}
          </TabsTrigger>
          <TabsTrigger value="edit">Edit & API</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" class="space-y-6">
          <div class="grid gap-6 xl:grid-cols-[1.35fr_minmax(0,0.95fr)]">
            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <CardTitle class="text-lg">Overview</CardTitle>
                <p class="text-sm text-muted-foreground">
                  {{ overviewDescription }}
                </p>
              </CardHeader>
              <CardContent class="space-y-5">
                <section class="space-y-3">
                  <h2 class="text-sm font-semibold">Key facts</h2>
                  <div v-if="overviewFacts.length" class="space-y-2">
                    <div
                      v-for="(fact, factIndex) in overviewFacts"
                      :key="factIndex"
                      class="flex items-center justify-between gap-4 rounded-2xl border bg-muted/20 px-4 py-3 text-sm"
                    >
                      <span class="text-muted-foreground">{{ fact.label }}</span>
                      <span class="text-right font-medium">{{ fact.value }}</span>
                    </div>
                  </div>
                  <p v-else class="text-sm text-muted-foreground">No schema facts available yet.</p>
                </section>
              </CardContent>
            </Card>

            <div class="space-y-6">
              <Card class="rounded-3xl shadow-sm">
                <CardHeader class="pb-3">
                  <CardTitle class="text-sm">Current State</CardTitle>
                </CardHeader>
                <CardContent class="space-y-4">
                  <div
                    v-for="(row, rowIndex) in stateRows"
                    :key="rowIndex"
                    class="rounded-2xl border bg-muted/20 px-4 py-3"
                  >
                    <p class="text-[11px] uppercase tracking-wider text-muted-foreground">{{ row.label }}</p>
                    <p class="mt-1 text-sm font-medium">{{ row.value }}</p>
                  </div>
                </CardContent>
              </Card>
              <slot name="overview-sidebar" />
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

        <TabsContent value="edit" class="space-y-6">
          <div class="grid gap-6 xl:grid-cols-[1.1fr_minmax(0,0.9fr)]">
            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <CardTitle class="text-lg">Edit {{ singularTitle }}</CardTitle>
                <p class="text-sm text-muted-foreground">
                  Schema-backed form editing stays first. JSON and cURL remain available in developer mode.
                </p>
              </CardHeader>
              <CardContent class="space-y-5">
                <slot name="edit-form" />

                <div class="flex flex-col gap-3 border-t pt-4 sm:flex-row sm:items-center sm:justify-between">
                  <p class="text-sm text-muted-foreground">
                    Save persists the schema-backed resource payload without changing routes or API contracts.
                  </p>
                  <Button :disabled="saving || !jsonValid" @click="$emit('save')">
                    {{ saving ? 'Saving…' : 'Save changes' }}
                  </Button>
                </div>
              </CardContent>
            </Card>

            <ResourceDeveloperTools
              v-model:json-content="jsonContent"
              :curl-snippets="curlSnippets"
              :json-error="jsonError"
              :schema="schema"
              @json-valid="$emit('json-valid', $event)"
              @json-error="$emit('json-error', $event)"
            />
          </div>
        </TabsContent>
      </Tabs>

      <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
        <DialogContent class="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete {{ singularTitle }}</DialogTitle>
            <DialogDescription>
              Delete <strong>{{ displayTitle }}</strong>? This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter class="gap-2">
            <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
            <Button
              variant="destructive"
              :disabled="deleting"
              @click="$emit('delete')"
            >
              {{ deleting ? 'Deleting…' : `Delete ${singularTitle}` }}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import type { CurlSnippet, SummaryFact } from '@/console/utils/schema-resource'
import ResourceDeveloperTools from '@/console/components/ResourceDeveloperTools.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ArrowLeft } from 'lucide-vue-next'

interface BadgeItem {
  label: string
  variant?: 'default' | 'secondary' | 'destructive' | 'outline'
}

interface DetailTab {
  label: string
  value: string
}

const props = withDefaults(defineProps<{
  backRoute: string
  badges?: BadgeItem[]
  curlSnippets: CurlSnippet[]
  deleting: boolean
  displayTitle: string
  eyebrow?: string
  extraTabs?: DetailTab[]
  jsonError?: string
  jsonValid: boolean
  loadError: string
  loading: boolean
  overviewDescription?: string
  overviewFacts: SummaryFact[]
  resource: Record<string, any> | null
  saving: boolean
  schema: Record<string, any> | null
  singularTitle: string
  stateRows: SummaryFact[]
  subtitle?: string
}>(), {
  badges: () => [],
  eyebrow: 'Resource cockpit',
  extraTabs: () => [],
  jsonError: '',
  overviewDescription: 'Review key facts and current state before making changes.',
  subtitle: '',
})

const activeTab = defineModel<string>('activeTab', { default: 'overview' })
const jsonContent = defineModel<string>('jsonContent', { default: '{}' })
const showDeleteConfirm = ref(false)

defineEmits<{
  save: []
  delete: []
  'json-valid': [parsed: Record<string, any>]
  'json-error': [message: string]
}>()

const tabsListClass = computed(() => {
  const count = props.extraTabs.length + 2
  if (count <= 3) return 'grid-cols-3'
  return 'md:grid-cols-4 grid-cols-2'
})
</script>
