<template>
  <div class="h-full flex flex-col pt-4">
    <div class="px-6 flex items-center justify-between shrink-0 mb-6">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Fingerprints</h1>
        <p class="text-sm text-muted-foreground mt-1">Raw device fingerprint payloads ingested via telemetry.</p>
      </div>
      <div class="flex items-center gap-3">
        <Button variant="outline" size="sm" :disabled="loading" @click="fetch">
          <RefreshCcw class="w-4 h-4 mr-2" :class="{ 'animate-spin': loading }" /> Refresh
        </Button>
      </div>
    </div>

    <!-- Data Table -->
    <div class="px-6 flex-1 min-h-0 overflow-y-auto pb-6">
      <Card class="h-[calc(100vh-140px)] flex flex-col border-muted/60 shadow-sm overflow-hidden">
        <!-- Table Header (Sticky) -->
        <div class="grid grid-cols-[160px_1fr_180px_120px] items-center px-4 py-3 border-b bg-muted/30 text-xs font-medium text-muted-foreground sticky top-0 z-10 shrink-0">
          <div>Fingerprint ID</div>
          <div>Payload Summary</div>
          <div>Seen At</div>
          <div class="text-right">Actions</div>
        </div>

        <div v-if="loading && items.length === 0" class="flex-1 flex flex-col items-center justify-center p-12 text-muted-foreground">
          <Loader2 class="w-6 h-6 animate-spin mb-4" />
          <p class="text-sm">Loading fingerprints...</p>
        </div>

        <div v-else-if="items.length === 0" class="flex-1 flex flex-col items-center justify-center p-12 text-center overflow-y-auto">
          <div class="w-12 h-12 rounded-full bg-muted/50 flex items-center justify-center mb-4 border border-border/50 shadow-sm">
            <Fingerprint class="w-6 h-6 text-muted-foreground" />
          </div>
          <h3 class="text-sm font-medium">No Fingerprints Found</h3>
          <p class="text-sm text-muted-foreground mt-2 max-w-[250px]">
            No device telemetry has been ingested yet.
          </p>
        </div>

        <!-- Scrollable List -->
        <div v-else class="flex-1 overflow-y-auto divide-y">
          <div v-for="item in items" :key="item.id" class="grid grid-cols-[160px_1fr_180px_120px] items-center px-4 py-3 hover:bg-muted/30 transition-colors text-sm group">
            <div class="font-mono text-xs text-primary truncate pr-4" :title="item.id">
              {{ truncateId(item.id, 12) }}
            </div>
            
            <div class="min-w-0 pr-4">
              <p class="text-xs text-muted-foreground truncate font-mono">
                {{ formatSummary(item.raw_data) }}
              </p>
            </div>

            <div class="text-xs text-muted-foreground">
              {{ formatDate(item.created_at) }}
            </div>

            <div class="flex items-center justify-end gap-2 outline-none">
              <Button variant="ghost" size="icon" class="h-8 w-8 text-muted-foreground group-hover:text-foreground" @click.stop="toggleInspect(item)">
                <Code class="w-4 h-4" />
              </Button>
            </div>

            <!-- Inspect Panel -->
            <div v-if="inspecting === item.id" class="col-span-4 mt-3 bg-muted/20 border rounded-md p-4 animate-in fade-in slide-in-from-top-2 duration-200">
              <div class="flex items-center justify-between mb-2">
                <span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Raw Payload</span>
                <RouterLink :to="`/events?fingerprint=${item.id}`" class="text-xs text-primary hover:underline flex items-center gap-1">
                  View Events <ExternalLink class="w-3 h-3" />
                </RouterLink>
              </div>
              <pre class="text-[11px] font-mono whitespace-pre-wrap max-h-[300px] overflow-y-auto bg-background border p-3 rounded text-foreground/80">{{ JSON.stringify(item.raw_data, null, 2) }}</pre>
            </div>
          </div>
          
          <!-- Next Page Loader -->
          <div ref="loadMoreSentinel" class="h-10 flex items-center justify-center py-2">
            <Loader2 v-if="loadingMore" class="w-4 h-4 animate-spin text-muted-foreground" />
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { RefreshCcw, Loader2, Fingerprint, Code, ExternalLink } from 'lucide-vue-next'
import { api } from '@/api/client'

