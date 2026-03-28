<template>
  <div class="analytics-page">
    <div class="analytics-header">
      <h2>📊 Analytics</h2>
      <p class="subtitle">Query your audit events, entities, and sessions with SQL</p>
    </div>

    <!-- SQL Editor -->
    <div class="editor-section">
      <div class="editor-toolbar">
        <span class="editor-label">SQL Query</span>
        <div class="toolbar-right">
          <select v-model="selectedTemplate" @change="applyTemplate" class="template-select">
            <option value="">— Templates —</option>
            <option v-for="t in templates" :key="t.name" :value="t.sql">{{ t.name }}</option>
          </select>
          <button class="btn-run" @click="runQuery" :disabled="running">
            {{ running ? '⏳ Running…' : '▶ Run Query' }}
          </button>
        </div>
      </div>
      <div class="monaco-wrap" :style="{ height: editorHeight + 'px' }">
        <vue-monaco-editor
          v-model:value="sql"
          language="sql"
          theme="vs"
          :options="editorOptions"
          @mount="onEditorMount"
        />
      </div>
      <div class="editor-resize" @mousedown="startResize">⋯</div>
    </div>

    <!-- Error -->
    <div v-if="error" class="error-banner">{{ error }}</div>

    <!-- Stats bar -->
    <div v-if="result" class="stats-bar">
      <span>{{ result.row_count }} rows</span>
      <span>·</span>
      <span>{{ result.execution_ms }}ms</span>
      <span>·</span>
      <span>{{ result.columns.length }} columns</span>
      <span class="spacer" />
      <button class="btn-chart" :class="{ active: showChart }" @click="showChart = !showChart">
        📈 Chart
      </button>
      <button class="btn-export" @click="exportCSV">⬇ CSV</button>
    </div>

    <!-- Chart (ECharts placeholder — renders bar chart from first two columns) -->
    <div v-if="showChart && result && result.rows.length" class="chart-section">
      <div class="chart-config">
        <label>X Axis:
          <select v-model="chartX">
            <option v-for="(col, i) in result.columns" :key="i" :value="i">{{ col }}</option>
          </select>
        </label>
        <label>Y Axis:
          <select v-model="chartY">
            <option v-for="(col, i) in result.columns" :key="i" :value="i">{{ col }}</option>
          </select>
        </label>
        <label>Type:
          <select v-model="chartType">
            <option value="bar">Bar</option>
            <option value="line">Line</option>
            <option value="pie">Pie</option>
          </select>
        </label>
      </div>
      <div class="chart-container">
        <canvas ref="chartCanvas"></canvas>
      </div>
    </div>

    <!-- Results table -->
    <div v-if="result" class="results-section">
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th v-for="(col, i) in result.columns" :key="i" @click="sortBy(i)" class="sortable">
                {{ col }}
                <span v-if="sortCol === i" class="sort-indicator">{{ sortDir === 'asc' ? '↑' : '↓' }}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, ri) in sortedRows" :key="ri">
              <td v-for="(cell, ci) in row" :key="ci" :class="{ num: isNumeric(cell) }">
                {{ formatCell(cell) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Available tables -->
    <div v-if="tables.length" class="tables-section">
      <h3>Available Tables</h3>
      <div class="table-cards">
        <div v-for="t in tables" :key="t.name" class="table-card" @click="insertTable(t.name)">
          <div class="table-card-name">{{ t.name }}</div>
          <div class="table-card-meta">{{ t.row_count }} rows · {{ t.file_count }} files</div>
          <div class="table-card-cols" v-if="t.columns">
            <span v-for="c in t.columns.slice(0, 6)" :key="c.name" class="col-chip">
              {{ c.name }} <small>{{ c.type }}</small>
            </span>
            <span v-if="t.columns.length > 6" class="col-chip more">+{{ t.columns.length - 6 }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { VueMonacoEditor } from '@guolao/vue-monaco-editor'
import { api } from '@/api/client'

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
const chartX = ref(0)
const chartY = ref(1)
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
  { name: 'Entity counts by type', sql: "SELECT schema_type, COUNT(*) as cnt\nFROM entities\nGROUP BY schema_type\nORDER BY cnt DESC" },
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
  // Register SQL completion provider with table/column names
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
        chartX.value = 0
        chartY.value = 1
      }
    }
  } catch (e: any) {
    error.value = e?.message || 'Query failed'
  } finally {
    running.value = false
  }
}

