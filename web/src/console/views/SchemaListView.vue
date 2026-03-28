<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Schemas</h1>
        <p class="text-sm text-muted-foreground mt-1">Schema types, version history, and template catalog.</p>
      </div>
      <!-- New Schema Type button -->
      <Dialog v-model:open="showCreateDialog">
        <DialogTrigger asChild>
          <Button size="sm" class="gap-1.5">
            <Plus class="size-3.5" />
            New Schema Type
          </Button>
        </DialogTrigger>
        <DialogContent class="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Create Schema Type</DialogTitle>
            <DialogDescription>Create a new entity schema type with a blank definition.</DialogDescription>
          </DialogHeader>
          <div class="space-y-3 py-2">
            <div class="space-y-1.5">
              <Label for="new-type">Type name</Label>
              <Input
                id="new-type"
                v-model="newTypeName"
                placeholder="e.g. device, webhook"
                class="font-mono"
                @keydown.enter="createSchemaType"
              />
              <p class="text-xs text-muted-foreground">Lowercase with underscores. Will become the entity type identifier.</p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" @click="showCreateDialog = false">Cancel</Button>
            <Button @click="createSchemaType" :disabled="!newTypeName.trim() || creatingType">
              {{ creatingType ? 'Creating…' : 'Create' }}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>

    <!-- Tab switcher -->
    <Tabs v-model="activeTab" class="w-full">
      <TabsList class="w-full max-w-xs">
        <TabsTrigger value="schemas" class="flex-1 gap-1.5">
          <FileJson class="size-3.5" />
          My Schemas
        </TabsTrigger>
        <TabsTrigger value="catalog" class="flex-1 gap-1.5">
          <Store class="size-3.5" />
          Catalog
          <Badge variant="secondary" class="ml-1 text-[10px] px-1">{{ catalogTemplates.length }}</Badge>
        </TabsTrigger>
      </TabsList>

      <!-- ═══════════════════════════════ My Schemas Tab ═══════════════════════════════ -->
      <TabsContent value="schemas" class="space-y-4 mt-4">
        <!-- Search bar -->
        <div class="flex items-center gap-3">
          <div class="relative w-full max-w-sm">
            <SearchIcon class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input v-model="searchQuery" placeholder="Filter schemas…" class="pl-9 bg-background" />
          </div>
          <span class="text-xs text-muted-foreground">
            {{ filteredGroups.length }} type{{ filteredGroups.length !== 1 ? 's' : '' }}
          </span>
        </div>

        <!-- Schema type cards — grid matching catalog style -->
        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          <router-link
            v-for="group in filteredGroups" :key="group.type"
            :to="'/schemas/' + group.defaultVersion.id"
            class="no-underline"
          >
            <Card class="group cursor-pointer transition-all hover:shadow-md hover:border-primary/30 h-full">
              <CardHeader class="pb-2">
                <div class="flex items-center justify-between">
                  <CardTitle class="text-sm font-semibold font-mono group-hover:text-primary transition-colors">
                    {{ group.type }}
                  </CardTitle>
                  <div class="flex items-center gap-1">
                    <Badge v-if="group.defaultVersion.is_default" class="text-[10px] shrink-0">
                      v{{ group.defaultVersion.version }}
                    </Badge>
                    <Badge
                      v-if="group.versions.length > 1"
                      variant="outline"
                      class="text-[10px] shrink-0 border-yellow-300 bg-yellow-50 text-yellow-700"
                    >
                      +{{ group.versions.length - 1 }} draft{{ group.versions.length - 1 > 1 ? 's' : '' }}
                    </Badge>
                  </div>
                </div>
                <!-- Field preview as description stand-in -->
                <p class="text-xs text-muted-foreground line-clamp-1 mt-0.5">
                  {{ schemaFields(group.defaultVersion).join(', ') || 'Empty schema' }}
                </p>
              </CardHeader>
              <CardContent class="pt-0 pb-3">
                <div class="flex items-center gap-1.5 flex-wrap">
                  <Badge v-for="field in schemaFields(group.defaultVersion).slice(0, 4)" :key="field" variant="outline" class="text-[10px] font-normal">
                    {{ field }}
                  </Badge>
                  <Badge v-if="schemaFields(group.defaultVersion).length > 4" variant="outline" class="text-[10px] font-normal text-muted-foreground">
                    +{{ schemaFields(group.defaultVersion).length - 4 }}
                  </Badge>
                  <span class="ml-auto text-[10px] text-muted-foreground">
                    {{ formatTime(group.defaultVersion.created_at) }}
                  </span>
                </div>
              </CardContent>
            </Card>
          </router-link>
        </div>

        <div v-if="!filteredGroups.length && searchQuery" class="flex h-24 items-center justify-center text-muted-foreground text-sm">
          No schemas matching "{{ searchQuery }}"
        </div>
        <div v-if="!schemaGroups.length && !searchQuery" class="flex flex-col h-32 items-center justify-center gap-2 text-muted-foreground">
          <FileJson class="size-8 opacity-30" />
          <p class="text-sm">No schemas found</p>
          <Button variant="outline" size="sm" @click="activeTab = 'catalog'" class="gap-1.5 mt-1">
            <Store class="size-3.5" />
            Browse Catalog
          </Button>
        </div>
      </TabsContent>

      <!-- ═══════════════════════════════ Catalog Tab ═══════════════════════════════ -->
      <TabsContent value="catalog" class="space-y-4 mt-4">
        <!-- Catalog filter bar -->
        <div class="flex items-center gap-3 flex-wrap">
          <div class="relative w-full max-w-sm">
            <SearchIcon class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input v-model="catalogSearch" placeholder="Search templates…" class="pl-9 bg-background" />
          </div>
          <!-- Type filter pills -->
          <div class="flex gap-1">
            <Button
              v-for="type in catalogTypes" :key="type"
              size="sm" variant="outline"
              class="h-7 text-xs"
              :class="catalogTypeFilter === type ? 'bg-primary text-primary-foreground hover:bg-primary/90' : ''"
              @click="catalogTypeFilter = catalogTypeFilter === type ? '' : type"
            >
              {{ type }}
            </Button>
          </div>
          <Button variant="ghost" size="sm" class="h-7 text-xs gap-1" @click="refreshCatalog" :disabled="refreshing">
            <RefreshCw class="size-3" :class="refreshing ? 'animate-spin' : ''" />
            Refresh
          </Button>
        </div>

        <!-- Template cards grid -->
        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          <Card
            v-for="tpl in filteredCatalog" :key="tpl.id"
            class="group cursor-pointer transition-all hover:shadow-md hover:border-primary/30"
            @click="openInstall(tpl.id)"
          >
            <CardHeader class="pb-2">
              <div class="flex items-center justify-between">
                <CardTitle class="text-sm font-semibold">{{ tpl.name }}</CardTitle>
                <Badge variant="secondary" class="text-[10px] shrink-0">{{ tpl.type }}</Badge>
              </div>
              <p class="text-xs text-muted-foreground line-clamp-2">{{ tpl.description }}</p>
            </CardHeader>
            <CardContent class="pt-0 pb-3">
              <div class="flex items-center gap-1.5 flex-wrap">
                <Badge v-for="tag in tpl.tags?.slice(0, 3)" :key="tag" variant="outline" class="text-[10px] font-normal">
                  {{ tag }}
                </Badge>
                <Badge v-if="tpl.tags?.length > 3" variant="outline" class="text-[10px] font-normal">
                  +{{ tpl.tags.length - 3 }}
                </Badge>
                <span class="ml-auto text-[10px] text-muted-foreground font-mono">v{{ tpl.version }}</span>
              </div>
            </CardContent>
          </Card>
        </div>

        <div v-if="!filteredCatalog.length && catalogSearch" class="flex h-24 items-center justify-center text-muted-foreground">
          No templates matching "{{ catalogSearch }}"
        </div>
        <div v-if="catalogLoading" class="flex h-24 items-center justify-center text-muted-foreground">
          <Spinner class="mr-2" /> Loading catalog…
        </div>
      </TabsContent>
    </Tabs>

    <!-- Install Dialog -->
    <CatalogInstallDialog
      v-model:open="showInstallDialog"
      :template-id="installTemplateId"
      @installed="onInstalled"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { schemaApi, catalogApi, type Schema, type CatalogTemplate } from '@/api/resources'
