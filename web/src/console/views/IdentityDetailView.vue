<template>
  <div v-if="identity" class="space-y-6">
    <!-- Header -->
    <div class="flex items-center gap-4">
      <Avatar class="size-12 rounded-xl">
        <AvatarFallback class="rounded-xl bg-primary text-primary-foreground text-lg font-bold">
          {{ (identity.display_name || identity.identifier)[0]?.toUpperCase() }}
        </AvatarFallback>
      </Avatar>
      <div class="flex-1">
        <h1 class="text-2xl font-semibold tracking-tight">{{ identity.display_name || identity.identifier }}</h1>
        <p class="text-sm text-muted-foreground">
          {{ identity.identifier }} ·
          <Badge
            :variant="identity.state === 'active' ? 'default' : identity.state === 'locked' ? 'secondary' : 'destructive'"
            class="text-xs ml-1"
          >{{ identity.state }}</Badge>
        </p>
      </div>
      <div class="flex gap-2">
        <Button v-if="!editing && isInteractiveIdentity" variant="outline" size="sm" @click="sendInviteLink" :disabled="inviting">
          <Mail class="mr-2 size-4" />
          {{ inviting ? 'Sending…' : 'Invite' }}
        </Button>
        <Button v-if="!editing" variant="outline" size="sm" @click="startEdit">
          <Pencil class="mr-2 size-4" />
          Edit
        </Button>
        <Button v-if="!editing" variant="destructive" size="sm" @click="showDeleteConfirm = true">
          <Trash2 class="mr-2 size-4" />
          Delete
        </Button>
        <template v-if="editing">
          <Button size="sm" @click="save" :disabled="saving">
            {{ saving ? 'Saving…' : '✓ Save' }}
          </Button>
          <Button variant="outline" size="sm" @click="cancelEdit">Cancel</Button>
        </template>
      </div>
    </div>

    <!-- Delete Confirmation Dialog -->
    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete {{ displayMeta.singular || 'Entity' }}</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ identity.identifier }}</strong>? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" @click="deleteIdentity" :disabled="deleting">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Message banner -->
    <div v-if="message" :class="[
      'rounded-lg border px-4 py-3 text-sm',
      messageType === 'success' ? 'border-green-200 bg-green-50 text-green-700' :
      messageType === 'invite' ? 'border-blue-200 bg-blue-50 text-blue-700' :
      'border-destructive/50 bg-destructive/10 text-destructive'
    ]">{{ message }}</div>

    <!-- Mode tabs -->
    <div class="inline-flex items-center rounded-lg bg-muted p-1">
      <button
        :class="['px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
          viewMode === 'form' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground']"
        @click="viewMode = 'form'"
      >📝 Form</button>
      <button
        :class="['px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
          viewMode === 'json' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground']"
        @click="switchToJson"
      >{ } JSON</button>
    </div>

    <!-- ═══ FORM VIEW ═══ -->
    <template v-if="viewMode === 'form' && !editing">
      <div class="grid grid-cols-2 gap-4">
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Profile</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="space-y-3" v-if="Object.keys(profileFields).length">
              <div class="flex gap-4" v-for="(val, key) in profileFields" :key="key">
                <span class="text-sm font-medium text-muted-foreground w-28 shrink-0">{{ formatKey(key as string) }}</span>
                <span class="text-sm break-all">{{ formatValue(val) }}</span>
              </div>
            </div>
            <p v-else class="text-sm text-muted-foreground">No profile data</p>
          </CardContent>
        </Card>
        <Card v-if="isInteractiveIdentity">
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Capabilities</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="flex flex-wrap gap-2" v-if="identity.capabilities?.length">
              <Badge v-for="cap in identity.capabilities" :key="cap" variant="outline" class="text-xs">{{ cap }}</Badge>
            </div>
            <p v-else class="text-sm text-muted-foreground">No capabilities</p>
          </CardContent>
        </Card>
      </div>
      <div class="grid grid-cols-2 gap-4">
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Metadata</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="space-y-3" v-if="Object.keys(metaFields).length">
              <div class="flex gap-4" v-for="(val, key) in metaFields" :key="key">
                <span class="text-sm font-medium text-muted-foreground w-28 shrink-0">{{ formatKey(key as string) }}</span>
                <span class="text-sm">{{ val }}</span>
              </div>
            </div>
            <p v-else class="text-sm text-muted-foreground">No metadata</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Details</CardTitle>
          </CardHeader>
          <CardContent>
            <dl class="grid grid-cols-[80px_1fr] gap-y-2 gap-x-4 text-sm">
              <dt class="font-medium text-muted-foreground">ID</dt>
              <dd class="font-mono text-xs break-all">{{ identity.id }}</dd>
              <dt class="font-medium text-muted-foreground">Org ID</dt>
              <dd>{{ identity.org_id }}</dd>
              <dt class="font-medium text-muted-foreground">Schema</dt>
              <dd>{{ identity.schema_name || '—' }}</dd>
              <dt class="font-medium text-muted-foreground">Created</dt>
              <dd>{{ formatTime(identity.created_at) }}</dd>
              <dt class="font-medium text-muted-foreground">Updated</dt>
              <dd>{{ formatTime(identity.updated_at) }}</dd>
            </dl>
          </CardContent>
        </Card>
      </div>
    </template>

    <!-- ═══ JSON VIEW ═══ -->
    <template v-if="viewMode === 'json' && !editing">
      <Card>
        <CardContent class="pt-6">
          <JsonEditor
            :modelValue="entityJsonReadonly"
            label="Stored Entity (read-only)"
            :schema="entitySchema"
            height="480px"
          />
          <p class="text-xs text-muted-foreground mt-2">This is the raw entity data as stored. Click <strong>Edit</strong> to modify.</p>
        </CardContent>
      </Card>
    </template>

    <!-- ═══ FORM EDIT ═══ -->
    <template v-if="editing && viewMode === 'form'">
      <div class="max-w-xl space-y-4">
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Account</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="space-y-2">
              <Label for="edit-name">Display Name</Label>
              <Input id="edit-name" v-model="editForm.display_name" />
            </div>
            <div class="space-y-2">
              <Label for="edit-state">State</Label>
              <Select v-model="editForm.state">
                <SelectTrigger id="edit-state"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="active">Active</SelectItem>
                  <SelectItem value="deactivated">Deactivated</SelectItem>
                  <SelectItem value="locked">Locked</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Profile</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="space-y-2" v-for="(val, key) in editForm.profile" :key="key">
              <div class="flex items-center justify-between">
                <Label>{{ formatKey(key as string) }}</Label>
                <Button variant="ghost" size="icon" class="size-6 text-destructive" @click="removeProfileField(key as string)">
                  <X class="size-3" />
                </Button>
              </div>
              <Input v-model="editForm.profile[key as string]" />
            </div>
            <Separator />
            <div class="flex gap-2">
              <Input v-model="newFieldName" placeholder="New field name" class="flex-1" />
              <Button variant="outline" size="sm" @click="addProfileField">+ Add</Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </template>

    <!-- ═══ JSON EDIT ═══ -->
    <template v-if="editing && viewMode === 'json'">
      <Card>
        <CardContent class="pt-6">
          <JsonEditor
            v-model="editJsonContent"
            label="Edit Entity JSON"
            :schema="entitySchema"
            height="480px"
            @valid="onEditJsonValid"
            @error="onEditJsonError"
          />
          <div v-if="editJsonError" class="mt-2 rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive">{{ editJsonError }}</div>
        </CardContent>
      </Card>
    </template>

    <!-- ═══ RELATED ACTIVITY ═══ -->
    <template v-if="!editing">
      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-sm flex items-center gap-2">
            <Activity class="size-4 text-muted-foreground" />
            Related Activity
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
            <RouterLink
              :to="{ path: '/events', query: { actor: identity?.id } }"
              class="flex items-center gap-3 p-3 rounded-lg border hover:bg-muted/50 transition-colors group cursor-pointer"
            >
              <div class="p-2 rounded-md bg-primary/10 text-primary">
                <FileJson class="size-4" />
              </div>
              <div>
                <p class="text-sm font-medium group-hover:text-primary transition-colors">Events</p>
                <p class="text-xs text-muted-foreground">Activity by this actor</p>
              </div>
              <ExternalLink class="size-3 ml-auto opacity-0 group-hover:opacity-50 transition-opacity" />
            </RouterLink>

            <RouterLink
              :to="{ path: '/sessions', query: { user: identity?.id } }"
              class="flex items-center gap-3 p-3 rounded-lg border hover:bg-muted/50 transition-colors group cursor-pointer"
            >
              <div class="p-2 rounded-md bg-green-500/10 text-green-600">
                <Key class="size-4" />
              </div>
              <div>
                <p class="text-sm font-medium group-hover:text-primary transition-colors">Sessions</p>
                <p class="text-xs text-muted-foreground">Active sessions for this entity</p>
              </div>
              <ExternalLink class="size-3 ml-auto opacity-0 group-hover:opacity-50 transition-opacity" />
            </RouterLink>

            <RouterLink
              :to="{ path: '/traces', query: { id: identity?.id } }"
              class="flex items-center gap-3 p-3 rounded-lg border hover:bg-muted/50 transition-colors group"
            >
              <div class="p-2 rounded-md bg-amber-500/10 text-amber-600">
                <Activity class="size-4" />
              </div>
              <div>
                <p class="text-sm font-medium group-hover:text-primary transition-colors">Traces</p>
                <p class="text-xs text-muted-foreground">Distributed trace chains</p>
              </div>
              <ExternalLink class="size-3 ml-auto opacity-0 group-hover:opacity-50 transition-opacity" />
            </RouterLink>
          </div>
        </CardContent>
      </Card>
    </template>

    <Button variant="link" as-child class="px-0 text-muted-foreground">
      <router-link :to="backRoute">← Back to {{ displayMeta.alias || 'list' }}</router-link>
    </Button>
  </div>
  <div v-else class="flex h-48 items-center justify-center text-muted-foreground">Loading...</div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue'
