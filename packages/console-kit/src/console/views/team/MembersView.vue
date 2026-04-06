<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Members</h1>
      <p class="text-sm text-muted-foreground">Platform users and their roles across organizations.</p>
    </div>

    <div v-if="loading" class="py-12 text-center text-sm text-muted-foreground">Loading members...</div>

    <template v-else>
      <Card v-for="group in orgGroups" :key="group.org.id">
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between">
            <div>
              <CardTitle class="text-base">{{ group.org.name }}</CardTitle>
              <CardDescription>{{ group.members.length }} member{{ group.members.length !== 1 ? 's' : '' }}</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>User</TableHead>
                <TableHead>Role</TableHead>
                <TableHead class="text-right">Added</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="member in group.members" :key="member.user_id">
                <TableCell>
                  <div class="flex items-center gap-3">
                    <Avatar class="size-7">
                      <AvatarFallback class="text-xs">{{ (member.display_name || 'U')[0].toUpperCase() }}</AvatarFallback>
                    </Avatar>
                    <span class="text-sm font-medium">{{ member.display_name || member.user_id }}</span>
                  </div>
                </TableCell>
                <TableCell>
                  <Badge variant="secondary">{{ member.role }}</Badge>
                </TableCell>
                <TableCell class="text-right text-sm text-muted-foreground">
                  {{ member.added_at ? new Date(member.added_at).toLocaleDateString() : '' }}
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <div v-if="group.members.length === 0" class="py-6 text-center text-sm text-muted-foreground">
            No members in this organization.
          </div>
        </CardContent>
      </Card>

      <div v-if="orgGroups.length === 0" class="py-12 text-center text-sm text-muted-foreground">
        No organizations found.
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { orgApi, orgMembersApi, type Org, type OrgMember } from '@/api/resources'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'

interface OrgGroup {
  org: Org
  members: OrgMember[]
}

const orgGroups = ref<OrgGroup[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    const orgs = await orgApi.list()
    const orgList = Array.isArray(orgs) ? orgs : (orgs as any).items ?? []

    const groups: OrgGroup[] = []
    for (const org of orgList) {
      try {
        const members = await orgMembersApi.list(org.id)
        groups.push({ org, members })
      } catch {
        groups.push({ org, members: [] })
      }
    }
    orgGroups.value = groups
  } catch {
    // API may not be available yet.
  }
  loading.value = false
})
</script>
