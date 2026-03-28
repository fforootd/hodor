<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Schemas</h1>
      <p class="text-sm text-muted-foreground mt-1">Schema types with version history.</p>
    </div>

    <!-- Search / filter bar -->
    <div class="flex items-center gap-3">
      <div class="relative w-full max-w-sm">
        <SearchIcon class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
        <Input
          v-model="searchQuery"
          placeholder="Filter schemas…"
          class="pl-9 bg-background"
        />
      </div>
      <span class="text-xs text-muted-foreground">
        {{ filteredGroups.length }} type{{ filteredGroups.length !== 1 ? 's' : '' }}
      </span>
    </div>

    <!-- Schema type cards -->
    <Card v-for="group in filteredGroups" :key="group.type">
      <Collapsible v-model:open="group.open">
        <CardHeader class="flex-row items-center justify-between space-y-0 py-3">
          <div class="flex items-center gap-3">
            <CollapsibleTrigger class="flex items-center gap-2 cursor-pointer hover:text-foreground transition-colors group">
              <ChevronRight class="size-4 text-muted-foreground transition-transform group-data-[state=open]:rotate-90" />
              <CardTitle class="text-base">{{ group.type }}</CardTitle>
            </CollapsibleTrigger>
            <Badge variant="secondary" class="text-xs font-normal">
              {{ group.versions.length }} version{{ group.versions.length !== 1 ? 's' : '' }}
            </Badge>
          </div>
          <div class="flex items-center gap-1.5">
            <!-- Quick field preview -->
            <div class="hidden md:flex gap-1">
              <Badge v-for="field in schemaFields(group.defaultVersion).slice(0, 4)" :key="field" variant="outline" class="text-[10px] font-normal">
                {{ field }}
              </Badge>
              <Badge v-if="schemaFields(group.defaultVersion).length > 4" variant="outline" class="text-[10px] font-normal">
                +{{ schemaFields(group.defaultVersion).length - 4 }}
              </Badge>
            </div>
          </div>
        </CardHeader>
        <CollapsibleContent>
          <CardContent class="pt-0 pb-4">
            <!-- Version timeline -->
            <div class="relative pl-6">
              <div
                v-for="(s, idx) in group.versions" :key="s.id"
                class="relative pb-3 last:pb-0"
                :class="idx < group.versions.length - 1 ? 'border-l-2' : 'border-l-2 border-transparent'"
                :style="{ borderColor: idx < group.versions.length - 1 ? (s.is_default ? 'hsl(var(--primary))' : 'hsl(var(--border))') : 'transparent' }"
              >
                <!-- Timeline dot -->
                <div
                  class="absolute -left-[7px] top-1 size-3 rounded-full border-2 border-background"
                  :class="s.is_default ? 'bg-primary ring-2 ring-primary/20' : 'bg-muted-foreground/30'"
                />

                <div class="pl-4">
                  <div class="flex items-center gap-2">
                    <router-link :to="'/schemas/' + s.id" class="group inline-flex items-center gap-2 no-underline">
                      <Badge variant="secondary" class="font-mono text-xs transition-colors group-hover:bg-primary/10">
                        v{{ s.version }}
                      </Badge>
                      <Badge v-if="s.is_default" class="text-[10px]">default</Badge>
                      <Badge v-else variant="outline" class="text-[10px] border-yellow-300 bg-yellow-50 text-yellow-700">
                        draft
                      </Badge>
                    </router-link>
                  </div>

                  <p v-if="s.message" class="mt-1 text-sm italic text-muted-foreground">{{ s.message }}</p>

                  <div class="mt-2 flex items-center gap-3 text-xs text-muted-foreground">
                    <span v-if="s.created_by">by {{ s.created_by }}</span>
                    <span>{{ formatTime(s.created_at) }}</span>
                    <div class="ml-auto flex gap-1" @click.stop>
                      <Tooltip v-if="!s.is_default">
                        <TooltipTrigger asChild>
                          <Button
                            variant="ghost" size="sm"
                            class="h-7 w-7 p-0"
                            @click="promoteVersion(s)"
                            :disabled="promoting === s.id"
                          >
                            <Star class="size-3.5" :class="promoting === s.id ? 'animate-spin' : ''" />
                          </Button>
                        </TooltipTrigger>
                        <TooltipContent>Promote to default</TooltipContent>
                      </Tooltip>
                      <Tooltip v-if="group.versions.length > 1 && !s.is_default">
                        <TooltipTrigger asChild>
                          <Button
                            variant="ghost" size="sm"
                            class="h-7 w-7 p-0"
                            @click="showDiff(group.defaultVersion, s)"
                          >
                            <GitCompareArrows class="size-3.5" />
                          </Button>
                        </TooltipTrigger>
                        <TooltipContent>Diff against default</TooltipContent>
                      </Tooltip>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </CardContent>
        </CollapsibleContent>
      </Collapsible>
    </Card>

    <div v-if="!filteredGroups.length && searchQuery" class="flex h-24 items-center justify-center text-muted-foreground">
      No schemas matching "{{ searchQuery }}"
    </div>
    <div v-if="!schemaGroups.length && !searchQuery" class="flex h-24 items-center justify-center text-muted-foreground">
      No schemas found
    </div>

    <!-- Diff Sheet -->
    <Sheet :open="!!diffResult" @update:open="val => { if (!val) diffResult = null }">
      <SheetContent class="sm:max-w-xl overflow-y-auto">
        <SheetHeader>
          <SheetTitle class="font-mono text-sm">
            {{ diffResult?.left?.id }}
            <span class="text-muted-foreground mx-1">→</span>
            {{ diffResult?.right?.id }}
          </SheetTitle>
          <SheetDescription>Field-level changes between versions</SheetDescription>
        </SheetHeader>
        <div class="mt-4 space-y-0">
          <div v-if="!diffResult?.changes?.length" class="flex h-24 items-center justify-center text-muted-foreground text-sm">
            No field-level changes detected
          </div>
          <div v-for="c in diffResult?.changes" :key="c.field" class="border-b px-1 py-3 last:border-0">
            <div class="flex items-center gap-2">
              <span class="font-semibold text-sm">{{ c.field }}</span>
              <Badge
                :variant="c.action === 'added' ? 'default' : c.action === 'removed' ? 'destructive' : 'secondary'"
                class="text-[10px] uppercase"
              >{{ c.action }}</Badge>
            </div>
            <div v-if="c.action === 'modified'" class="mt-1.5 flex items-center gap-2 text-xs font-mono">
              <code class="rounded bg-red-50 px-1.5 py-0.5 text-red-800">{{ JSON.stringify(c.old?.['x-claim-mapping'] || c.old?.type || c.old, null, 0) }}</code>
              <span class="text-muted-foreground">→</span>
              <code class="rounded bg-emerald-50 px-1.5 py-0.5 text-emerald-800">{{ JSON.stringify(c.new?.['x-claim-mapping'] || c.new?.type || c.new, null, 0) }}</code>
            </div>
            <div v-else-if="c.action === 'added'" class="mt-1.5 text-xs font-mono">
              <code class="rounded bg-emerald-50 px-1.5 py-0.5 text-emerald-800">+ {{ JSON.stringify(c.new?.type || c.new, null, 0) }}</code>
            </div>
            <div v-else-if="c.action === 'removed'" class="mt-1.5 text-xs font-mono">
              <code class="rounded bg-red-50 px-1.5 py-0.5 text-red-800">- {{ JSON.stringify(c.old?.type || c.old, null, 0) }}</code>
            </div>
          </div>
        </div>
      </SheetContent>
    </Sheet>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, reactive } from 'vue'
