<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Explore</h1>
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

    <!-- Main Explorer -->
    <Tabs v-model="activeMode" class="w-full">
      <div class="flex items-center justify-between mb-2">
        <TabsList>
          <TabsTrigger value="visual">Visual Builder</TabsTrigger>
          <TabsTrigger value="sql">SQL Editor</TabsTrigger>
        </TabsList>
        <div class="flex items-center gap-2">
          <!-- Saved Queries dropdown -->
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button size="sm" variant="outline">
                <BookmarkIcon class="mr-1.5 size-3.5" />
                Saved
                <Badge v-if="savedQueries.length" variant="secondary" class="ml-1.5 text-[10px] px-1.5 py-0 h-4">{{ savedQueries.length }}</Badge>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-72">
              <div v-if="!savedQueries.length" class="px-3 py-4 text-center text-xs text-muted-foreground">
                No saved queries yet
              </div>
              <div v-for="sq in savedQueries" :key="sq.id" class="flex items-center justify-between px-2 py-1.5 hover:bg-muted rounded-sm transition-colors group">
                <button class="flex-1 text-left text-sm truncate" @click="loadSavedQuery(sq)">
                  <span class="font-medium">{{ sq.name }}</span>
                  <span class="text-xs text-muted-foreground block truncate">{{ sq.description || sq.sql.slice(0, 60) }}</span>
                </button>
                <button class="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive p-1 transition-opacity" @click.stop="requestDeleteSavedQuery(sq)">
                  <Trash2 class="size-3.5" />
                </button>
              </div>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button size="sm" variant="outline" @click="saveCurrentQuery" :disabled="!query">
            <Save class="mr-1.5 size-3.5" />
            Save
          </Button>
          <Button size="sm" variant="outline" @click="formatQuery" v-if="activeMode === 'sql'">
            <FileJson class="mr-1.5 size-3.5" />
            Format
          </Button>
          <Button size="sm" @click="runQuery" :disabled="running">
            <Play class="mr-1.5 size-3.5" />
            {{ running ? 'Running...' : 'Run Query' }}
          </Button>
        </div>
      </div>

      <TabsContent value="visual" class="mt-0">
        <Card>
          <CardContent class="pt-6 space-y-4">
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
              <!-- Table -->
              <div class="space-y-1.5">
                <Label>Table</Label>
                <Select v-model="visTable">
                  <SelectTrigger><SelectValue placeholder="Select table..."/></SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="t in Object.keys(schemas)" :key="t" :value="t">{{ t }}</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <!-- Metric -->
              <div class="space-y-1.5 col-span-2">
                <Label>Metric</Label>
                <div class="flex gap-2">
                  <Select v-model="visMetricFunc">
                    <SelectTrigger class="w-[120px]"><SelectValue/></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="NONE">SELECT ROWS</SelectItem>
                      <SelectItem value="COUNT">COUNT</SelectItem>
                      <SelectItem value="SUM">SUM</SelectItem>
                      <SelectItem value="AVG">AVG</SelectItem>
                      <SelectItem value="MIN">MIN</SelectItem>
                      <SelectItem value="MAX">MAX</SelectItem>
                    </SelectContent>
                  </Select>
                  <Select v-model="visMetricCol">
                    <SelectTrigger class="flex-1"><SelectValue/></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="*">*</SelectItem>
                      <SelectItem v-for="c in currentColumns" :key="c.name" :value="c.name">{{ c.name }}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <!-- Group By -->
              <div class="space-y-2 border rounded-md p-3 bg-muted/10">
                <Label>Group by (Dimension)</Label>
                <Select v-model="visGroup">
                  <SelectTrigger><SelectValue placeholder="None" /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="NONE">None</SelectItem>
                    <SelectItem v-for="c in currentColumns" :key="c.name" :value="c.name">{{ c.name }}</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <!-- Filters -->
              <div class="space-y-2 border rounded-md p-3 bg-muted/10">
                <div class="flex justify-between items-center">
                  <Label>Filters</Label>
                  <Button variant="ghost" size="sm" class="h-6" @click="addFilter">+ Add</Button>
                </div>
                <div v-for="(f, i) in visFilters" :key="i" class="flex gap-2 items-center">
                  <Select v-model="f.col">
                    <SelectTrigger class="w-1/3"><SelectValue/></SelectTrigger>
                    <SelectContent>
                      <SelectItem v-for="c in currentColumns" :key="c.name" :value="c.name">{{ c.name }}</SelectItem>
                    </SelectContent>
                  </Select>
                  <Select v-model="f.op">
                    <SelectTrigger class="w-[80px]"><SelectValue/></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="=">=</SelectItem>
                      <SelectItem value="!=">!=</SelectItem>
                      <SelectItem value=">">></SelectItem>
                      <SelectItem value="<">&lt;</SelectItem>
                      <SelectItem value="LIKE">LIKE</SelectItem>
                    </SelectContent>
                  </Select>
                  <Input v-model="f.val" placeholder="Value" class="flex-1 h-9" />
                  <Button variant="ghost" size="icon" class="h-9 w-9 text-muted-foreground hover:text-destructive shrink-0" @click="removeFilter(i)">✕</Button>
                </div>
                <p v-if="visFilters.length === 0" class="text-xs text-muted-foreground py-2 text-center">No active filters.</p>
              </div>
            </div>
            
            <div class="pt-2" v-if="queryError">
              <p class="text-sm text-destructive">{{ queryError }}</p>
            </div>
          </CardContent>
        </Card>
      </TabsContent>

      <TabsContent value="sql" class="mt-0 data-[state=inactive]:hidden" force-mount>
        <Card>
          <CardHeader class="pb-3 border-b bg-muted/20">
            <CardTitle class="text-xs font-mono font-normal flex items-center gap-2">
              <span class="text-amber-600">⚠</span> Edits made here will NOT sync back to the Visual Builder. Switching back overwrites this view.
            </CardTitle>
          </CardHeader>
          <CardContent class="p-0 border-b">
            <div class="overflow-hidden" style="height: 320px;">
              <vue-monaco-editor
                v-model:value="query"
                language="sql"
                :options="editorOptions"
                theme="vs"
                @keydown.meta.enter="runQuery"
                @keydown.ctrl.enter="runQuery"
              />
            </div>
          </CardContent>
          <CardContent class="py-3 bg-muted/20">
            <p v-if="queryError" class="text-sm text-destructive">{{ queryError }}</p>
            <p v-if="queryTime" class="text-xs text-muted-foreground">
              Query completed in {{ queryTime }}ms · {{ rows.length }} rows
            </p>
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>

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
        <ChartContainer :config="chartConfig" class="h-64 w-full">
          <VisXYContainer :data="chartData" :margin="{top: 20, right: 20, bottom: 20, left: 40}">
            <template v-if="chartType === 'bar'">
              <VisStackedBar :x="(d: any, i: number) => i" :y="[(d: any) => d.value]" color="hsl(12, 76%, 61%)" :roundedCorners="4" />
            </template>
            <template v-else>
              <VisLine :x="(d: any, i: number) => i" :y="[(d: any) => d.value]" color="hsl(12, 76%, 61%)" :lineWidth="2" />
              <VisArea :x="(d: any, i: number) => i" :y="[(d: any) => d.value]" color="hsl(12, 76%, 61%)" :opacity="0.1" />
            </template>
            <VisAxis type="x" :tickFormat="(i: number) => chartData[i]?.label || ''" :gridLine="false" />
            <VisAxis type="y" :tickLine="false" :domainLine="false" />
            <ChartCrosshair :customTooltip="(d: any) => `<div class='bg-background text-foreground border rounded px-2 py-1 shadow-sm font-mono text-xs'><strong>${d.label}</strong>: ${d.value}</div>`" />
          </VisXYContainer>
        </ChartContainer>
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
                  <template v-if="getColRef(col) && row[col]">
                    <RouterLink
                      :to="resolveRefLink(getColRef(col), row)"
                      class="inline-flex items-center gap-1 text-primary hover:underline"
                    >
                      {{ row[col] }}
                      <ExternalLink class="size-3 opacity-50" />
                    </RouterLink>
                  </template>
                  <template v-else>{{ row[col] }}</template>
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

    <Dialog :open="savedQueryPendingDelete !== null" @update:open="handleSavedQueryDialogOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Saved Query</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete
            <strong>{{ savedQueryPendingDelete?.name || 'this saved query' }}</strong
            >? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="savedQueryPendingDelete = null">Cancel</Button>
          <Button variant="destructive" :disabled="deletingSavedQuery" @click="deleteSavedQuery">
            {{ deletingSavedQuery ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter, RouterLink } from 'vue-router'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Play, FileJson, BarChart3, TrendingUp, Search, Database, ExternalLink, Save, Trash2, Bookmark as BookmarkIcon } from 'lucide-vue-next'
