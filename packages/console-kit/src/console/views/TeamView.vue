<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Team</h1>
      <p class="text-sm text-muted-foreground">Manage who can access and manage your instances.</p>
    </div>

    <Card>
      <CardContent class="pt-6">
        <div class="flex items-center justify-between mb-4">
          <p class="text-sm text-muted-foreground">{{ members.length }} member{{ members.length !== 1 ? 's' : '' }}</p>
          <Button size="sm" disabled>
            <Plus class="mr-1 size-4" /> Invite Member
          </Button>
        </div>

        <div class="divide-y">
          <div
            v-for="member in members"
            :key="member.user_id"
            class="flex items-center justify-between py-3"
          >
            <div class="flex items-center gap-3">
              <Avatar class="size-8">
                <AvatarFallback>{{ (member.display_name || 'U')[0].toUpperCase() }}</AvatarFallback>
              </Avatar>
              <div>
                <p class="text-sm font-medium">{{ member.display_name || member.user_id }}</p>
                <p class="text-xs text-muted-foreground">{{ member.role }}</p>
              </div>
            </div>
          </div>
          <div v-if="members.length === 0 && !loading" class="py-8 text-center text-sm text-muted-foreground">
            No team members yet.
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { orgMembersApi, type OrgMember } from '@/api/resources'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Plus } from 'lucide-vue-next'

const { currentOrgId } = useOrgContext()
const members = ref<OrgMember[]>([])
const loading = ref(true)

onMounted(async () => {
  if (currentOrgId.value) {
    try {
      members.value = await orgMembersApi.list(currentOrgId.value)
    } catch {
      // Org may not have members API yet.
    }
  }
  loading.value = false
})
</script>
