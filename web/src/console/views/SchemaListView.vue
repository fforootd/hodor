<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Schemas</h1>
        <p class="text-sm text-muted-foreground mt-1">Schema types, version history, and definitions.</p>
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

    <!-- Search bar -->
    <div class="flex items-center gap-3">
      <div class="relative w-full max-w-sm">
        <SearchIcon class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
        <Input v-model="searchQuery" placeholder="Filter schemas…" class="pl-9 bg-background" />
      </div>
      <span class="text-xs text-muted-foreground">
        {{ filteredGroups.length }} type{{ filteredGroups.length !== 1 ? 's' : '' }}
      </span>
      <router-link to="/marketplace" class="ml-auto no-underline">
        <Button variant="outline" size="sm" class="gap-1.5 text-xs">
          <Store class="size-3.5" />
          Browse Marketplace
        </Button>
      </router-link>
    </div>

    <!-- Schema type cards -->
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
      <router-link to="/marketplace" class="no-underline">
        <Button variant="outline" size="sm" class="gap-1.5 mt-1">
          <Store class="size-3.5" />
          Browse Marketplace
        </Button>
      </router-link>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { schemaApi, type Schema } from '@/api/resources'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog'
import {
  Search as SearchIcon, Plus, FileJson, Store,
} from 'lucide-vue-next'

const router = useRouter()

// ─── Schemas ───
const allSchemas = ref<Schema[]>([])
const searchQuery = ref('')

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
})
</script>
