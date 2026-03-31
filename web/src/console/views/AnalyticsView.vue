<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Analytics</h1>
        <p class="text-sm text-muted-foreground">Query your audit events, users, and sessions with SQL.</p>
      </div>
    </div>

    <!-- SQL Editor -->
    <Card>
      <CardHeader class="flex flex-row items-center justify-between space-y-0 py-3 border-b bg-muted/20">
        <CardTitle class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">SQL Query</CardTitle>
        <div class="flex items-center gap-2">
          <Select v-model="selectedTemplate" @update:model-value="(val: any) => applyTemplate(String(val))">
            <SelectTrigger class="w-[180px] h-8 text-xs">
              <SelectValue placeholder="— Templates —" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="t in templates" :key="t.name" :value="t.sql">{{ t.name }}</SelectItem>
            </SelectContent>
          </Select>
          <Button size="sm" :disabled="running" @click="runQuery">
            {{ running ? '⏳ Running…' : '▶ Run Query' }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="p-0">
        <div class="monaco-wrap" :style="{ height: editorHeight + 'px' }">
          <vue-monaco-editor
            v-model:value="sql"
            language="sql"
            theme="vs"
            :options="editorOptions"
            @mount="onEditorMount"
          />
        </div>
      </CardContent>
      <div class="text-center cursor-ns-resize text-muted-foreground/40 hover:text-primary text-sm py-0.5 select-none bg-muted/20 border-t" @mousedown="startResize">⋯</div>
    </Card>

    <!-- Error -->
    <div v-if="error" class="p-4 rounded-lg border border-destructive/50 bg-destructive/10 text-destructive text-sm font-mono">{{ error }}</div>

    <!-- Stats bar -->
    <div v-if="result" class="flex items-center gap-4 p-4 rounded-lg border text-sm bg-card text-muted-foreground">
      <span class="font-medium text-foreground">{{ result.row_count }} rows</span>
      <span>·</span>
      <span>{{ result.execution_ms }}ms</span>
      <span>·</span>
      <span>{{ result.columns.length }} columns</span>
      <div class="flex-1" />
      <Button variant="outline" size="sm" :class="showChart ? 'bg-primary text-primary-foreground hover:bg-primary/90' : ''" @click="showChart = !showChart">
        📈 Chart
      </Button>
      <Button variant="outline" size="sm" @click="exportCSV">⬇ CSV</Button>
    </div>

    <!-- Chart -->
    <Card v-if="showChart && result && result.rows.length">
      <CardHeader class="pb-3">
        <div class="flex items-center gap-4 text-sm text-muted-foreground">
          <label class="flex items-center gap-1.5">
            X Axis:
            <Select v-model="chartXStr">
              <SelectTrigger class="w-[140px] h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="(col, i) in result.columns" :key="i" :value="String(i)">{{ col }}</SelectItem>
              </SelectContent>
            </Select>
          </label>
          <label class="flex items-center gap-1.5">
            Y Axis:
            <Select v-model="chartYStr">
              <SelectTrigger class="w-[140px] h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="(col, i) in result.columns" :key="i" :value="String(i)">{{ col }}</SelectItem>
              </SelectContent>
            </Select>
          </label>
          <label class="flex items-center gap-1.5">
            Type:
            <Select v-model="chartType">
              <SelectTrigger class="w-[100px] h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="bar">Bar</SelectItem>
                <SelectItem value="line">Line</SelectItem>
                <SelectItem value="pie">Pie</SelectItem>
              </SelectContent>
            </Select>
          </label>
        </div>
      </CardHeader>
      <CardContent>
        <canvas ref="chartCanvas"></canvas>
      </CardContent>
    </Card>

    <!-- Results table -->
    <Card v-if="result">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Query Results</CardTitle>
      </CardHeader>
      <CardContent class="p-0">
        <div class="overflow-auto max-h-[480px]">
          <Table>
            <TableHeader class="bg-muted/30 sticky top-0 z-10">
              <TableRow>
                <TableHead
                  v-for="(col, i) in result.columns"
                  :key="i"
                  class="cursor-pointer select-none hover:text-primary whitespace-nowrap"
                  @click="sortBy(i)"
                >
                  {{ col }}
                  <span v-if="sortCol === i" class="ml-1">{{ sortDir === 'asc' ? '↑' : '↓' }}</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="(row, ri) in sortedRows" :key="ri">
                <TableCell
                  v-for="(cell, ci) in row"
                  :key="ci"
                  class="max-w-[300px] truncate"
                  :class="isNumeric(cell) ? 'font-mono text-right text-primary' : ''"
                >
                  {{ formatCell(cell) }}
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>

    <!-- Available tables -->
    <div v-if="tables.length" class="space-y-3">
      <h3 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Available Tables</h3>
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        <Card
          v-for="t in tables" :key="t.name"
          class="cursor-pointer transition-colors hover:border-primary/50"
          @click="insertTable(t.name)"
        >
          <CardContent class="p-4">
            <div class="font-semibold font-mono text-sm">{{ t.name }}</div>
            <div class="text-xs text-muted-foreground mt-1">{{ t.row_count }} rows · {{ t.file_count }} files</div>
            <div v-if="t.columns" class="flex flex-wrap gap-1 mt-2">
              <Badge v-for="c in t.columns.slice(0, 6)" :key="c.name" variant="secondary" class="text-[10px] font-mono font-normal">
                {{ c.name }} <small class="text-muted-foreground/60 ml-0.5">{{ c.type }}</small>
              </Badge>
              <Badge v-if="t.columns.length > 6" variant="outline" class="text-[10px] font-mono text-primary">+{{ t.columns.length - 6 }}</Badge>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { VueMonacoEditor } from '@guolao/vue-monaco-editor'
import { api } from '@/api/client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

interface QueryResult {
  columns: string[]
  column_types: string[]
  rows: any[][]
  row_count: number
  execution_ms: number
  error?: string
}

interface TableInfo {
  name: string
  columns: { name: string; type: string }[]
  row_count: number
  file_count: number
  last_update: string
}

const sql = ref('SELECT event_type, COUNT(*) as cnt\nFROM events\nGROUP BY event_type\nORDER BY cnt DESC\nLIMIT 20')
const result = ref<QueryResult | null>(null)
const error = ref('')
const running = ref(false)
const tables = ref<TableInfo[]>([])
const showChart = ref(false)
const chartXStr = ref('0')
const chartYStr = ref('1')
const chartX = computed({ get: () => Number(chartXStr.value), set: v => { chartXStr.value = String(v) } })
const chartY = computed({ get: () => Number(chartYStr.value), set: v => { chartYStr.value = String(v) } })
const chartType = ref('bar')
const sortCol = ref(-1)
const sortDir = ref<'asc' | 'desc'>('desc')
const selectedTemplate = ref('')
const editorHeight = ref(180)
const chartCanvas = ref<HTMLCanvasElement | null>(null)
let editorInstance: any = null

const templates = [
  { name: 'Event counts by type', sql: "SELECT event_type, COUNT(*) as cnt\nFROM events\nGROUP BY event_type\nORDER BY cnt DESC\nLIMIT 20" },
  { name: 'Recent events', sql: "SELECT event_type, actor_id, identifier, created_at\nFROM events\nORDER BY event_id DESC\nLIMIT 50" },
  { name: 'Logins per hour', sql: "SELECT strftime(created_at, '%Y-%m-%d %H:00') as hour, COUNT(*) as logins\nFROM events\nWHERE event_type LIKE '%login%'\nGROUP BY hour\nORDER BY hour DESC\nLIMIT 24" },
  { name: 'User counts by type', sql: "SELECT user_type, COUNT(*) as cnt\nFROM users\nGROUP BY user_type\nORDER BY cnt DESC" },
  { name: 'Failed auth attempts', sql: "SELECT identifier, reason, ip_address, created_at\nFROM events\nWHERE event_type = 'auth.login_failed'\nORDER BY event_id DESC\nLIMIT 50" },
  { name: 'Active sessions', sql: "SELECT * FROM sessions LIMIT 50" },
]

const editorOptions = {
  minimap: { enabled: false },
  fontSize: 13,
  lineNumbers: 'on' as const,
  scrollBeyondLastLine: false,
  wordWrap: 'on' as const,
  tabSize: 2,
  automaticLayout: true,
  padding: { top: 8, bottom: 8 },
  scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
}

function onEditorMount(editor: any, monaco: any) {
  editorInstance = editor
  loadTablesIntoAutocomplete(monaco)
}

async function loadTablesIntoAutocomplete(monaco: any) {
  try {
    const schema = await api.get<any>('/v1/analytics/schema')

    monaco.languages.registerCompletionItemProvider('sql', {
      provideCompletionItems: (_model: any, position: any) => {
        const suggestions: any[] = []
        for (const [tableName, tableInfo] of Object.entries(schema) as any[]) {
          suggestions.push({
            label: tableName,
            kind: monaco.languages.CompletionItemKind.Class,
            insertText: tableName,
            detail: `Table (${tableInfo.row_count} rows)`,
          })
          if (tableInfo.columns) {
            for (const col of tableInfo.columns) {
              suggestions.push({
                label: col.name,
                kind: monaco.languages.CompletionItemKind.Field,
                insertText: col.name,
                detail: `${tableName}.${col.name} (${col.type})`,
              })
            }
          }
        }
        return { suggestions }
      },
    })
  } catch {}
}

async function runQuery() {
  running.value = true
  error.value = ''
  result.value = null

  try {
    const data = await api.post<any>('/v1/analytics/query', { sql: sql.value, limit: 1000 })
    if (data.error) {
      error.value = data.error
    } else {
      result.value = data
      if (data.columns.length >= 2) {
        chartXStr.value = '0'
        chartYStr.value = '1'
      }
    }
  } catch (e: any) {
    error.value = e?.message || 'Query failed'
  } finally {
    running.value = false
  }
}

function applyTemplate(val: string) {
  if (val) {
    sql.value = val
    selectedTemplate.value = ''
  }
}

function insertTable(name: string) {
  sql.value = `SELECT *\nFROM ${name}\nLIMIT 50`
}

function sortBy(col: number) {
  if (sortCol.value === col) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortCol.value = col
    sortDir.value = 'desc'
  }
}