import CatalogInstallDialog from '@/console/components/catalog/CatalogInstallDialog.vue'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Spinner } from '@/components/ui/spinner'
import {
  Search as SearchIcon, Plus,
  FileJson, Store, RefreshCw,
} from 'lucide-vue-next'

const router = useRouter()

// ─── Schemas Tab ───
const allSchemas = ref<Schema[]>([])
const searchQuery = ref('')
const activeTab = ref('schemas')

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

const filteredGroups = computed(() => {
  if (!searchQuery.value) return schemaGroups.value
  const q = searchQuery.value.toLowerCase()
  return schemaGroups.value.filter(g =>
    g.type.toLowerCase().includes(q) ||
    schemaFields(g.defaultVersion).some(f => f.toLowerCase().includes(q))
  )
})

function schemaFields(s: Schema): string[] {
  const props = (s.schema as any)?.properties
  return props ? Object.keys(props) : []
}

function formatTime(ts: string) {
  if (!ts) return ''
  return new Date(ts).toLocaleDateString()
}

// ─── Catalog Tab ───
const catalogTemplates = ref<CatalogTemplate[]>([])
const catalogSearch = ref('')
const catalogTypeFilter = ref('')
const catalogLoading = ref(false)
const refreshing = ref(false)
const showInstallDialog = ref(false)
const installTemplateId = ref('')

