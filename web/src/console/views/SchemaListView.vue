<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold tracking-tight">Schemas</h1>
      <p class="text-muted-foreground">Schema types with version history.</p>
    </div>

    <Card v-for="group in schemaGroups" :key="group.type">
      <CardHeader class="flex-row items-center justify-between space-y-0">
        <CardTitle>{{ group.type }}</CardTitle>
        <span class="text-xs text-muted-foreground">
          {{ group.versions.length }} version{{ group.versions.length !== 1 ? 's' : '' }}
        </span>
      </CardHeader>
      <CardContent>
        <!-- Version timeline -->
        <div class="relative pl-6">
          <div
            v-for="(s, idx) in group.versions" :key="s.id"
            class="relative pb-4 last:pb-0"
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

              <div class="mt-2 flex flex-wrap gap-1">
                <Badge v-for="field in schemaFields(s)" :key="field" variant="outline" class="text-[10px] font-normal">
                  {{ field }}
                </Badge>
              </div>

              <div class="mt-2 flex items-center gap-3 text-xs text-muted-foreground">
                <span v-if="s.created_by">by {{ s.created_by }}</span>
                <span>{{ formatTime(s.created_at) }}</span>
                <div class="ml-auto flex gap-2" @click.stop>
                  <Button
                    v-if="!s.is_default"
                    variant="outline" size="sm"
                    class="h-7 text-xs"
                    @click="promoteVersion(s)"
                    :disabled="promoting === s.id"
                  >
                    {{ promoting === s.id ? 'Promoting…' : '★ Promote' }}
                  </Button>
                  <Button
                    v-if="group.versions.length > 1 && !s.is_default"
                    variant="ghost" size="sm"
                    class="h-7 text-xs"
                    @click="showDiff(group.defaultVersion, s)"
                  >
                    Diff
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Diff Dialog -->
    <Dialog :open="!!diffResult" @update:open="diffResult = null">
      <DialogContent class="max-w-2xl max-h-[80vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle class="font-mono text-sm">
            {{ diffResult?.left?.id }}
            <span class="text-muted-foreground mx-1">→</span>
            {{ diffResult?.right?.id }}
          </DialogTitle>
        </DialogHeader>
        <div class="flex-1 overflow-y-auto py-2">
          <div v-if="!diffResult?.changes?.length" class="flex h-24 items-center justify-center text-muted-foreground">
            No field-level changes detected
          </div>
          <div v-for="c in diffResult?.changes" :key="c.field" class="border-b px-4 py-3 last:border-0">
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
              <code class="rounded bg-green-50 px-1.5 py-0.5 text-green-800">{{ JSON.stringify(c.new?.['x-claim-mapping'] || c.new?.type || c.new, null, 0) }}</code>
            </div>
            <div v-else-if="c.action === 'added'" class="mt-1.5 text-xs font-mono">
              <code class="rounded bg-green-50 px-1.5 py-0.5 text-green-800">+ {{ JSON.stringify(c.new?.type || c.new, null, 0) }}</code>
            </div>
            <div v-else-if="c.action === 'removed'" class="mt-1.5 text-xs font-mono">
              <code class="rounded bg-red-50 px-1.5 py-0.5 text-red-800">- {{ JSON.stringify(c.old?.type || c.old, null, 0) }}</code>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>

    <div v-if="!schemaGroups.length" class="flex h-24 items-center justify-center text-muted-foreground">
      No schemas found
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { schemaApi, type Schema } from '@/api/resources'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'

const allSchemas = ref<Schema[]>([])
const promoting = ref<string | null>(null)
const diffResult = ref<any>(null)

interface SchemaGroup {
  type: string
  versions: Schema[]
  defaultVersion: Schema
}

const schemaGroups = computed<SchemaGroup[]>(() => {
  const groups = new Map<string, Schema[]>()
  for (const s of allSchemas.value) {
    if (!groups.has(s.type)) groups.set(s.type, [])
    groups.get(s.type)!.push(s)
  }
  return Array.from(groups.entries()).map(([type, versions]) => ({
    type,
    versions: versions.sort((a, b) => b.version - a.version),
    defaultVersion: versions.find(v => v.is_default) || versions[0],
  }))
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
