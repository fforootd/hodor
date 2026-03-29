<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Authorization Model</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Types, relations, and permission hierarchy. Click a node to inspect.
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Badge variant="outline" class="gap-1.5">
          <Shield class="size-3" />
          OpenFGA v1.1
        </Badge>
        <Button variant="outline" size="sm" @click="fetchModelGraph" :disabled="loadingModel">
          <RefreshCw class="size-3.5 mr-1" :class="{ 'animate-spin': loadingModel }" />
          Refresh
        </Button>
      </div>
    </div>

    <!-- Graph visualization -->
    <Card>
      <div class="p-4">
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { fgaApi, type FGAModelNode, type FGAModelEdge } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Shield, RefreshCw, Link } from 'lucide-vue-next'

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

const nodeLayout: Record<string, { row: number; col: number }> = {
  user:     { row: 0, col: 1 },
  instance: { row: 1, col: 1 },
  org:      { row: 2, col: 0 },
  settings: { row: 2, col: 1 },
  session:  { row: 2, col: 2 },
  project:  { row: 3, col: 0 },
  app:      { row: 3, col: 1 },
  group:    { row: 3, col: 2 },
}

function layoutGraph() {
  const nodeW = 140, nodeH = 46, colGap = 200, rowGap = 120, offsetX = 100, offsetY = 20
  const positioned: GraphNode[] = []
  for (const n of modelNodes.value) {
    const layout = nodeLayout[n.id] || { row: 4, col: positioned.filter(p => !nodeLayout[p.id]).length }
    positioned.push({ ...n, x: offsetX + layout.col * colGap, y: offsetY + layout.row * rowGap, w: nodeW, h: nodeH })
  }
  graphNodes.value = positioned

  const nodeMap = new Map(positioned.map(n => [n.id, n]))
  const edges: GraphEdge[] = []
  for (const e of modelEdges.value) {
    const from = nodeMap.get(e.from)
    const to = nodeMap.get(e.to)
    if (!from || !to) continue
    edges.push({ x1: from.x + from.w / 2, y1: from.y + from.h, x2: to.x + to.w / 2, y2: to.y, relation: e.relation })
  }
  graphEdges.value = edges
}

async function fetchModelGraph() {
  loadingModel.value = true
  try {
    const data = await fgaApi.getModelGraph()
    modelNodes.value = (data.nodes || []) as FGAModelNode[]
    modelEdges.value = (data.edges || []) as FGAModelEdge[]
    layoutGraph()
  } catch (err: any) {
    toast.error('Failed to load model', { description: err.message })
  } finally {
    loadingModel.value = false
  }
}

onMounted(fetchModelGraph)
</script>
