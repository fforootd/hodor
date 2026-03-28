<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Query</h1>
        <p class="text-sm text-muted-foreground">Explore events with SQL queries.</p>
      </div>
      <div class="flex items-center gap-2">
        <Select v-model="timeRange">
          <SelectTrigger class="w-36">
            <SelectValue placeholder="Time range" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="1h">Last 1 hour</SelectItem>
            <SelectItem value="12h">Last 12 hours</SelectItem>
            <SelectItem value="24h">Last 24 hours</SelectItem>
            <SelectItem value="7d">Last 7 days</SelectItem>
            <SelectItem value="30d">Last 30 days</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <!-- Query Editor -->
    <Card>
      <CardHeader class="pb-3">
        <div class="flex items-center justify-between">
          <CardTitle class="text-sm">SQL Query</CardTitle>
          <div class="flex items-center gap-2">
            <Button size="sm" variant="outline" @click="formatQuery">
              <FileJson class="mr-1.5 size-3.5" />
              Format
            </Button>
            <Button size="sm" @click="runQuery" :disabled="running">
              <Play class="mr-1.5 size-3.5" />
              {{ running ? 'Running...' : 'Run Query' }}
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div class="rounded-md border bg-muted/30 overflow-hidden">
          <vue-monaco-editor
            v-model:value="query"
            language="sql"
            :options="editorOptions"
            theme="vs"
            class="h-32"
            @keydown.meta.enter="runQuery"
            @keydown.ctrl.enter="runQuery"
          />
        </div>
        <p v-if="queryError" class="mt-2 text-sm text-destructive">{{ queryError }}</p>
        <p v-if="queryTime" class="mt-2 text-xs text-muted-foreground">
          Query completed in {{ queryTime }}ms · {{ rows.length }} rows
        </p>
      </CardContent>
    </Card>

    <!-- Chart Panel -->
    <Card v-if="rows.length > 0 && chartData.length > 0">
      <CardHeader class="pb-3">
        <div class="flex items-center justify-between">
          <CardTitle class="text-sm">Results</CardTitle>
          <div class="flex items-center gap-1">
            <Button
              v-for="t in chartTypes"
              :key="t.value"
              size="sm"
              :variant="chartType === t.value ? 'default' : 'outline'"
              class="h-7 px-2"
              @click="chartType = t.value"
            >
              <component :is="t.icon" class="size-3.5" />
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div class="h-64 w-full flex items-end gap-1 px-2 pb-2 border rounded-md bg-muted/20">
          <template v-if="chartType === 'bar'">
            <div
              v-for="(d, i) in chartData"
              :key="i"
              class="flex-1 rounded-t transition-all duration-300 hover:opacity-80"
              :style="{ height: `${d.pct}%`, backgroundColor: colors[i % colors.length] }"
              :title="`${d.label}: ${d.value}`"
            />
          </template>
          <template v-else>
            <svg class="w-full h-full" :viewBox="`0 0 ${chartData.length * 40} 256`" preserveAspectRatio="none">
              <polyline
                :points="chartData.map((d, i) => `${i * 40 + 20},${256 - d.pct * 2.56}`).join(' ')"
                fill="none"
                stroke="hsl(12, 76%, 61%)"
                stroke-width="2"
              />
              <circle
                v-for="(d, i) in chartData"
                :key="i"
                :cx="i * 40 + 20"
                :cy="256 - d.pct * 2.56"
                r="3"
                fill="hsl(12, 76%, 61%)"
              />
            </svg>
          </template>
        </div>
      </CardContent>
    </Card>

    <!-- Results Table -->
    <Card v-if="rows.length > 0">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Data</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="flex items-center gap-2 mb-4">
          <Search class="size-4 text-muted-foreground" />
          <Input v-model="tableFilter" placeholder="Filter results..." class="max-w-sm" />
        </div>
        <div class="rounded-md border overflow-auto max-h-96">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead v-for="col in columns" :key="col">{{ col }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="(row, ri) in filteredRows" :key="ri">
                <TableCell v-for="col in columns" :key="col" class="font-mono text-xs">
                  {{ row[col] }}
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>

    <!-- Empty state -->
    <Card v-if="!rows.length && !running && hasRun" class="py-12">
      <CardContent class="text-center text-muted-foreground">
        <Database class="mx-auto size-10 mb-3 opacity-40" />
        <p class="font-medium">No results</p>
        <p class="text-sm mt-1">Try modifying your query or time range.</p>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Play, FileJson, BarChart3, TrendingUp, Search, Database } from 'lucide-vue-next'