import { VueMonacoEditor } from '@guolao/vue-monaco-editor'
import { api } from '@/api/client'
import { notifyError, notifyMutationError, notifyMutationSuccess, notifySuccess } from '@/lib/notify'
import { VisXYContainer, VisLine, VisArea, VisAxis, VisStackedBar } from '@unovis/vue'
import { ChartContainer, ChartCrosshair } from '@/components/ui/chart'
import { Badge } from '@/components/ui/badge'
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'

const route = useRoute()
const router = useRouter()

const activeMode = ref(route.query.mode === 'sql' ? 'sql' : 'visual')
const schemas = ref<Record<string, any>>({})

// Visual State
const visTable = ref(String(route.query.table || 'events'))
const visGroup = ref(String(route.query.group || 'event_type'))
const visMetricFunc = ref(String(route.query.func || 'COUNT'))
const visMetricCol = ref(String(route.query.mcol || '*'))
const visFilters = ref<{col: string, op: string, val: string}[]>(
  route.query.filters ? JSON.parse(String(route.query.filters)) : []
)

const timeRange = ref(String(route.query.time || '12h'))
const chartType = ref<'bar' | 'line'>('bar')
const chartTypes = [
  { value: 'bar' as const, icon: BarChart3 },
  { value: 'line' as const, icon: TrendingUp },
]
const colors = ['hsl(12, 76%, 61%)', 'hsl(173, 58%, 39%)', 'hsl(197, 37%, 24%)', 'hsl(43, 74%, 66%)', 'hsl(27, 87%, 67%)']

