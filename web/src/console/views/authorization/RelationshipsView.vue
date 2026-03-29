<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Relationships</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Browse and manage authorization tuples that define who can access what.
        </p>
      </div>
      <div class="flex items-center gap-2">
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
    <div class="flex gap-3 items-end max-w-2xl">
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
    <div class="rounded-lg border overflow-hidden">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b bg-muted/50">
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">User</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Relation</th>
            <th class="h-10 px-4 text-left font-medium text-muted-foreground">Object</th>
            <th class="h-10 px-4 text-right font-medium text-muted-foreground">Actions</th>
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
            class="border-b last:border-0 hover:bg-muted/50 transition-colors"
          >
            <td class="p-4">
              <code class="text-xs bg-muted px-1.5 py-0.5 rounded">{{ tuple.user }}</code>
            </td>
            <td class="p-4">
              <Badge variant="secondary" class="text-xs">{{ tuple.relation }}</Badge>
            </td>
            <td class="p-4">
              <code class="text-xs bg-muted px-1.5 py-0.5 rounded">{{ tuple.object }}</code>
            </td>
            <td class="p-4 text-right">
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
      <div class="p-3 text-xs text-muted-foreground border-t">
        {{ tuples.length }} tuple{{ tuples.length !== 1 ? 's' : '' }} shown
      </div>
    </div>

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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { fgaApi, type FGATuple } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Plus, RefreshCw, Trash2, Search } from 'lucide-vue-next'

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

onMounted(fetchTuples)
</script>