const sortedRows = computed(() => {
  if (!result.value) return []
  const rows = [...result.value.rows]
  if (sortCol.value >= 0) {
    rows.sort((a, b) => {
      const va = a[sortCol.value]
      const vb = b[sortCol.value]
      const na = Number(va)
      const nb = Number(vb)
      if (!isNaN(na) && !isNaN(nb)) {
        return sortDir.value === 'asc' ? na - nb : nb - na
      }
      const sa = String(va || '')
      const sb = String(vb || '')
      return sortDir.value === 'asc' ? sa.localeCompare(sb) : sb.localeCompare(sa)
    })
  }
  return rows
})

function isNumeric(val: any): boolean {
  return typeof val === 'number' || (typeof val === 'string' && !isNaN(Number(val)) && val.trim() !== '')
}

function formatCell(val: any): string {
  if (val === null || val === undefined) return '—'
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}

function exportCSV() {
  if (!result.value) return
  const header = result.value.columns.join(',')
  const rows = result.value.rows.map(row =>
    row.map(cell => {
      const s = String(cell ?? '')
      return s.includes(',') || s.includes('"') ? `"${s.replace(/"/g, '""')}"` : s
    }).join(',')
  )
  const csv = [header, ...rows].join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'query_results.csv'
  a.click()
  URL.revokeObjectURL(url)
}