const items = ref<any[]>([])
const loading = ref(true)
const loadingMore = ref(false)
const nextCursor = ref('')
const hasMore = ref(true)
const inspecting = ref<string | null>(null)
const loadMoreSentinel = ref<HTMLElement | null>(null)
let observer: IntersectionObserver | null = null

async function fetch() {
  loading.value = true
  items.value = []
  nextCursor.value = ''
  hasMore.value = true
  inspecting.value = null
  
  try {
    const res = await api.get<{ items: any[], next_cursor?: string }>('/v1/telemetry/fingerprints')
    items.value = res.items || []
    nextCursor.value = res.next_cursor || ''
    hasMore.value = !!res.next_cursor
  } catch (err) {
    console.error('Failed to load fingerprints', err)
  } finally {
    loading.value = false
  }
}

async function loadMore() {
  if (loading.value || loadingMore.value || !hasMore.value || !nextCursor.value) return
  loadingMore.value = true
  try {
    const res = await api.get<{ items: any[], next_cursor?: string }>(`/v1/telemetry/fingerprints?cursor=${encodeURIComponent(nextCursor.value)}`)
    if (res.items && res.items.length > 0) {
      items.value.push(...res.items)
    }
    nextCursor.value = res.next_cursor || ''
    hasMore.value = !!res.next_cursor
  } catch (err) {
    console.error('Failed to load more fingerprints', err)
  } finally {
    loadingMore.value = false
  }
}

function toggleInspect(item: any) {
  if (inspecting.value === item.id) {
    inspecting.value = null
  } else {
    inspecting.value = item.id
  }
}

function truncateId(id: string, len = 8) {
  if (!id) return ''
  return id.length > len ? id.substring(0, len) + '…' : id
}

function formatDate(ds: string) {
  if (!ds) return '—'
  return new Date(ds).toLocaleString(undefined, {
    month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', second: '2-digit'
  })
}

function formatSummary(data: any): string {
  if (!data || typeof data !== 'object') return ''
  const parts: string[] = []

  // FingerprintJS OSS v5 format — components have {value, duration} structure
  const c = data.components
  if (c) {
    const renderer = c.webGlBasics?.value?.renderer
    if (renderer) parts.push(renderer.replace(/ANGLE \(/, '').replace(/\)$/, '').split(',')[0].trim())
    const res = c.screenResolution?.value
    if (Array.isArray(res)) parts.push(`${res[0]}×${res[1]}`)
    if (c.platform?.value) parts.push(c.platform.value)
    const langs = c.languages?.value?.[0]
    if (Array.isArray(langs) && langs[0]) parts.push(langs[0])
    if (c.hardwareConcurrency?.value) parts.push(`${c.hardwareConcurrency.value} cores`)
    if (c.deviceMemory?.value) parts.push(`${c.deviceMemory.value}GB`)
  }

  // Legacy/simple formats
  if (parts.length === 0) {
    if (data.ua) parts.push(data.ua.split(/[/()]/)[0].trim())
    if (data.screen) parts.push(data.screen)
    if (data.platform) parts.push(data.platform)
    if (data.language || data.lang) parts.push(data.language || data.lang)
  }

  if (parts.length > 0) return parts.join(', ')

  // Fallback to first few keys
  const keys = Object.keys(data)
  if (keys.length === 0) return ''
  return keys.slice(0, 4).join(', ') + (keys.length > 4 ? '...' : '')
}

onMounted(() => {
  fetch()
  
  observer = new IntersectionObserver((entries) => {
    if (entries[0].isIntersecting) {
      loadMore()
    }
  }, { threshold: 0.1 })
  
  if (loadMoreSentinel.value) {
    observer.observe(loadMoreSentinel.value)
  }
})

onUnmounted(() => {
  if (observer) observer.disconnect()
})
</script>