const catalogTypes = computed(() => {
  const types = new Set(catalogTemplates.value.map(t => t.type))
  return Array.from(types).sort()
})

const filteredCatalog = computed(() => {
  let result = catalogTemplates.value
  if (catalogTypeFilter.value) {
    result = result.filter(t => t.type === catalogTypeFilter.value)
  }
  if (catalogSearch.value) {
    const q = catalogSearch.value.toLowerCase()
    result = result.filter(t =>
      t.name.toLowerCase().includes(q) ||
      t.description?.toLowerCase().includes(q) ||
      t.tags?.some(tag => tag.toLowerCase().includes(q))
    )
  }
  return result
})

function openInstall(templateId: string) {
  installTemplateId.value = templateId
  showInstallDialog.value = true
}

function onInstalled(_result: { id: string; template_id: string; type: string }) {
  // Stay on schemas page — the toast confirms success.
  // User can find the new entity in its type-specific view (Actions, Users, etc.)
  activeTab.value = 'schemas'
  // Reload schemas to reflect any new schema types
  schemaApi.list().then(s => { allSchemas.value = s }).catch(() => {})
}

async function refreshCatalog() {
  refreshing.value = true
  try {
    const res = await catalogApi.refresh()
    toast.success('Catalog refreshed', { description: `${res.new} new templates` })
    catalogTemplates.value = await catalogApi.list()
  } catch (e: any) {
    toast.error('Refresh failed', { description: e.message })
  } finally {
    refreshing.value = false
  }
}

// ─── Create New Schema Type ───
const showCreateDialog = ref(false)
const newTypeName = ref('')
const creatingType = ref(false)

async function createSchemaType() {
  const typeName = newTypeName.value.trim().toLowerCase().replace(/\s+/g, '_').replace(/[^a-z0-9_]/g, '')
  if (!typeName) return

  creatingType.value = true
  try {
    const result = await schemaApi.update('new', {
      type: typeName,
      properties: {},
    }, `Initial ${typeName} schema`)
    showCreateDialog.value = false
    newTypeName.value = ''
    toast.success(`Schema type "${typeName}" created`)
    router.push('/schemas/' + result.id)
  } catch (e: any) {
    toast.error('Create failed', { description: e.message })
  } finally {
    creatingType.value = false
  }
}

// ─── Init ───
onMounted(async () => {
  try { allSchemas.value = await schemaApi.list() } catch {}

  catalogLoading.value = true
  try { catalogTemplates.value = await catalogApi.list() } catch {}
  catalogLoading.value = false
})
</script>