import { useRoute, useRouter, RouterLink } from 'vue-router'
import { entityApi, magicLinkApi, schemaApi, metaSchemaApi, type Identity } from '@/api/resources'
import JsonEditor from '@/console/components/JsonEditor.vue'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { ArrowLeft, Mail, Pencil, Trash2, X, Activity, FileJson, Key, ExternalLink } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()
const identity = ref<Identity | null>(null)
const editing = ref(false)
const saving = ref(false)
const deleting = ref(false)
const showDeleteConfirm = ref(false)
const inviting = ref(false)
const message = ref('')
const messageType = ref<'success' | 'error' | 'invite'>('success')
const newFieldName = ref('')
const viewMode = ref<'form' | 'json'>('form')
const editJsonContent = ref('{}')
const editJsonError = ref('')
const editJsonParsed = ref<any>({})
const displayMeta = ref<any>({})
const entitySchema = ref<any>(null)

const schemaType = computed(() => (route.params as any).schemaType || identity.value?.schema_name || '')
const isInteractiveIdentity = computed(() => {
  if (!entitySchema.value) return true
  return !!(entitySchema.value['x-identifier'] || entitySchema.value['x-auth-methods'])
})
const backRoute = computed(() => schemaType.value ? `/s/${schemaType.value}` : '/')