const chartConfig = {
  value: {
    label: 'Value',
    color: 'hsl(12, 76%, 61%)'
  }
}

const query = ref('')
const running = ref(false)
const hasRun = ref(false)
const queryError = ref('')
const queryTime = ref(0)
const tableFilter = ref('')

const rows = ref<Record<string, any>[]>([])
const columns = ref<string[]>([])

// Saved queries
interface SavedQueryItem { id: string; name: string; description: string; sql: string; created_at: string }
const savedQueries = ref<SavedQueryItem[]>([])
const savedQueryPendingDelete = ref<SavedQueryItem | null>(null)
const deletingSavedQuery = ref(false)

async function fetchSavedQueries() {
  try {
    const data = await api.get<any>('/v1/analytics/queries')
    savedQueries.value = data.items || []
  } catch { /* ignore */ }
}

async function saveCurrentQuery() {
  const name = prompt('Query name:', '')
  if (!name) return
  const desc = prompt('Description (optional):', '')
  try {
    await api.post('/v1/analytics/queries', { name, description: desc || '', sql: query.value })
    await fetchSavedQueries()
    notifySuccess('Saved query created')
  } catch (err: any) {
    notifyError('Failed to save query', err)
  }
}

function loadSavedQuery(sq: SavedQueryItem) {
  query.value = sq.sql
  activeMode.value = 'sql'
}

function requestDeleteSavedQuery(savedQuery: SavedQueryItem) {
  savedQueryPendingDelete.value = savedQuery
}

function handleSavedQueryDialogOpen(next: boolean) {
  if (!next) savedQueryPendingDelete.value = null
}

async function deleteSavedQuery() {
  if (!savedQueryPendingDelete.value) return
  deletingSavedQuery.value = true
  try {
    await api.delete(`/v1/analytics/queries/${savedQueryPendingDelete.value.id}`)
    await fetchSavedQueries()
    notifyMutationSuccess('Saved query', 'delete')
    savedQueryPendingDelete.value = null
  } catch (err: any) {
    notifyMutationError('Saved query', 'delete', err)
  } finally {
    deletingSavedQuery.value = false
  }
}

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
  automaticLayout: true,
  scrollbar: { vertical: 'hidden' as const, horizontal: 'hidden' as const },
}

const currentColumns = computed(() => {
  if (!schemas.value[visTable.value]) return []
  return schemas.value[visTable.value].columns || []
})

const generatedSQL = computed(() => {
  const t = visTable.value
  if (!t) return ''
  
  const metric = visMetricCol.value === '*' ? '*' : visMetricCol.value
  let sel: string
  if (visMetricFunc.value === 'NONE') {
    // "SELECT ROWS" mode — no aggregation
    sel = visGroup.value && visGroup.value !== 'NONE' ? `${visGroup.value}, *` : '*'
  } else {
    sel = `${visMetricFunc.value}(${metric}) as metric`
    if (visGroup.value && visGroup.value !== 'NONE') sel = `${visGroup.value}, ${sel}`
  }

  let sql = `SELECT ${sel}\nFROM ${t}`

  let wheres: string[] = []
  if (visFilters.value.length > 0) {
    wheres.push(...visFilters.value.map((f) => {
      const v = isNaN(Number(f.val)) ? `'${f.val}'` : f.val
      return `${f.col} ${f.op} ${v}`
    }))
  }
  
  const cols = currentColumns.value.map((c: any) => c.name)
  const timeCol = cols.find((c: string) => c === 'created_at' || c === 'timestamp')
  if (timeCol) {
    if (timeRange.value === '1h') wheres.push(`${timeCol} >= datetime('now', '-1 hour')`)
    else if (timeRange.value === '12h') wheres.push(`${timeCol} >= datetime('now', '-12 hours')`)
    else if (timeRange.value === '24h') wheres.push(`${timeCol} >= datetime('now', '-24 hours')`)
    else if (timeRange.value === '7d') wheres.push(`${timeCol} >= datetime('now', '-7 days')`)
    else if (timeRange.value === '30d') wheres.push(`${timeCol} >= datetime('now', '-30 days')`)
  }

  if (wheres.length > 0) {
    sql += `\nWHERE ${wheres.join(' AND ')}`
  }

  if (visGroup.value && visGroup.value !== 'NONE' && visMetricFunc.value !== 'NONE') {
    sql += `\nGROUP BY ${visGroup.value}`
    sql += `\nORDER BY metric DESC`
  } else {
    sql += `\nORDER BY 1 DESC`
  }

  sql += `\nLIMIT 100`
  return sql
})

