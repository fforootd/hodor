<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">System Authorization</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Zitadel's internal ReBAC model — controls platform access (admin, org management, entity CRUD).
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Badge variant="secondary" class="gap-1.5">
          <Lock class="size-3" />
          Internal Model
        </Badge>
        <Badge variant="outline" class="gap-1.5">
          <Shield class="size-3" />
          OpenFGA v1.1
        </Badge>
      </div>
    </div>

    <!-- Distinction callout -->
    <div class="rounded-lg border border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/30 px-4 py-3 flex items-start gap-3">
      <Info class="size-4 text-blue-500 mt-0.5 shrink-0" />
      <div class="text-sm">
        <p class="font-medium text-blue-900 dark:text-blue-200">This is Zitadel's internal authorization engine.</p>
        <p class="text-blue-700 dark:text-blue-400 mt-0.5">
          It governs who can manage instances, orgs, entities, and platform settings.
          Tenant-level authorization models (for your applications) will be available in a future release.
        </p>
      </div>
    </div>

    <!-- Tabs -->
    <Tabs v-model="activeTab" class="space-y-4">
      <TabsList class="grid w-full max-w-md grid-cols-3">
        <TabsTrigger value="model" class="gap-1.5">
          <Network class="size-3.5" />
          Model
        </TabsTrigger>
        <TabsTrigger value="tuples" class="gap-1.5">
          <Database class="size-3.5" />
          Tuples
        </TabsTrigger>
        <TabsTrigger value="playground" class="gap-1.5">
          <Play class="size-3.5" />
          Playground
        </TabsTrigger>
      </TabsList>

      <!-- ═══════════ Tab 1: Model Graph ═══════════ -->
      <TabsContent value="model" class="space-y-4">
        <Card>
          <div class="p-4 pb-2 flex items-center justify-between">
            <div>
              <h3 class="font-medium">Authorization Model</h3>
              <p class="text-xs text-muted-foreground">
                Types, relations, and permission hierarchy. Click a node to inspect.
              </p>
            </div>
            <div class="flex gap-2">
              <Button variant="outline" size="sm" @click="fetchModelGraph" :disabled="loadingModel">
                <RefreshCw class="size-3.5 mr-1" :class="{ 'animate-spin': loadingModel }" />
                Refresh
              </Button>
            </div>
          </div>
          <div class="px-4 pb-4">
            <!-- Graph visualization -->
            <div
              ref="graphContainer"
              class="relative w-full rounded-lg border bg-muted/30 overflow-hidden"
              style="height: 480px;"
              @wheel.prevent="onGraphWheel"
              @mousedown="onGraphMouseDown"
              @mousemove="onGraphMouseMove"
              @mouseup="onGraphMouseUp"
              @mouseleave="onGraphMouseUp"
            >
              <svg
                :viewBox="`${viewBox.x} ${viewBox.y} ${viewBox.w} ${viewBox.h}`"
                class="w-full h-full"
                style="user-select: none;"
              >
                <defs>
                  <marker id="arrowhead" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
                    <polygon points="0 0, 8 3, 0 6" class="fill-muted-foreground/50" />
                  </marker>
                </defs>

                <!-- Edges -->
                <g v-for="(edge, i) in graphEdges" :key="'e-'+i">
                  <line
                    :x1="edge.x1" :y1="edge.y1"
                    :x2="edge.x2" :y2="edge.y2"
                    class="stroke-muted-foreground/30"
                    stroke-width="1.5"
                    marker-end="url(#arrowhead)"
                  />
                  <text
                    :x="(edge.x1 + edge.x2) / 2"
                    :y="(edge.y1 + edge.y2) / 2 - 6"
                    text-anchor="middle"
                    class="fill-muted-foreground text-[9px]"
                  >{{ edge.relation }}</text>
                </g>

                <!-- Nodes -->
                <g
                  v-for="node in graphNodes"
                  :key="node.id"
                  :transform="`translate(${node.x}, ${node.y})`"
                  class="cursor-pointer"
                  @click="selectNode(node)"
                >
                  <rect
                    :width="node.w" :height="node.h"
                    :rx="8"
                    :class="selectedNode?.id === node.id
                      ? 'fill-primary/20 stroke-primary stroke-2'
                      : 'fill-card stroke-border'"
                    stroke-width="1.5"
                  />
                  <text
                    :x="node.w / 2" :y="20"
                    text-anchor="middle"
                    class="fill-foreground text-[12px] font-semibold"
                  >{{ node.id }}</text>
                  <text
                    :x="node.w / 2" :y="34"
                    text-anchor="middle"
                    class="fill-muted-foreground text-[9px]"
                  >{{ node.relations.length }} rels · {{ node.permissions.length }} perms</text>
                </g>
              </svg>
            </div>

            <!-- Selected node detail -->
            <div v-if="selectedNode" class="mt-4 grid grid-cols-2 gap-4">
              <Card class="p-4">
                <h4 class="text-sm font-medium mb-2 flex items-center gap-1.5">
                  <Link class="size-3.5 text-blue-500" />
                  Relations
                </h4>
                <div class="flex flex-wrap gap-1.5">
                  <Badge v-for="rel in selectedNode.relations" :key="rel" variant="secondary" class="text-xs">
                    {{ rel }}
                  </Badge>
                  <span v-if="!selectedNode.relations.length" class="text-xs text-muted-foreground">None</span>
                </div>
              </Card>
              <Card class="p-4">
                <h4 class="text-sm font-medium mb-2 flex items-center gap-1.5">
                  <Shield class="size-3.5 text-emerald-500" />
                  Permissions
                </h4>
                <div class="flex flex-wrap gap-1.5">
                  <Badge v-for="perm in selectedNode.permissions" :key="perm" variant="outline" class="text-xs">
                    {{ perm }}
                  </Badge>
                  <span v-if="!selectedNode.permissions.length" class="text-xs text-muted-foreground">None</span>
                </div>
              </Card>
            </div>
          </div>
        </Card>
      </TabsContent>

      <!-- ═══════════ Tab 2: Tuple Explorer ═══════════ -->
      <TabsContent value="tuples" class="space-y-4">
        <Card>
          <div class="p-4 pb-2 flex items-center justify-between">
            <div>
              <h3 class="font-medium">Relationship Tuples</h3>
              <p class="text-xs text-muted-foreground">Browse and manage authorization relationships.</p>
            </div>
            <div class="flex gap-2">
              <Button variant="outline" size="sm" @click="showAddTuple = true">
                <Plus class="size-3.5 mr-1" />
                Add Tuple
              </Button>
              <Button variant="outline" size="sm" @click="fetchTuples" :disabled="loadingTuples">
                <RefreshCw class="size-3.5 mr-1" :class="{ 'animate-spin': loadingTuples }" />
                Refresh
              </Button>
            </div>
          </div>

          <!-- Filters -->
          <div class="px-4 pb-3 flex gap-3 items-end">
            <div class="flex-1">
              <label class="text-xs font-medium text-muted-foreground mb-1 block">User</label>
              <Input v-model="tupleFilter.user" placeholder="e.g. user:admin" class="h-8 text-sm" @keyup.enter="fetchTuples" />
            </div>
            <div class="flex-1">
              <label class="text-xs font-medium text-muted-foreground mb-1 block">Relation</label>
              <Input v-model="tupleFilter.relation" placeholder="e.g. owner" class="h-8 text-sm" @keyup.enter="fetchTuples" />
            </div>
            <div class="flex-1">
              <label class="text-xs font-medium text-muted-foreground mb-1 block">Object</label>
              <Input v-model="tupleFilter.object" placeholder="e.g. org:1" class="h-8 text-sm" @keyup.enter="fetchTuples" />
            </div>
            <Button size="sm" @click="fetchTuples" class="h-8">
              <Search class="size-3.5" />
            </Button>
          </div>

          <!-- Table -->
          <div class="border-t">
            <table class="w-full">
              <thead>
                <tr class="border-b bg-muted/50">
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">User</th>
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Relation</th>
                  <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground">Object</th>
                  <th class="px-4 py-2 text-right text-xs font-medium text-muted-foreground">Actions</th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="loadingTuples" class="border-b">
                  <td colspan="4" class="px-4 py-8 text-center text-sm text-muted-foreground">
                    <RefreshCw class="size-4 animate-spin inline mr-2" />
                    Loading tuples…
                  </td>
                </tr>
                <tr v-else-if="!tuples.length" class="border-b">
                  <td colspan="4" class="px-4 py-8 text-center text-sm text-muted-foreground">
                    No tuples found. Try adjusting your filters.
                  </td>
                </tr>
                <tr
                  v-for="(tuple, i) in tuples"
                  :key="i"
                  class="border-b hover:bg-muted/30 transition-colors"
                >
                  <td class="px-4 py-2.5">
                    <code class="text-xs bg-muted px-1.5 py-0.5 rounded">{{ tuple.user }}</code>
                  </td>
                  <td class="px-4 py-2.5">
                    <Badge variant="secondary" class="text-xs">{{ tuple.relation }}</Badge>
                  </td>
                  <td class="px-4 py-2.5">
                    <code class="text-xs bg-muted px-1.5 py-0.5 rounded">{{ tuple.object }}</code>
                  </td>
                  <td class="px-4 py-2.5 text-right">
                    <Button
                      variant="ghost" size="sm"
                      class="h-7 text-destructive hover:text-destructive"
                      @click="removeTuple(tuple)"
                    >
                      <Trash2 class="size-3.5" />
                    </Button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <div class="p-3 text-xs text-muted-foreground border-t">
            {{ tuples.length }} tuple{{ tuples.length !== 1 ? 's' : '' }} shown
          </div>
        </Card>

        <!-- Add Tuple Dialog -->
        <Dialog v-model:open="showAddTuple">
          <DialogContent class="max-w-md">
            <DialogHeader>
              <DialogTitle>Add Relationship Tuple</DialogTitle>
            </DialogHeader>
            <div class="space-y-3 py-2">
              <div>
                <label class="text-sm font-medium">User</label>
                <Input v-model="newTuple.user" placeholder="user:alice" class="mt-1" />
              </div>
              <div>
                <label class="text-sm font-medium">Relation</label>
                <Input v-model="newTuple.relation" placeholder="member" class="mt-1" />
              </div>
              <div>
                <label class="text-sm font-medium">Object</label>
                <Input v-model="newTuple.object" placeholder="org:default" class="mt-1" />
              </div>
            </div>
            <div class="flex justify-end gap-2 pt-2">
              <Button variant="outline" @click="showAddTuple = false">Cancel</Button>
              <Button @click="addTuple" :disabled="!newTuple.user || !newTuple.relation || !newTuple.object">
                <Plus class="size-3.5 mr-1" />
                Add Tuple
              </Button>
            </div>
          </DialogContent>
        </Dialog>
      </TabsContent>

      <!-- ═══════════ Tab 3: Check Playground ═══════════ -->
      <TabsContent value="playground" class="space-y-4">
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <!-- Check Form -->
          <Card class="p-4 space-y-4">
            <div>
              <h3 class="font-medium flex items-center gap-1.5">
                <Play class="size-4 text-primary" />
                Authorization Check
              </h3>
              <p class="text-xs text-muted-foreground mt-0.5">
                Test whether a user has a specific permission on an object.
              </p>
            </div>

            <div class="space-y-3">
              <div>
                <label class="text-sm font-medium">User</label>
                <Input v-model="checkForm.user" placeholder="user:admin" class="mt-1" />
              </div>
              <div>
                <label class="text-sm font-medium">Relation / Permission</label>
                <Input v-model="checkForm.relation" placeholder="can_read" class="mt-1" />
              </div>
              <div>
                <label class="text-sm font-medium">Object</label>
                <Input v-model="checkForm.object" placeholder="org:1" class="mt-1" />
              </div>
            </div>

            <div class="flex gap-2">
              <Button @click="runCheck" :disabled="checkRunning || !checkFormValid" class="flex-1">
                <Play class="size-3.5 mr-1" />
                Check
              </Button>
              <Button variant="outline" @click="runExpand" :disabled="checkRunning || !checkForm.relation || !checkForm.object">
                <GitBranch class="size-3.5 mr-1" />
                Expand
              </Button>
            </div>

            <!-- Quick presets -->
            <div class="space-y-2">
              <p class="text-xs font-medium text-muted-foreground">Quick Checks</p>
              <div class="flex flex-wrap gap-1.5">
                <Button
                  v-for="preset in presets"
                  :key="preset.label"
                  variant="outline"
                  size="sm"
                  class="text-xs h-7"
                  @click="applyPreset(preset)"
                >
                  {{ preset.label }}
                </Button>
              </div>
            </div>
          </Card>

          <!-- Result Panel -->
          <Card class="p-4 space-y-4">
            <h3 class="font-medium">Result</h3>

            <!-- Empty state -->
            <div v-if="!checkResult && !expandResult" class="flex flex-col items-center justify-center py-12 text-muted-foreground">
              <Shield class="size-10 mb-3 opacity-30" />
              <p class="text-sm">Run a check or expand to see results</p>
            </div>

            <!-- Check result -->
            <div v-if="checkResult" class="space-y-3">
              <div
                class="flex items-center gap-3 p-4 rounded-lg border-2 transition-all"
                :class="checkResult.allowed
                  ? 'border-emerald-500/40 bg-emerald-500/5'
                  : 'border-red-500/40 bg-red-500/5'"
              >
                <div
                  class="flex items-center justify-center size-10 rounded-full"
                  :class="checkResult.allowed ? 'bg-emerald-500/20' : 'bg-red-500/20'"
                >
                  <CheckCircle2 v-if="checkResult.allowed" class="size-5 text-emerald-500" />
                  <XCircle v-else class="size-5 text-red-500" />
                </div>
                <div>
                  <p class="font-semibold" :class="checkResult.allowed ? 'text-emerald-600' : 'text-red-600'">
                    {{ checkResult.allowed ? 'ALLOWED' : 'DENIED' }}
                  </p>
                  <p class="text-xs text-muted-foreground">
                    {{ checkResult.user }} → {{ checkResult.relation }} → {{ checkResult.object }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Expand result -->
            <div v-if="expandResult" class="space-y-2">
              <p class="text-xs font-medium text-muted-foreground">Expansion Tree</p>
              <div class="rounded-lg border bg-muted/30 p-4 overflow-auto max-h-64">
                <pre class="text-xs font-mono">{{ JSON.stringify(expandResult, null, 2) }}</pre>
              </div>
            </div>

            <!-- Check History -->
            <div v-if="checkHistory.length" class="space-y-2">
              <p class="text-xs font-medium text-muted-foreground">Recent Checks</p>
              <div class="space-y-1">
                <button
                  v-for="(entry, i) in checkHistory"
                  :key="i"
                  class="w-full flex items-center gap-2 p-2 rounded-md hover:bg-muted/50 transition-colors text-left"
                  @click="replayCheck(entry)"
                >
                  <CheckCircle2 v-if="entry.allowed" class="size-3.5 text-emerald-500 shrink-0" />
                  <XCircle v-else class="size-3.5 text-red-500 shrink-0" />
                  <code class="text-xs truncate flex-1">{{ entry.user }} → {{ entry.relation }} → {{ entry.object }}</code>
                </button>
              </div>
            </div>
          </Card>
        </div>
      </TabsContent>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, reactive } from 'vue'