const editForm = reactive({
  display_name: '',
  state: '',
  profile: {} as Record<string, string>,
})

const profileFields = computed(() => {
  const p = identity.value?.profile
  return (p && typeof p === 'object') ? p as Record<string, unknown> : {}
})

const metaFields = computed(() => {
  const m = identity.value?.metadata
  return (m && typeof m === 'object') ? m as Record<string, unknown> : {}
})

const entityJsonReadonly = computed(() => {
  if (!identity.value) return '{}'
  return JSON.stringify(identity.value, null, 2)
})

function formatKey(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}
function formatValue(val: unknown): string {
  if (val === null || val === undefined) return '—'
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}
function formatTime(ts: string) { return new Date(ts).toLocaleString() }

function switchToJson() {
  if (editing.value) {
    const data: any = { ...identity.value }
    data.display_name = editForm.display_name
    data.state = editForm.state
    data.profile = { ...editForm.profile }
    editJsonContent.value = JSON.stringify(data, null, 2)
  }
  viewMode.value = 'json'
}

function startEdit() {
  if (!identity.value) return
  editForm.display_name = identity.value.display_name || ''
  editForm.state = identity.value.state
  const p = identity.value.profile || {}
  editForm.profile = {}
  for (const [k, v] of Object.entries(p)) editForm.profile[k] = String(v ?? '')
  editJsonContent.value = JSON.stringify(identity.value, null, 2)
  editing.value = true
  message.value = ''
}

