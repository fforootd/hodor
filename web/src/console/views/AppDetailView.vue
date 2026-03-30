<template>
  <div v-if="app" class="space-y-6">
    <div class="flex items-center gap-4">
      <Avatar class="size-12 rounded-xl">
        <AvatarFallback class="rounded-xl bg-primary text-primary-foreground text-lg font-bold">
          {{ (app.name || app.client_id)[0]?.toUpperCase() }}
        </AvatarFallback>
      </Avatar>
      <div class="flex-1">
        <h1 class="text-2xl font-semibold tracking-tight">{{ app.name }}</h1>
        <p class="text-sm text-muted-foreground">
          <code class="rounded bg-muted px-1.5 py-0.5 text-xs">{{ app.client_id }}</code>
          <Badge class="ml-2 text-xs" :variant="app.state === 'active' ? 'default' : 'secondary'">{{ app.state }}</Badge>
        </p>
      </div>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" :disabled="saving || !jsonValid" @click="save">
          {{ saving ? 'Saving…' : 'Save' }}
        </Button>
        <Button variant="destructive" size="sm" @click="showDeleteConfirm = true">Delete</Button>
      </div>
    </div>

    <SchemaTabsEditor
      v-model="formData"
      :schema="schemaContext.schema"
      :curl-snippets="curlSnippets"
      form-title="Application Fields"
      @update:json-valid="(value) => jsonValid = value"
    />

    <Card>
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">System Information</CardTitle>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-[100px_1fr] gap-x-4 gap-y-2 text-sm">
          <dt class="text-muted-foreground">ID</dt>
          <dd class="font-mono text-xs break-all">{{ app.id }}</dd>
          <dt class="text-muted-foreground">Org</dt>
          <dd>{{ app.org_id || '—' }}</dd>
          <dt class="text-muted-foreground">Schema</dt>
          <dd>{{ app.schema_id || '—' }}</dd>
          <dt class="text-muted-foreground">Created</dt>
          <dd>{{ formatDateTime(app.created_at) }}</dd>
          <dt class="text-muted-foreground">Updated</dt>
          <dd>{{ formatDateTime(app.updated_at) }}</dd>
        </dl>
      </CardContent>
    </Card>

    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      {{ error }}
    </div>

    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Application</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ app.name }}</strong>? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" :disabled="deleting" @click="deleteApp">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Button variant="link" as-child class="px-0 text-muted-foreground">
      <router-link to="/applications">← Back to Applications</router-link>
    </Button>
  </div>

  <div v-else class="flex h-48 items-center justify-center text-muted-foreground">Loading…</div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { appApi, type App } from '@/api/resources'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { formatDateTime } from '@/console/utils/format'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'

const route = useRoute()
const router = useRouter()
const { currentOrgId } = useOrgContext()

const app = ref<App | null>(null)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: 'app',
  versions: [],
})
const saving = ref(false)
const deleting = ref(false)
const showDeleteConfirm = ref(false)
const jsonValid = ref(true)
const error = ref('')

const payload = computed(() => buildResourceWriteBody('app', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/apps/${encodeURIComponent(String(route.params.id || ''))}`,
  body: payload.value,
  includeOrgHeader: true,
  orgId: currentOrgId.value,
  methods: ['GET', 'PATCH'],
}))

onMounted(async () => {
  try {
    app.value = await appApi.get(route.params.id as string)
    formData.value = normalizeResourceData(app.value.data || {})
    schemaContext.value = await loadResourceSchemaContext(app.value.schema_type || 'app', app.value.schema_id || '')
  } catch (err: any) {
    error.value = err?.message || 'Failed to load application'
  }
})

async function save() {
  if (!app.value) return
  saving.value = true
  error.value = ''
  try {
    app.value = await appApi.update(app.value.id, payload.value)
    formData.value = normalizeResourceData(app.value.data || {})
  } catch (err: any) {
    error.value = err?.message || 'Failed to update application'
  } finally {
    saving.value = false
  }
}

async function deleteApp() {
  if (!app.value) return
  deleting.value = true
  try {
    await appApi.delete(app.value.id)
    router.push('/applications')
  } catch (err: any) {
    error.value = err?.message || 'Failed to delete application'
    showDeleteConfirm.value = false
  } finally {
    deleting.value = false
  }
}
</script>