function applyTemplate() {
  if (selectedTemplate.value) {
    sql.value = selectedTemplate.value
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

// Simple canvas chart rendering (no ECharts dependency for POC)
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
.analytics-page { max-width: 1200px; }
.analytics-header { margin-bottom: 1.25rem; }
.analytics-header h2 { font-size: 1.25rem; font-weight: 700; color: #1a1a2e; }
.subtitle { font-size: 0.8125rem; color: #6b7280; margin-top: 0.25rem; }

.editor-section {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden;
  margin-bottom: 1rem;
}
.editor-toolbar {
  display: flex; justify-content: space-between; align-items: center;
  padding: 0.5rem 0.75rem; background: #f8f9fa; border-bottom: 1px solid #e5e7eb;
}
.editor-label { font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; }
.toolbar-right { display: flex; gap: 0.5rem; align-items: center; }
.template-select {
  padding: 0.25rem 0.5rem; border: 1px solid #d1d5db; border-radius: 6px;
  font-size: 0.75rem; color: #6b7280; background: #fff;
}
.btn-run {
  padding: 0.375rem 1rem; border: none; border-radius: 8px; background: #6366f1;
  color: #fff; font-size: 0.8125rem; font-weight: 600; cursor: pointer; transition: opacity 0.15s;
}
.btn-run:hover { opacity: 0.9; }
.btn-run:disabled { opacity: 0.5; cursor: not-allowed; }

.monaco-wrap { border-bottom: 1px solid #e5e7eb; }
.editor-resize {
  text-align: center; cursor: ns-resize; color: #d1d5db; font-size: 1rem;
  padding: 2px 0; user-select: none; background: #f8f9fa;
}
.editor-resize:hover { color: #6366f1; }

.error-banner {
  padding: 0.75rem 1rem; background: #fef2f2; border: 1px solid #fecaca; border-radius: 8px;
  color: #dc2626; font-size: 0.8125rem; margin-bottom: 1rem; font-family: monospace;
}

.stats-bar {
  display: flex; align-items: center; gap: 0.5rem;
  padding: 0.5rem 0.75rem; background: #f3f4f6; border-radius: 8px; margin-bottom: 1rem;
  font-size: 0.75rem; color: #6b7280;
}
.spacer { flex: 1; }
.btn-chart, .btn-export {
  padding: 0.25rem 0.625rem; border: 1px solid #d1d5db; border-radius: 6px;
  background: #fff; color: #6b7280; font-size: 0.6875rem; cursor: pointer; transition: all 0.15s;
}
.btn-chart:hover, .btn-export:hover { border-color: #6366f1; color: #6366f1; }
.btn-chart.active { background: #6366f1; color: #fff; border-color: #6366f1; }

.chart-section {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1rem;
  margin-bottom: 1rem;
}
.chart-config {
  display: flex; gap: 1rem; margin-bottom: 0.75rem; font-size: 0.75rem; color: #6b7280;
}
.chart-config select { margin-left: 0.25rem; padding: 0.125rem 0.375rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.75rem; }
.chart-container { width: 100%; }

.results-section {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden;
  margin-bottom: 1rem;
}
.table-wrap { overflow-x: auto; max-height: 480px; overflow-y: auto; }
table { width: 100%; border-collapse: collapse; font-size: 0.8125rem; }
thead { position: sticky; top: 0; z-index: 1; }
th {
  background: #f8f9fa; padding: 0.5rem 0.75rem; text-align: left; font-weight: 600;
  color: #4b5563; border-bottom: 2px solid #e5e7eb; white-space: nowrap;
}
th.sortable { cursor: pointer; user-select: none; }
th.sortable:hover { color: #6366f1; }
.sort-indicator { margin-left: 0.25rem; }
td {
  padding: 0.375rem 0.75rem; border-bottom: 1px solid #f3f4f6; color: #1a1a2e;
  max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
td.num { font-family: 'SF Mono', monospace; text-align: right; color: #6366f1; }
tr:hover td { background: #fafafe; }

.tables-section { margin-bottom: 2rem; }
.tables-section h3 { font-size: 0.8125rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 0.75rem; }
.table-cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 0.75rem; }
.table-card {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1rem;
  cursor: pointer; transition: all 0.15s;
}
.table-card:hover { border-color: #a5b4fc; box-shadow: 0 2px 8px rgba(99,102,241,.1); }
.table-card-name { font-size: 0.9375rem; font-weight: 700; color: #1a1a2e; font-family: 'SF Mono', monospace; }
.table-card-meta { font-size: 0.75rem; color: #9ca3af; margin-top: 0.25rem; }
.table-card-cols { display: flex; flex-wrap: wrap; gap: 0.25rem; margin-top: 0.5rem; }
.col-chip {
  font-size: 0.6875rem; padding: 0.125rem 0.375rem; background: #f3f4f6; border-radius: 4px;
  color: #6b7280; font-family: 'SF Mono', monospace;
}
.col-chip small { color: #9ca3af; margin-left: 0.125rem; }
.col-chip.more { background: #eff6ff; color: #6366f1; }
</style>
