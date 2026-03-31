<template>
  <Sheet :open="open" @update:open="$emit('update:open', $event)">
    <SheetContent class="sm:max-w-2xl overflow-y-auto">
      <SheetHeader>
        <SheetTitle class="flex items-center gap-2">
          <AlertTriangle class="size-5 text-amber-500" />
          Schema Upgrade Impact
        </SheetTitle>
        <SheetDescription>
          Preview how this schema change would affect
          <span class="font-semibold">{{ report?.total_entities || 0 }}</span>
          existing {{ schemaType }} entities.
        </SheetDescription>
      </SheetHeader>

      <!-- Loading -->
      <div v-if="loading" class="flex h-40 items-center justify-center">
        <Spinner class="mr-2" /> Analyzing impact…
      </div>

      <!-- Error -->
      <div v-else-if="error" class="mt-4 rounded-md border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive">
        {{ error }}
      </div>

      <!-- Report -->
      <div v-else-if="report" class="mt-4 space-y-5">

        <!-- Impact summary cards -->
        <div class="grid grid-cols-3 gap-3">
          <div class="rounded-lg border bg-emerald-50 p-3 text-center">
            <div class="text-2xl font-bold text-emerald-700">{{ report.impact.valid }}</div>
            <div class="text-xs text-emerald-600 font-medium">Valid</div>
          </div>
          <div class="rounded-lg border bg-amber-50 p-3 text-center">
            <div class="text-2xl font-bold text-amber-700">{{ report.impact.warnings }}</div>
            <div class="text-xs text-amber-600 font-medium">Warnings</div>
          </div>
          <div class="rounded-lg border bg-red-50 p-3 text-center">
            <div class="text-2xl font-bold text-red-700">{{ report.impact.breaking }}</div>
            <div class="text-xs text-red-600 font-medium">Breaking</div>
          </div>
        </div>

        <!-- Blast radius -->
        <div v-if="report.total_entities > report.sampled" class="flex items-center gap-2 rounded-md border bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
          <Info class="size-3.5 shrink-0" />
          Sampled {{ report.sampled }} of {{ report.total_entities }} entities.
          Estimated blast radius: {{ blastRadius }}% affected.
        </div>

        <!-- Field changes table -->
        <div v-if="report.field_changes?.length" class="space-y-2">
          <h4 class="text-sm font-semibold">Schema Changes</h4>
          <div class="rounded-md border overflow-hidden">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b bg-muted/30">
                  <th class="px-3 py-2 text-left font-medium text-xs">Field</th>
                  <th class="px-3 py-2 text-left font-medium text-xs">Change</th>
                  <th class="px-3 py-2 text-left font-medium text-xs">Severity</th>
                  <th class="px-3 py-2 text-right font-medium text-xs">Affected</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="fc in report.field_changes" :key="fc.path" class="border-b last:border-0">
                  <td class="px-3 py-2 font-mono text-xs">{{ fc.path.replace('properties.', '') }}</td>
                  <td class="px-3 py-2">
                    <Badge variant="outline" class="text-[10px] font-normal">
                      {{ formatChange(fc.change) }}
                    </Badge>
                  </td>
                  <td class="px-3 py-2">
                    <Badge :class="severityClass(fc.severity)" class="text-[10px]">
                      {{ fc.severity }}
                    </Badge>
                  </td>
                  <td class="px-3 py-2 text-right text-xs tabular-nums text-muted-foreground">
                    {{ fc.affected_estimate !== undefined ? '~' + fc.affected_estimate : '—' }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <!-- Entity samples -->
        <div v-if="report.sample_entities?.length" class="space-y-2">
          <h4 class="text-sm font-semibold">Sampled Entities</h4>
          <div class="space-y-2">
            <Collapsible v-for="entity in report.sample_entities" :key="entity.id">
              <CollapsibleTrigger class="flex w-full items-center gap-2 rounded-md border px-3 py-2 text-sm cursor-pointer transition-colors hover:bg-muted/30">
                <Badge :class="statusClass(entity.status)" class="text-[10px]">
                  {{ entity.status }}
                </Badge>
                <span class="font-medium truncate flex-1 text-left">{{ entity.display_name || entity.id }}</span>
                <ChevronRight class="size-3.5 text-muted-foreground transition-transform data-[state=open]:rotate-90" />
              </CollapsibleTrigger>
              <CollapsibleContent>
                <div class="ml-4 mt-1 mb-2 space-y-1.5">
                  <div v-if="!entity.changes?.length" class="text-xs text-muted-foreground py-1">
                    No issues detected
                  </div>
                  <div
v-for="(c, i) in entity.changes" :key="i"
                    class="flex items-start gap-2 rounded-sm border-l-2 px-2 py-1.5 text-xs"
                    :class="{
                      'border-l-red-400 bg-red-50/50': c.issue.includes('required') || c.issue.includes('missing'),
                      'border-l-amber-400 bg-amber-50/50': c.issue.includes('mismatch'),
                      'border-l-blue-400 bg-blue-50/50': !c.issue.includes('required') && !c.issue.includes('mismatch'),
                    }"
                  >
                    <div class="flex-1">
                      <span class="font-mono font-medium">{{ c.path }}</span>
                      <span class="text-muted-foreground ml-1">{{ c.issue }}</span>
                      <div v-if="c.current_value !== undefined" class="mt-0.5 text-muted-foreground">
                        Current: <code class="rounded bg-muted px-1">{{ JSON.stringify(c.current_value) }}</code>
                      </div>
                      <div v-if="c.suggestion" class="mt-0.5 text-emerald-700">
                        💡 {{ c.suggestion }}
                      </div>
                    </div>
                  </div>
                </div>
              </CollapsibleContent>
            </Collapsible>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div v-if="report && !loading" class="mt-6 flex items-center gap-2 justify-end border-t pt-4">
        <Button variant="outline" @click="$emit('update:open', false)">
          Cancel
        </Button>
        <Button :variant="hasBreaking ? 'destructive' : 'default'" @click="$emit('confirm')">
          {{ hasBreaking ? 'Save Anyway' : 'Confirm Save' }}
        </Button>
      </div>
    </SheetContent>
  </Sheet>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { schemaApi, type UpgradeReport } from '@/api/resources'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { AlertTriangle, ChevronRight, Info } from 'lucide-vue-next'