import { fgaApi, type FGATuple, type FGAModelNode, type FGAModelEdge, type FGACheckResult } from '@/api/resources'
import { toast } from 'vue-sonner'

// shadcn components
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'

// Icons
import {
  Shield, Network, Database, Play, RefreshCw, Plus, Trash2, Search,
  Link, CheckCircle2, XCircle, GitBranch, Lock, Info,
} from 'lucide-vue-next'

const activeTab = ref('model')

// ═══════════ Model Graph ═══════════

interface GraphNode extends FGAModelNode {
  x: number; y: number; w: number; h: number
}
interface GraphEdge {
  x1: number; y1: number; x2: number; y2: number
  relation: string
}

const loadingModel = ref(false)
const graphNodes = ref<GraphNode[]>([])
const graphEdges = ref<GraphEdge[]>([])
const selectedNode = ref<GraphNode | null>(null)
const modelNodes = ref<FGAModelNode[]>([])
const modelEdges = ref<FGAModelEdge[]>([])

// Graph viewport
const viewBox = reactive({ x: -50, y: -30, w: 900, h: 550 })
const isPanning = ref(false)
const panStart = reactive({ x: 0, y: 0 })

function onGraphWheel(e: WheelEvent) {
  const zoomFactor = e.deltaY > 0 ? 1.1 : 0.9
  viewBox.w *= zoomFactor
  viewBox.h *= zoomFactor
}

