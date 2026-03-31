<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Authorization Overview</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Manage who has access to organizations, groups, and projects.
        </p>
      </div>
      <Badge variant="outline" class="gap-1.5">
        <ShieldCheck class="size-3" />
        ReBAC
      </Badge>
    </div>

    <!-- Quick check card -->
    <Card class="p-4">
      <h3 class="font-medium mb-3">Quick Permission Check</h3>
      <div class="grid grid-cols-3 gap-3">
        <div>
          <label class="text-xs font-medium text-muted-foreground">User</label>
          <Input v-model="checkForm.user" placeholder="user:admin" class="mt-1" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground">Relation</label>
          <Input v-model="checkForm.relation" placeholder="member" class="mt-1" />
        </div>
        <div>
          <label class="text-xs font-medium text-muted-foreground">Object</label>
          <Input v-model="checkForm.object" placeholder="org:1" class="mt-1" />
        </div>
      </div>
      <div class="flex items-center gap-2 mt-3">
        <Button size="sm" :disabled="!checkFormValid" @click="runCheck">
          <Play class="size-3.5 mr-1" /> Check
        </Button>
        <Button variant="ghost" size="sm" class="text-xs" @click="$router.push('/authorization/permissions')">
          Open Full Playground →
        </Button>
      </div>
      <div v-if="checkResult !== null" class="mt-3 rounded-lg p-3" :class="checkResult ? 'bg-green-50 dark:bg-green-950/30 border border-green-200 dark:border-green-800' : 'bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800'">
        <div class="flex items-center gap-2">
          <CheckCircle2 v-if="checkResult" class="size-4 text-green-600" />
          <XCircle v-else class="size-4 text-red-600" />
          <span class="text-sm font-medium" :class="checkResult ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300'">
            {{ checkResult ? 'Allowed' : 'Denied' }}
          </span>
        </div>
      </div>
    </Card>

    <!-- Resource access summary cards -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <Card class="p-4 space-y-3">
        <div class="flex items-center gap-2">
          <div class="p-2 rounded-lg bg-primary/10">
            <Building2 class="size-4 text-primary" />
          </div>
          <div>
            <h4 class="font-medium text-sm">Organizations</h4>
            <p class="text-xs text-muted-foreground">{{ orgCount }} total</p>
          </div>
        </div>
        <p class="text-xs text-muted-foreground">
          Top-level tenants. Members inherit access to nested groups and projects.
        </p>
        <Button variant="outline" size="sm" class="w-full" @click="$router.push('/orgs')">
          Manage Orgs
        </Button>
      </Card>

      <Card class="p-4 space-y-3">
        <div class="flex items-center gap-2">
          <div class="p-2 rounded-lg bg-primary/10">
            <UsersRound class="size-4 text-primary" />
          </div>
          <div>
            <h4 class="font-medium text-sm">Groups</h4>
            <p class="text-xs text-muted-foreground">{{ groupCount }} total</p>
          </div>
        </div>
        <p class="text-xs text-muted-foreground">
          Bundle users for shared access. SCIM-compatible for directory sync.
        </p>
        <Button variant="outline" size="sm" class="w-full" @click="$router.push('/groups')">
          Manage Groups
        </Button>
      </Card>

      <Card class="p-4 space-y-3">
        <div class="flex items-center gap-2">
          <div class="p-2 rounded-lg bg-primary/10">
            <FolderKanban class="size-4 text-primary" />
          </div>
          <div>
            <h4 class="font-medium text-sm">Projects</h4>
            <p class="text-xs text-muted-foreground">{{ projectCount }} total</p>
          </div>
        </div>
        <p class="text-xs text-muted-foreground">
          Container for apps, roles, and grants with scoped member access.
        </p>
        <Button variant="outline" size="sm" class="w-full" @click="$router.push('/projects')">
          Manage Projects
        </Button>
      </Card>
    </div>

    <!-- Recent tuples -->
    <Card>
      <div class="p-4 pb-2 flex items-center justify-between">
        <div>
          <h3 class="font-medium">Recent Relationships</h3>
          <p class="text-xs text-muted-foreground mt-0.5">Latest authorization tuples.</p>
        </div>
        <Button variant="ghost" size="sm" class="text-xs" @click="$router.push('/authorization/relationships')">
          View All →
        </Button>
      </div>
      <div class="border-t">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b bg-muted/50">
              <th class="h-10 px-4 text-left font-medium text-muted-foreground">User</th>
              <th class="h-10 px-4 text-left font-medium text-muted-foreground">Relation</th>
              <th class="h-10 px-4 text-left font-medium text-muted-foreground">Object</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loadingTuples" class="border-b">
              <td colspan="3" class="px-4 py-6 text-center text-muted-foreground text-sm">Loading…</td>
            </tr>
            <tr v-else-if="!recentTuples.length" class="border-b">
              <td colspan="3" class="px-4 py-6 text-center text-muted-foreground text-sm">No tuples yet.</td>
            </tr>
            <tr
              v-for="(t, i) in recentTuples.slice(0, 5)" :key="i"
              class="border-b last:border-0 hover:bg-muted/50 transition-colors"
            >
              <td class="p-4 font-mono text-xs">{{ t.user }}</td>
              <td class="p-4">
                <Badge variant="secondary" class="text-xs">{{ t.relation }}</Badge>
              </td>
              <td class="p-4 font-mono text-xs">{{ t.object }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { fgaApi, groupApi, projectApi, orgApi } from '@/api/resources'
import type { FGATuple } from '@/api/resources'

import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  ShieldCheck, Building2, UsersRound, FolderKanban,
  Play, CheckCircle2, XCircle,
} from 'lucide-vue-next'

const orgCount = ref(0)
const groupCount = ref(0)
const projectCount = ref(0)

const checkForm = reactive({ user: '', relation: '', object: '' })
const checkFormValid = computed(() => checkForm.user && checkForm.relation && checkForm.object)
const checkResult = ref<boolean | null>(null)

async function runCheck() {
  try {
    const result = await fgaApi.check(checkForm.user, checkForm.relation, checkForm.object)
    checkResult.value = !!(result as any)?.allowed
  } catch {
    checkResult.value = false
  }
}

const loadingTuples = ref(false)
const recentTuples = ref<FGATuple[]>([])

onMounted(async () => {
  const [orgs, groups, projects] = await Promise.all([
    orgApi.list().catch(() => []),
    groupApi.list().catch(() => []),
    projectApi.list().catch(() => []),
  ])
  orgCount.value = orgs.length
  groupCount.value = groups.length
  projectCount.value = projects.length

  loadingTuples.value = true
  try {
    recentTuples.value = await fgaApi.readTuples()
  } catch { /* FGA might not be running */ }
  loadingTuples.value = false
})
</script>
