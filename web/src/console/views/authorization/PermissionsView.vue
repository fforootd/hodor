<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Permissions</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Test whether a user has a specific permission on an object.
        </p>
      </div>
      <Badge variant="outline" class="gap-1.5">
        <Shield class="size-3" />
        OpenFGA v1.1
      </Badge>
    </div>

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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive } from 'vue'
import { fgaApi, type FGACheckResult } from '@/api/resources'
import { toast } from 'vue-sonner'

import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Shield, Play, CheckCircle2, XCircle, GitBranch,
} from 'lucide-vue-next'

const checkForm = reactive({ user: '', relation: '', object: '' })
const checkFormValid = computed(() => checkForm.user && checkForm.relation && checkForm.object)
const checkRunning = ref(false)
const checkResult = ref<FGACheckResult | null>(null)
const expandResult = ref<any>(null)
const checkHistory = ref<FGACheckResult[]>([])

const presets = [
  { label: 'Admin → manage orgs', user: 'user:admin', relation: 'can_manage_orgs', object: 'instance:default' },
  { label: 'Admin → create resource', user: 'user:admin', relation: 'can_create_resource', object: 'org:1' },
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
</script>