function onGraphMouseDown(e: MouseEvent) {
  isPanning.value = true
  panStart.x = e.clientX
  panStart.y = e.clientY
}

function onGraphMouseMove(e: MouseEvent) {
  if (!isPanning.value) return
  const dx = (e.clientX - panStart.x) * (viewBox.w / 900)
  const dy = (e.clientY - panStart.y) * (viewBox.h / 550)
  viewBox.x -= dx
  viewBox.y -= dy
  panStart.x = e.clientX
  panStart.y = e.clientY
}

function onGraphMouseUp() {
  isPanning.value = false
}

function selectNode(node: GraphNode) {
  selectedNode.value = selectedNode.value?.id === node.id ? null : node
}

// Layout: place nodes in a hierarchy
//   Row 0: user
//   Row 1: instance
//   Row 2: org, settings, session
//   Row 3: entity, app, group
const nodeLayout: Record<string, { row: number; col: number }> = {
  user:     { row: 0, col: 1 },
  instance: { row: 1, col: 1 },
  org:      { row: 2, col: 0 },
  settings: { row: 2, col: 1 },
  session:  { row: 2, col: 2 },
  entity:   { row: 3, col: 0 },
  app:      { row: 3, col: 1 },
  group:    { row: 3, col: 2 },
}

function layoutGraph() {
  const nodeW = 140
  const nodeH = 46
  const colGap = 200
  const rowGap = 120
  const offsetX = 100
  const offsetY = 20

  const positioned: GraphNode[] = []
  for (const n of modelNodes.value) {
    const layout = nodeLayout[n.id] || { row: 4, col: positioned.filter(p => !nodeLayout[p.id]).length }
    positioned.push({
      ...n,
      x: offsetX + layout.col * colGap,
      y: offsetY + layout.row * rowGap,
      w: nodeW,
      h: nodeH,
    })
  }
  graphNodes.value = positioned

  // Build edges
  const nodeMap = new Map(positioned.map(n => [n.id, n]))
  const edges: GraphEdge[] = []
  for (const e of modelEdges.value) {
    const from = nodeMap.get(e.from)
    const to = nodeMap.get(e.to)
    if (!from || !to) continue
    edges.push({
      x1: from.x + from.w / 2,
      y1: from.y + from.h,
      x2: to.x + to.w / 2,
      y2: to.y,
      relation: e.relation,
    })
  }
  graphEdges.value = edges
}

