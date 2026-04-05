<template>
  <Card class="rounded-3xl shadow-sm">
    <CardHeader class="pb-3">
      <div class="flex items-center justify-between gap-4">
        <div>
          <CardTitle class="text-lg">Members</CardTitle>
          <p class="mt-1 text-sm text-muted-foreground">
            Manage memberships for this {{ resourceLabel.toLowerCase() }} without leaving the detail page.
          </p>
        </div>
        <Button variant="outline" size="sm" @click="showAddMember = true">
          <UserPlus class="mr-1 size-3.5" />
          Add member
        </Button>
      </div>
    </CardHeader>
    <CardContent>
      <div v-if="members.length" class="space-y-2">
        <div
          v-for="member in members"
          :key="member.user_id"
          class="flex items-center justify-between rounded-2xl border bg-muted/20 px-4 py-3"
        >
          <div class="space-y-1">
            <p class="text-sm font-medium">{{ member.display_name || member.user_id }}</p>
            <p class="text-xs text-muted-foreground">{{ member.role }}</p>
          </div>
          <Button
            variant="ghost"
            size="icon"
            class="size-8 text-muted-foreground hover:text-destructive"
            @click="$emit('remove', member.user_id)"
          >
            <Trash2 class="size-3.5" />
          </Button>
        </div>
      </div>
      <p v-else class="text-sm text-muted-foreground">
        No members yet. Add users to start collaborating in this {{ resourceLabel.toLowerCase() }}.
      </p>
    </CardContent>
  </Card>

  <Dialog v-model:open="showAddMember">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Add Member</DialogTitle>
        <DialogDescription>
          Add a user by ID to this {{ resourceLabel.toLowerCase() }}.
        </DialogDescription>
      </DialogHeader>
      <div class="space-y-2 py-2">
        <Label :for="`${resourceType}-member-user-id`">User ID</Label>
        <Input
          :id="`${resourceType}-member-user-id`"
          v-model="newMemberUserId"
          :name="`${resourceType}-member-user-id`"
          type="search"
          placeholder="user ID"
          autocomplete="off"
          autocapitalize="none"
          autocorrect="off"
          spellcheck="false"
          data-1p-ignore="true"
          data-lpignore="true"
        />
      </div>
      <DialogFooter class="gap-2">
        <Button variant="outline" @click="showAddMember = false">Cancel</Button>
        <Button :disabled="!newMemberUserId.trim()" @click="handleAddMember">
          <UserPlus class="mr-1 size-3.5" />
          Add
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Trash2, UserPlus } from 'lucide-vue-next'

interface MemberLike {
  user_id: string
  display_name?: string
  role: string
}

const props = defineProps<{
  members: MemberLike[]
  resourceLabel: string
  resourceType: string
}>()

const emit = defineEmits<{
  add: [userId: string]
  remove: [userId: string]
}>()

const showAddMember = ref(false)
const newMemberUserId = ref('')

function handleAddMember() {
  emit('add', newMemberUserId.value.trim())
  newMemberUserId.value = ''
  showAddMember.value = false
}
</script>