// Simple canvas chart rendering
watch([() => showChart.value, () => chartX.value, () => chartY.value, () => chartType.value, () => result.value], () => {
  if (showChart.value && result.value && chartCanvas.value) {
    nextTick(() => renderChart())
  }
})

function renderChart() {
  if (!chartCanvas.value || !result.value) return
  const canvas = chartCanvas.value
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const dpr = window.devicePixelRatio || 1
  const rect = canvas.parentElement!.getBoundingClientRect()
  canvas.width = rect.width * dpr
  canvas.height = 280 * dpr
  canvas.style.width = rect.width + 'px'
  canvas.style.height = '280px'
  ctx.scale(dpr, dpr)

  const w = rect.width
  const h = 280
  const padding = { top: 20, right: 20, bottom: 60, left: 60 }

  ctx.clearRect(0, 0, w, h)
  ctx.fillStyle = '#fafafa'
  ctx.fillRect(0, 0, w, h)

  const data = result.value.rows
  const labels = data.map(row => String(row[chartX.value] ?? ''))
  const values = data.map(row => Number(row[chartY.value]) || 0)
  const maxVal = Math.max(...values, 1)

  const chartW = w - padding.left - padding.right
  const chartH = h - padding.top - padding.bottom
  const colors = ['#6366f1', '#8b5cf6', '#a78bfa', '#c4b5fd', '#ddd6fe', '#ede9fe']

  if (chartType.value === 'bar' || chartType.value === 'line') {
    const barW = chartW / Math.max(values.length, 1)
    const gap = Math.max(2, barW * 0.15)

    // Grid lines
    ctx.strokeStyle = '#e5e7eb'
    ctx.lineWidth = 1
    for (let i = 0; i <= 4; i++) {
      const y = padding.top + (chartH / 4) * i
      ctx.beginPath() ; ctx.moveTo(padding.left, y) ; ctx.lineTo(w - padding.right, y) ; ctx.stroke()
      ctx.fillStyle = '#9ca3af'
      ctx.font = '11px system-ui'
      ctx.textAlign = 'right'
      ctx.fillText(String(Math.round(maxVal * (4 - i) / 4)), padding.left - 6, y + 4)
    }

    if (chartType.value === 'bar') {
      values.forEach((val, i) => {
        const barH = (val / maxVal) * chartH
        const x = padding.left + i * barW + gap / 2
        const y = padding.top + chartH - barH
        ctx.fillStyle = colors[i % colors.length]
        ctx.beginPath()
        ctx.roundRect(x, y, barW - gap, barH, 4)
        ctx.fill()
      })
    } else {
      // Line chart
      ctx.beginPath()
      ctx.strokeStyle = '#6366f1'
      ctx.lineWidth = 2
      values.forEach((val, i) => {
        const x = padding.left + i * barW + barW / 2
        const y = padding.top + chartH - (val / maxVal) * chartH
        if (i === 0) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
      })
      ctx.stroke()
      // Dots
      values.forEach((val, i) => {
        const x = padding.left + i * barW + barW / 2
        const y = padding.top + chartH - (val / maxVal) * chartH
        ctx.beginPath()
        ctx.arc(x, y, 4, 0, Math.PI * 2)
        ctx.fillStyle = '#6366f1'
        ctx.fill()
        ctx.strokeStyle = '#fff'
        ctx.lineWidth = 2
        ctx.stroke()
      })
    }

    // X labels
    ctx.fillStyle = '#6b7280'
    ctx.font = '10px system-ui'
    ctx.textAlign = 'center'
    labels.forEach((label, i) => {
      const x = padding.left + i * barW + barW / 2
      ctx.save()
      ctx.translate(x, h - padding.bottom + 12)
      ctx.rotate(-Math.PI / 6)
      ctx.fillText(label.substring(0, 20), 0, 0)
      ctx.restore()
    })
  } else if (chartType.value === 'pie') {
    const total = values.reduce((a, b) => a + b, 0)
    const cx = w / 2
    const cy = h / 2
    const radius = Math.min(chartW, chartH) / 2 - 10
    let startAngle = -Math.PI / 2

    values.forEach((val, i) => {
      const slice = (val / total) * Math.PI * 2
      ctx.beginPath()
      ctx.moveTo(cx, cy)
      ctx.arc(cx, cy, radius, startAngle, startAngle + slice)
      ctx.closePath()
      ctx.fillStyle = colors[i % colors.length]
      ctx.fill()
      // Label
      const mid = startAngle + slice / 2
      const lx = cx + Math.cos(mid) * radius * 0.65
      const ly = cy + Math.sin(mid) * radius * 0.65
      ctx.fillStyle = '#fff'
      ctx.font = 'bold 11px system-ui'
      ctx.textAlign = 'center'
      if (slice > 0.2) ctx.fillText(labels[i].substring(0, 12), lx, ly)
      startAngle += slice
    })
  }
}

let resizing = false
function startResize(e: MouseEvent) {
  resizing = true
  const startY = e.clientY
  const startH = editorHeight.value
  const onMove = (e: MouseEvent) => {
    if (!resizing) return
    editorHeight.value = Math.max(100, Math.min(600, startH + (e.clientY - startY)))
  }
  const onUp = () => { resizing = false; document.removeEventListener('mousemove', onMove); document.removeEventListener('mouseup', onUp) }
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}

onMounted(async () => {
  try {
    const data = await api.get<any>('/v1/analytics/tables')
    tables.value = data.tables || []
  } catch {}
})
</script>

<style scoped>
.monaco-wrap {
  border-bottom: 1px solid hsl(var(--border));
}
</style>
