<template>
  <div v-if="resource" class="mx-auto max-w-2xl space-y-5">
    <!-- Header -->
    <div class="flex items-start justify-between gap-4">
      <div class="flex items-start gap-3">
        <Button variant="ghost" size="icon" as-child>
          <router-link :to="backRoute"><ArrowLeft class="size-4" /></router-link>
        </Button>
        <div class="flex items-start gap-3">
          <slot name="avatar">
            <Avatar v-if="showAvatar" class="size-12 rounded-xl">
              <AvatarFallback class="rounded-xl bg-primary text-primary-foreground text-lg font-bold">
                {{ initial }}
              </AvatarFallback>
            </Avatar>
          </slot>
          <div>
            <h1 class="text-2xl font-semibold tracking-tight">{{ displayTitle }}</h1>
            <div class="mt-1 flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
              <slot name="header-badges">
                <StateBadge :state="resource.state || 'active'" />
                <span v-if="resource.created_at">Created {{ formatDate(resource.created_at) }}</span>
              </slot>
            </div>
          </div>
        </div>
      </div>
      <div class="flex gap-2">
        <slot name="header-actions" />
        <Button variant="outline" size="sm" :disabled="saving || !jsonValid" @click="$emit('save')">
          {{ saving ? 'Saving…' : 'Save' }}
        </Button>
        <Button variant="destructive" size="sm" @click="showDeleteConfirm = true">Delete</Button>
      </div>
    </div>

    <!-- Schema Form -->
    <SchemaTabsEditor
      v-if="schemaContext?.schema"
      v-model="formData"
      :schema="schemaContext.schema"
      :curl-snippets="curlSnippets"
      :form-title="`${singularTitle} Fields`"
      @update:json-valid="(value) => $emit('update:jsonValid', value)"
    />
    <Card v-else>
      <CardContent class="flex items-center gap-2 pt-6 text-sm text-muted-foreground">
        <Spinner class="size-4" /> Loading schema…
      </CardContent>
    </Card>

    <!-- Members Card (optional) -->
    <Card v-if="showMembers">
      <CardHeader class="pb-3">
        <div class="flex items-center justify-between gap-4">
          <CardTitle class="text-sm">Members</CardTitle>
          <Button variant="outline" size="sm" @click="showAddMember = true">
            <UserPlus class="mr-1 size-3.5" /> Add Member
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div v-if="(members || []).length" class="space-y-2">
          <div
            v-for="member in members"
            :key="member.user_id"
            class="flex items-center justify-between rounded-lg border bg-muted/30 p-3"
          >
            <div>
              <p class="text-sm font-medium">{{ member.display_name || member.user_id }}</p>
              <p class="text-xs text-muted-foreground">{{ member.role }}</p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground hover:text-destructive"
              @click="$emit('remove-member', member.user_id)"
            >
              <Trash2 class="size-3.5" />
            </Button>
          </div>
        </div>
        <p v-else class="text-sm text-muted-foreground">
          No members yet. Add users to this {{ singularTitle.toLowerCase() }}.
        </p>
      </CardContent>
    </Card>

    <!-- Extra content slot -->
    <slot name="after-form" />

    <!-- System metadata -->
    <div class="flex flex-wrap gap-x-4 gap-y-1 border-t pt-3 text-xs text-muted-foreground">
      <span>ID: <code class="font-mono">{{ resource.id }}</code></span>
      <span v-if="resource.schema_id">Schema: {{ resource.schema_id }}</span>
      <span>Created {{ formatDateTime(resource.created_at) }}</span>
      <span>Updated {{ formatDateTime(resource.updated_at) }}</span>
    </div>

    <!-- Error -->
    <FormError :error="loadError" />

    <!-- Delete Dialog -->
    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete {{ singularTitle }}</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ displayTitle }}</strong>? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" :disabled="deleting" @click="$emit('delete')">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Add Member Dialog -->
    <Dialog v-if="showMembers" v-model:open="showAddMember">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add Member</DialogTitle>
          <DialogDescription>Add a user by ID to this {{ singularTitle.toLowerCase() }}.</DialogDescription>
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
            <UserPlus class="mr-1 size-3.5" /> Add
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>

  <!-- Loading state -->
  <div v-else class="flex h-48 items-center justify-center">
    <Spinner class="size-6 text-muted-foreground" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { ResourceSchemaContext } from '@/console/utils/schema-resource'
import { formatDate, formatDateTime } from '@/console/utils/format'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { StateBadge } from '@/components/ui/state-badge'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { FormError } from '@/components/ui/form-error'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'
import { ArrowLeft, Trash2, UserPlus } from 'lucide-vue-next'

interface MemberItem {
  user_id: string
  display_name?: string
  role: string
  added_at?: string
}

const props = defineProps<{
  resource: Record<string, any> | null
  resourceType: string
  singularTitle: string
  backRoute: string
  displayTitle: string
  schemaContext: ResourceSchemaContext | null
  curlSnippets: any[]
  // State
  saving: boolean
  deleting: boolean
  loadError: string
  jsonValid: boolean
  // Members
  showMembers?: boolean
  members?: MemberItem[]
  // Appearance
  showAvatar?: boolean
}>()

const formData = defineModel<Record<string, any>>('formData', { default: () => ({}) })

const emit = defineEmits<{
  save: []
  delete: []
  'remove-member': [userId: string]
  'add-member': [userId: string]
  'update:jsonValid': [value: boolean]
}>()

const showDeleteConfirm = ref(false)
const showAddMember = ref(false)
const newMemberUserId = ref('')

const initial = computed(() =>
  (props.displayTitle || '?').charAt(0).toUpperCase()
)

function handleAddMember() {
  emit('add-member', newMemberUserId.value.trim())
  newMemberUserId.value = ''
  showAddMember.value = false
}
</script>