// Sync SQL into editor
watch([generatedSQL, activeMode], ([sql, mode]) => {
  if (mode === 'visual') {
    query.value = sql
  }
}, { immediate: true })

// Sync state to URL
watch([activeMode, visTable, visGroup, visMetricFunc, visMetricCol, visFilters, timeRange], () => {
  router.replace({
    query: {
      ...route.query,
      mode: activeMode.value,
      table: visTable.value,
      group: visGroup.value,
      func: visMetricFunc.value,
      mcol: visMetricCol.value,
      filters: visFilters.value.length > 0 ? JSON.stringify(visFilters.value) : undefined,
      time: timeRange.value,
    }
  }).catch(() => {})
}, { deep: true })

onMounted(async () => {
  try {
    const data = await api.get<any>('/v1/analytics/schema')
    schemas.value = data || {}
  } catch (err) {
    console.error('Failed to load schemas', err)
  }
  fetchSavedQueries()
  if (activeMode.value === 'visual') {
    runQuery()
  }
})

function addFilter() {
  visFilters.value.push({ col: currentColumns.value[0]?.name || '', op: '=', val: '' })
}
function removeFilter(i: number) {
  visFilters.value.splice(i, 1)
}

// --- x-ref helpers: make FK columns clickable ---

/** Look up x-ref metadata for a column in the current query's table schema. */
function getColRef(colName: string): any | null {
  // Find which table this column belongs to by checking all schemas.
  for (const [, tableSchema] of Object.entries(schemas.value)) {
    const cols = (tableSchema as any)?.columns || []
    const col = cols.find((c: any) => c.name === colName && c.ref)
    if (col) return col.ref
  }
  return null
}

/** Resolve an x-ref path template into a Vue Router path. */
function resolveRefLink(refInfo: any, row: Record<string, any>): string {
  if (!refInfo?.resource) return '#'

  // For user references, navigate to the identity detail page.
  if (refInfo.resource === 'users' || refInfo.resource === 'entities') {
    const id = row[Object.keys(row).find(k => k.endsWith('_id') && row[k]) || ''] || ''
    const type = row['actor_type'] || row['aggregate_type'] || 'human_user'
    return `/users/${id}`
  }

  // For session references, link to sessions view.
  if (refInfo.resource === 'sessions') {
    const sessionId = row['session_id'] || ''
    return `/console/sessions?id=${sessionId}`
  }

  return '#'
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
    const data = await api.post<any>('/v1/analytics/query', { sql: query.value, limit: 1000 })
    queryTime.value = Math.round(performance.now() - start)

    if (data.error) {
      queryError.value = data.error
      rows.value = []
      columns.value = []
      return
    }

    const resultRows = data.rows || []
    const colNames: string[] = data.columns || []

    if (resultRows.length > 0) {
      // API returns rows as arrays — transform to keyed objects using column names
      if (Array.isArray(resultRows[0])) {
        columns.value = colNames.length > 0 ? colNames : resultRows[0].map((_: any, i: number) => String(i))
        rows.value = resultRows.map((r: any[]) => {
          const out: Record<string, any> = {}
          for (let i = 0; i < columns.value.length; i++) {
            const v = r[i]
            out[columns.value[i]] = isNaN(Number(v)) || v === '' || v === null ? v : Number(v)
          }
          return out
        })
      } else {
        // Rows already keyed (shouldn't happen with current API, but handle gracefully)
        columns.value = Object.keys(resultRows[0])
        rows.value = resultRows.map((r: any) => {
          const out: Record<string, any> = {}
          for (const [k, v] of Object.entries(r)) {
            out[k] = isNaN(Number(v)) || v === '' || v === null ? v : Number(v)
          }
          return out
        })
      }
    } else {
      columns.value = colNames
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
