<template>
  <div v-if="app" class="space-y-6">
    <!-- Header -->
    <div class="flex items-center gap-4">
      <Avatar class="size-12 rounded-xl">
        <AvatarFallback class="rounded-xl bg-primary text-primary-foreground text-lg font-bold">
          {{ (app.name || app.client_id)[0]?.toUpperCase() }}
        </AvatarFallback>
      </Avatar>
      <div class="flex-1">
        <h1 class="text-2xl font-semibold tracking-tight">{{ app.name }}</h1>
        <p class="text-sm text-muted-foreground">
          <code class="font-mono text-xs bg-muted px-1.5 py-0.5 rounded">{{ app.client_id }}</code> ·
          <Badge
            :variant="app.state === 'active' ? 'default' : 'destructive'"
            class="text-xs ml-1"
          >{{ app.state }}</Badge>
        </p>
      </div>
      <div class="flex gap-2">
        <Button v-if="!editing" variant="outline" size="sm" @click="startEdit">
          <Pencil class="mr-2 size-4" /> Edit
        </Button>
        <Button v-if="!editing" variant="destructive" size="sm" @click="showDeleteConfirm = true">
          <Trash2 class="mr-2 size-4" /> Delete
        </Button>
        <template v-if="editing">
          <Button size="sm" @click="save" :disabled="saving">
            {{ saving ? 'Saving…' : '✓ Save' }}
          </Button>
          <Button variant="outline" size="sm" @click="cancelEdit">Cancel</Button>
        </template>
      </div>
    </div>

    <!-- Delete Confirmation -->
    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Application</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ app.name }}</strong>? This cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" @click="deleteApp" :disabled="deleting">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>



    <!-- Read view -->
    <template v-if="!editing">
      <div class="grid grid-cols-2 gap-4">
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">OIDC Settings</CardTitle>
          </CardHeader>
          <CardContent>
            <dl class="space-y-3">
              <div class="flex gap-4">
                <dt class="text-sm font-medium text-muted-foreground w-28 shrink-0">Client ID</dt>
                <dd class="text-sm font-mono break-all cursor-pointer hover:text-primary" @click="copy(app.client_id)">{{ app.client_id }}</dd>
              </div>
              <div class="flex gap-4">
                <dt class="text-sm font-medium text-muted-foreground w-28 shrink-0">App Type</dt>
                <dd class="text-sm uppercase">{{ app.app_type }}</dd>
              </div>
              <div class="flex gap-4">
                <dt class="text-sm font-medium text-muted-foreground w-28 shrink-0">Grant Types</dt>
                <dd class="flex flex-wrap gap-1">
                  <Badge v-for="gt in (app.grant_types || [])" :key="gt" variant="outline" class="text-xs">{{ gt }}</Badge>
                </dd>
              </div>
              <div class="flex gap-4">
                <dt class="text-sm font-medium text-muted-foreground w-28 shrink-0">Response Types</dt>
                <dd class="flex flex-wrap gap-1">
                  <Badge v-for="rt in (app.response_types || [])" :key="rt" variant="outline" class="text-xs">{{ rt }}</Badge>
                </dd>
              </div>
            </dl>
          </CardContent>
        </Card>

        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Details</CardTitle>
          </CardHeader>
          <CardContent>
            <dl class="grid grid-cols-[80px_1fr] gap-y-2 gap-x-4 text-sm">
              <dt class="font-medium text-muted-foreground">ID</dt>
              <dd class="font-mono text-xs break-all">{{ app.id }}</dd>
              <dt class="font-medium text-muted-foreground">Org ID</dt>
              <dd>{{ app.org_id || '—' }}</dd>
              <dt class="font-medium text-muted-foreground">Created</dt>
              <dd>{{ formatDateTime(app.created_at) }}</dd>
              <dt class="font-medium text-muted-foreground">Updated</dt>
              <dd>{{ formatDateTime(app.updated_at) }}</dd>
            </dl>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-sm">Redirect URIs</CardTitle>
        </CardHeader>
        <CardContent>
          <div v-if="(app.redirect_uris || []).length > 0" class="space-y-1">
            <code v-for="(uri, idx) in app.redirect_uris" :key="idx"
              class="block text-sm font-mono rounded bg-muted px-2 py-1.5">{{ uri }}</code>
          </div>
          <p v-else class="text-sm text-muted-foreground">No redirect URIs configured.</p>
        </CardContent>
      </Card>
    </template>

    <!-- Edit view -->
    <template v-if="editing">
      <div class="max-w-xl space-y-4">
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Application</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="space-y-2">
              <Label for="edit-name">Name</Label>
              <Input id="edit-name" v-model="editForm.name" />
            </div>
            <div class="space-y-2">
              <Label for="edit-state">State</Label>
              <Select v-model="editForm.state">
                <SelectTrigger id="edit-state"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="active">Active</SelectItem>
                  <SelectItem value="deactivated">Deactivated</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Redirect URIs</CardTitle>
          </CardHeader>
          <CardContent class="space-y-3">
            <div v-for="(uri, idx) in editForm.redirect_uris" :key="idx" class="flex gap-2">
              <Input v-model="editForm.redirect_uris[idx]" class="font-mono text-sm" />
              <Button variant="ghost" size="icon" class="shrink-0 text-destructive" @click="editForm.redirect_uris.splice(idx, 1)">
                <X class="size-4" />
              </Button>
            </div>
            <Button variant="outline" size="sm" @click="editForm.redirect_uris.push('')">
              <Plus class="mr-2 size-3" /> Add URI
            </Button>
          </CardContent>
        </Card>
      </div>
    </template>

    <Button variant="link" as-child class="px-0 text-muted-foreground">
      <router-link to="/applications">← Back to Applications</router-link>
    </Button>
  </div>
  <div v-else class="flex h-48 items-center justify-center text-muted-foreground">Loading...</div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { appApi, type App } from '@/api/resources'
import { formatDateTime } from '@/console/utils/format'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { Pencil, Trash2, X, Plus } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()
const app = ref<App | null>(null)
const editing = ref(false)
const saving = ref(false)
const deleting = ref(false)
const showDeleteConfirm = ref(false)

const editForm = reactive({
  name: '',
  state: '',
  redirect_uris: [] as string[],
})

function copy(text: string) {
  navigator.clipboard.writeText(text)
  toast.success('Copied to clipboard')
}

function startEdit() {
  if (!app.value) return
  editForm.name = app.value.name
  editForm.state = app.value.state
  editForm.redirect_uris = [...(app.value.redirect_uris || [])]
  editing.value = true
}

function cancelEdit() { editing.value = false }

async function save() {
  if (!app.value) return
  saving.value = true
  try {
    const payload: any = {
      name: editForm.name.trim(),
      state: editForm.state,
      redirect_uris: editForm.redirect_uris.filter(u => u.trim()),
    }
    await appApi.update(app.value.id, payload)
    app.value = await appApi.get(route.params.id as string)
    editing.value = false
    toast.success('Application updated')
  } catch (e: any) {
    toast.error('Failed to update application', { description: e?.message })
  } finally { saving.value = false }
}

async function deleteApp() {
  if (!app.value) return
  deleting.value = true
  try {
    await appApi.delete(app.value.id)
    toast.success('Application deleted')
    router.push('/applications')
  } catch (e: any) {
    showDeleteConfirm.value = false
    toast.error('Failed to delete application', { description: e?.message })
    deleting.value = false
  }
}

onMounted(async () => {
  try {
    app.value = await appApi.get(route.params.id as string)
  } catch {}
})
</script>