async function fetchModelGraph() {
  loadingModel.value = true
  try {
    const data = await fgaApi.getModelGraph()
    modelNodes.value = data.nodes || []
    modelEdges.value = data.edges || []
    layoutGraph()
  } catch (err: any) {
    toast.error('Failed to load model', { description: err.message })
  } finally {
    loadingModel.value = false
  }
}

// ═══════════ Tuple Explorer ═══════════

const loadingTuples = ref(false)
const tuples = ref<FGATuple[]>([])
const tupleFilter = reactive({ user: '', relation: '', object: '' })
const showAddTuple = ref(false)
const newTuple = reactive({ user: '', relation: '', object: '' })

async function fetchTuples() {
  loadingTuples.value = true
  try {
    const params: Record<string, string> = {}
    if (tupleFilter.user) params.user = tupleFilter.user
    if (tupleFilter.relation) params.relation = tupleFilter.relation
    if (tupleFilter.object) params.object = tupleFilter.object
    tuples.value = await fgaApi.readTuples(params)
  } catch (err: any) {
    toast.error('Failed to load tuples', { description: err.message })
  } finally {
    loadingTuples.value = false
  }
}

async function addTuple() {
  try {
    await fgaApi.writeTuples([{ user: newTuple.user, relation: newTuple.relation, object: newTuple.object }])
    toast.success('Tuple added')
    showAddTuple.value = false
    newTuple.user = ''
    newTuple.relation = ''
    newTuple.object = ''
    await fetchTuples()
  } catch (err: any) {
    toast.error('Failed to add tuple', { description: err.message })
  }
}