import { schemaApi, type Schema } from '@/api/resources'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Search as SearchIcon, ChevronRight, Star, GitCompareArrows } from 'lucide-vue-next'

const allSchemas = ref<Schema[]>([])
const promoting = ref<string | null>(null)
const diffResult = ref<any>(null)
const searchQuery = ref('')

interface SchemaGroup {
  type: string
  versions: Schema[]
  defaultVersion: Schema
  open: boolean
}

const schemaGroups = computed<SchemaGroup[]>(() => {
  const groups = new Map<string, Schema[]>()
  for (const s of allSchemas.value) {
    if (!groups.has(s.type)) groups.set(s.type, [])
    groups.get(s.type)!.push(s)
  }
  return Array.from(groups.entries()).map(([type, versions]) => reactive({
    type,
    versions: versions.sort((a, b) => b.version - a.version),
    defaultVersion: versions.find(v => v.is_default) || versions[0],
    open: true,
  }))
})

const filteredGroups = computed(() => {
  if (!searchQuery.value) return schemaGroups.value
  const q = searchQuery.value.toLowerCase()
  return schemaGroups.value.filter(g => g.type.toLowerCase().includes(q))
})

onMounted(async () => {
  try { allSchemas.value = await schemaApi.list() } catch {}
})

function schemaFields(s: Schema): string[] {
  const props = (s.schema as any)?.properties
  return props ? Object.keys(props) : []
}

async function promoteVersion(s: Schema) {
  promoting.value = s.id
  try {
    await schemaApi.promote(s.id)
    allSchemas.value = await schemaApi.list()
  } catch {}
  promoting.value = null
}

async function showDiff(current: Schema, draft: Schema) {
  try {
    diffResult.value = await schemaApi.diff(current.id, draft.id)
  } catch {}
}

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>