function cancelEdit() { editing.value = false; message.value = '' }
function addProfileField() {
  const name = newFieldName.value.trim()
  if (name && !(name in editForm.profile)) { editForm.profile[name] = ''; newFieldName.value = '' }
}
function removeProfileField(key: string) { delete editForm.profile[key] }
function onEditJsonValid(parsed: any) { editJsonError.value = ''; editJsonParsed.value = parsed }
function onEditJsonError(msg: string) { editJsonError.value = msg }

async function save() {
  if (!identity.value) return
  saving.value = true; message.value = ''
  try {
    let payload: any
    if (viewMode.value === 'json') {
      const data = editJsonParsed.value
      payload = { display_name: data.display_name || editForm.display_name, state: data.state || editForm.state, profile: data.profile || {} }
    } else {
      const profile: Record<string, string> = {}
      for (const [k, v] of Object.entries(editForm.profile)) { if (v.trim()) profile[k] = v.trim() }
      payload = { display_name: editForm.display_name.trim(), state: editForm.state, profile }
    }
    await entityApi.update(identity.value.id, payload as any)
    identity.value = await entityApi.get(route.params.id as string)
    editing.value = false
    message.value = 'Updated successfully'; messageType.value = 'success'
  } catch (e: any) {
    message.value = e?.message || 'Update failed'; messageType.value = 'error'
  } finally { saving.value = false }
}

async function sendInviteLink() {
  if (!identity.value) return
  inviting.value = true; message.value = ''
  try {
    const resp = await magicLinkApi.send(identity.value.identifier)
    message.value = resp.purpose === 'register' ? 'Registration invite sent — check server logs.' : 'Login link sent — check server logs.'
    messageType.value = 'invite'
  } catch (e: any) {
    message.value = e?.message || 'Failed to send invite'; messageType.value = 'error'
  } finally { inviting.value = false }
}

async function deleteIdentity() {
  if (!identity.value) return
  deleting.value = true
  try {
    await entityApi.delete(identity.value.id)
    router.push(backRoute.value)
  } catch (e: any) {
    showDeleteConfirm.value = false
    message.value = e?.message || 'Delete failed'; messageType.value = 'error'
    deleting.value = false
  }
}

onMounted(async () => {
  try {
    identity.value = await entityApi.get(route.params.id as string)
    if (identity.value?.schema_name) {
      const allSchemas = await schemaApi.list()
      const match = allSchemas.find((s: any) => s.type === identity.value!.schema_name && s.is_default)
        || allSchemas.find((s: any) => s.type === identity.value!.schema_name)
      if (match) entitySchema.value = match.schema
    }
    try {
      const metaData = await metaSchemaApi.get()
      const st = identity.value?.schema_name
      if (st) {
        const entry = (metaData['x-catalog'] || {})[st]
        if (entry) displayMeta.value = { singular: entry.singular, alias: entry.alias, path: entry.path, icon: entry.icon }
      }
    } catch { /* ignore */ }
  } catch {}
})
</script>