import { api } from '@/api/client'

const timeRange = ref('12h')
const chartType = ref<'bar' | 'line'>('bar')
const chartTypes = [
  { value: 'bar' as const, icon: BarChart3 },
  { value: 'line' as const, icon: TrendingUp },
]
const colors = ['hsl(12, 76%, 61%)', 'hsl(173, 58%, 39%)', 'hsl(197, 37%, 24%)', 'hsl(43, 74%, 66%)', 'hsl(27, 87%, 67%)']

const query = ref(`SELECT event_type, COUNT(*) as count\nFROM events\nGROUP BY event_type\nORDER BY count DESC\nLIMIT 20`)
const running = ref(false)
const hasRun = ref(false)
const queryError = ref('')
const queryTime = ref(0)
const tableFilter = ref('')

const rows = ref<Record<string, any>[]>([])
const columns = ref<string[]>([])

const editorOptions = {
  minimap: { enabled: false },
  lineNumbers: 'off' as const,
  scrollBeyondLastLine: false,
  fontSize: 13,
  fontFamily: 'JetBrains Mono, monospace',
  padding: { top: 12, bottom: 12 },
  wordWrap: 'on' as const,
  renderLineHighlight: 'none' as const,
  overviewRulerLanes: 0,
  hideCursorInOverviewRuler: true,
  overviewRulerBorder: false,
  scrollbar: { vertical: 'hidden' as const, horizontal: 'hidden' as const },
}

const chartData = computed(() => {
  if (!rows.value.length || !columns.value.length) return []
  // Try to find a numeric column for values and a text column for labels
  const numCol = columns.value.find(c => typeof rows.value[0][c] === 'number') || columns.value[columns.value.length - 1]
  const labelCol = columns.value.find(c => c !== numCol) || columns.value[0]
  const vals = rows.value.map(r => ({ label: String(r[labelCol] || ''), value: Number(r[numCol] || 0) }))
  const max = Math.max(...vals.map(v => v.value), 1)
  return vals.map(v => ({ ...v, pct: (v.value / max) * 100 }))
})

const filteredRows = computed(() => {
  const f = tableFilter.value.toLowerCase()
  if (!f) return rows.value
  return rows.value.filter(r => Object.values(r).some(v => String(v).toLowerCase().includes(f)))
})

function formatQuery() {
  // Basic SQL formatting
  query.value = query.value
    .replace(/\b(SELECT|FROM|WHERE|GROUP BY|ORDER BY|LIMIT|HAVING|JOIN|LEFT|RIGHT|INNER|ON|AND|OR|AS|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|SET|VALUES|INTO|DISTINCT|UNION|EXCEPT|INTERSECT)\b/gi, m => '\n' + m.toUpperCase())
    .replace(/^\n/, '')
    .replace(/\n\n+/g, '\n')
}

async function runQuery() {
  running.value = true
  queryError.value = ''
  queryTime.value = 0
  hasRun.value = true

  const start = performance.now()
  try {
    const data = await api.post<any>('/v1/analytics/query', { query: query.value })
    queryTime.value = Math.round(performance.now() - start)

    if (data.error) {
      queryError.value = data.error
      rows.value = []
      columns.value = []
      return
    }

    const resultRows = data.rows || []
    if (resultRows.length > 0) {
      columns.value = Object.keys(resultRows[0])
      // Coerce numeric values
      rows.value = resultRows.map((r: any) => {
        const out: Record<string, any> = {}
        for (const [k, v] of Object.entries(r)) {
          out[k] = isNaN(Number(v)) || v === '' || v === null ? v : Number(v)
        }
        return out
      })
    } else {
      columns.value = data.columns || []
      rows.value = []
    }
  } catch (err: any) {
    queryError.value = err.message || 'Query failed'
    queryTime.value = Math.round(performance.now() - start)
    rows.value = []
    columns.value = []
  } finally {
    running.value = false
  }
}
</script>
