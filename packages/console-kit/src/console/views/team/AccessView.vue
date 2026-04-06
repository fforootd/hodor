<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Access</h1>
      <p class="text-sm text-muted-foreground">Which organizations manage which instances, and who has access.</p>
    </div>

    <div v-if="loading" class="py-12 text-center text-sm text-muted-foreground">Loading access data...</div>

    <template v-else>
      <Card v-for="item in accessItems" :key="item.instance.instance_id">
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between">
            <div>
              <CardTitle class="text-base">{{ item.instance.primary_domain || item.instance.instance_id }}</CardTitle>
              <CardDescription>
                Owner: {{ item.ownerOrgName }}
              </CardDescription>
            </div>
            <div class="flex items-center gap-2">
              <Badge :variant="item.instance.state === 'active' ? 'default' : 'secondary'">
                {{ item.instance.state }}
              </Badge>
              <Badge variant="outline">{{ item.instance.placement_mode }}</Badge>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div v-if="item.accessEntries.length > 0">
            <p class="mb-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">Access via org membership</p>
            <div class="divide-y">
              <div
                v-for="entry in item.accessEntries"
                :key="entry.userId"
                class="flex items-center justify-between py-2"
              >
                <div class="flex items-center gap-3">
                  <Avatar class="size-6">
                    <AvatarFallback class="text-xs">{{ (entry.displayName || 'U')[0].toUpperCase() }}</AvatarFallback>
                  </Avatar>
                  <span class="text-sm">{{ entry.displayName || entry.userId }}</span>
                </div>
                <div class="flex items-center gap-2 text-sm text-muted-foreground">
                  <Badge variant="outline" class="text-xs">{{ entry.orgRole }}</Badge>
                  <span class="text-muted-foreground/50">&rarr;</span>
                  <Badge variant="secondary" class="text-xs">{{ entry.instanceAccess }}</Badge>
                </div>
              </div>
            </div>
          </div>
          <div v-else class="py-4 text-center text-sm text-muted-foreground">
            No members in the owning organization.
          </div>
        </CardContent>
      </Card>

      <div v-if="accessItems.length === 0" class="py-12 text-center text-sm text-muted-foreground">
        No child instances found. Create an instance to see access information.
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { instanceApi, orgApi, orgMembersApi, type Instance, type Org } from '@/api/resources'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'

interface AccessEntry {
  userId: string
  displayName: string
  orgRole: string
  instanceAccess: string
}

interface AccessItem {
  instance: Instance
  ownerOrgName: string
  accessEntries: AccessEntry[]
}

// Map org role to instance access level (mirrors the FGA hierarchy tuples).
function orgRoleToInstanceAccess(orgRole: string): string {
  switch (orgRole) {
    case 'owner':
      return 'admin'
    case 'admin':
      return 'admin'
    case 'member':
      return 'viewer'
    case 'viewer':
      return 'viewer'
    default:
      return 'none'
  }
}

const accessItems = ref<AccessItem[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    const instanceRes = await instanceApi.list({ limit: 100 })
    const instances = instanceRes.items ?? []

    const orgs = await orgApi.list()
    const orgList: Org[] = Array.isArray(orgs) ? orgs : (orgs as any).items ?? []
    const orgMap = new Map(orgList.map((o) => [o.id, o]))

    const items: AccessItem[] = []
    for (const instance of instances) {
      const ownerOrgId = (instance as any).owner_org_id
      const ownerOrg = ownerOrgId ? orgMap.get(ownerOrgId) : undefined
      const ownerOrgName = ownerOrg?.name || ownerOrgId || 'Unknown'

      let accessEntries: AccessEntry[] = []
      if (ownerOrgId) {
        try {
          const members = await orgMembersApi.list(ownerOrgId)
          accessEntries = members.map((m) => ({
            userId: m.user_id,
            displayName: m.display_name || '',
            orgRole: m.role,
            instanceAccess: orgRoleToInstanceAccess(m.role),
          }))
        } catch {
          // Org members API may not be available.
        }
      }

      items.push({ instance, ownerOrgName, accessEntries })
    }
    accessItems.value = items
  } catch {
    // API may not be available yet.
  }
  loading.value = false
})
</script>