const props = defineProps<{
  open: boolean
  schemaType: string
  proposedSchema: Record<string, any> | null
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  confirm: []
}>()

const report = ref<UpgradeReport | null>(null)
const loading = ref(false)
const error = ref('')

const hasBreaking = computed(() => (report.value?.impact?.breaking || 0) > 0)
const blastRadius = computed(() => {
  if (!report.value || report.value.sampled === 0) return 0
  const broken = report.value.impact.breaking + report.value.impact.warnings
  const rate = broken / report.value.sampled
  return Math.round(rate * 100)
})

watch(() => props.open, async (isOpen) => {
  if (!isOpen || !props.proposedSchema || !props.schemaType) return
  loading.value = true
  error.value = ''
  report.value = null

  try {
    report.value = await schemaApi.previewUpgrade(props.schemaType, props.proposedSchema, 10)
  } catch (e: any) {
    error.value = e.message || 'Failed to analyze impact'
  } finally {
    loading.value = false
  }
}, { immediate: true })

function formatChange(change: string): string {
  return change.replace(/_/g, ' ')
}

function severityClass(severity: string): string {
  switch (severity) {
    case 'breaking': return 'bg-red-100 text-red-800 border-red-200'
    case 'warning': return 'bg-amber-100 text-amber-800 border-amber-200'
    default: return 'bg-blue-100 text-blue-800 border-blue-200'
  }
}

function statusClass(status: string): string {
  switch (status) {
    case 'valid': return 'bg-emerald-100 text-emerald-800 border-emerald-200'
    case 'warning': return 'bg-amber-100 text-amber-800 border-amber-200'
    case 'breaking': return 'bg-red-100 text-red-800 border-red-200'
    default: return ''
  }
}
</script>