async function removeTuple(tuple: FGATuple) {
  try {
    await fgaApi.deleteTuples([{ user: tuple.user, relation: tuple.relation, object: tuple.object }])
    toast.success('Tuple removed')
    await fetchTuples()
  } catch (err: any) {
    toast.error('Failed to remove tuple', { description: err.message })
  }
}

// ═══════════ Check Playground ═══════════

const checkForm = reactive({ user: '', relation: '', object: '' })
const checkFormValid = computed(() => checkForm.user && checkForm.relation && checkForm.object)
const checkRunning = ref(false)
const checkResult = ref<FGACheckResult | null>(null)
const expandResult = ref<any>(null)
const checkHistory = ref<(FGACheckResult)[]>([])

const presets = [
  { label: 'Admin → manage orgs', user: 'user:admin', relation: 'can_manage_orgs', object: 'instance:default' },
  { label: 'Admin → create entity', user: 'user:admin', relation: 'can_create_entity', object: 'org:1' },
  { label: 'Admin → view audit', user: 'user:admin', relation: 'can_view_audit', object: 'instance:default' },
  { label: 'Admin → manage FGA', user: 'user:admin', relation: 'can_manage_fga', object: 'instance:default' },
]

function applyPreset(preset: typeof presets[0]) {
  checkForm.user = preset.user
  checkForm.relation = preset.relation
  checkForm.object = preset.object
  runCheck()
}

async function runCheck() {
  checkRunning.value = true
  expandResult.value = null
  try {
    const result = await fgaApi.check(checkForm.user, checkForm.relation, checkForm.object)
    checkResult.value = result
    // Add to history (dedupe)
    checkHistory.value = [
      result,
      ...checkHistory.value.filter(h =>
        !(h.user === result.user && h.relation === result.relation && h.object === result.object)
      ),
    ].slice(0, 10)
  } catch (err: any) {
    toast.error('Check failed', { description: err.message })
  } finally {
    checkRunning.value = false
  }
}

async function runExpand() {
  checkRunning.value = true
  checkResult.value = null
  try {
    const result = await fgaApi.expand(checkForm.relation, checkForm.object)
    expandResult.value = result.tree
  } catch (err: any) {
    toast.error('Expand failed', { description: err.message })
  } finally {
    checkRunning.value = false
  }
}

function replayCheck(entry: FGACheckResult) {
  checkForm.user = entry.user
  checkForm.relation = entry.relation
  checkForm.object = entry.object
  runCheck()
}

// ═══════════ Init ═══════════

onMounted(() => {
  fetchModelGraph()
  fetchTuples()
})
</script>
