<template>
  <div v-if="provider && form" class="space-y-6">
    <div class="flex items-start gap-4">
      <Avatar class="size-12 rounded-xl">
        <AvatarFallback class="rounded-xl bg-primary text-primary-foreground text-lg font-bold">
          {{ providerIcon(templateLabel) }}
        </AvatarFallback>
      </Avatar>
      <div class="min-w-0 flex-1">
        <h1 class="truncate text-2xl font-semibold tracking-tight">{{ form.display_name }}</h1>
        <div class="mt-1 flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
          <code class="rounded bg-muted px-1.5 py-0.5 text-xs">{{ templateLabel }}</code>
          <Badge :variant="provider.enabled ? 'default' : 'secondary'" class="text-xs">
            {{ provider.enabled ? 'enabled' : 'disabled' }}
          </Badge>
          <Badge variant="outline" class="text-xs uppercase">{{
            provider.protocol || 'provider'
          }}</Badge>
        </div>
      </div>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" :disabled="saving" @click="save">
          {{ saving ? 'Saving…' : 'Save' }}
        </Button>
        <Button variant="destructive" size="sm" @click="showDeleteConfirm = true">Delete</Button>
      </div>
    </div>

    <Card>
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Provider Settings</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="grid gap-4 md:grid-cols-2">
          <div class="space-y-2">
            <Label for="provider-display-name">Provider Name</Label>
            <Input id="provider-display-name" v-model="form.display_name" />
          </div>
          <div class="space-y-2">
            <Label for="provider-enabled">Status</Label>
            <div class="flex h-10 items-center gap-3 rounded-md border px-3">
              <Switch id="provider-enabled" v-model:checked="form.enabled" />
              <span class="text-sm text-muted-foreground">
                {{ form.enabled ? 'Available for login' : 'Disabled for login' }}
              </span>
            </div>
          </div>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <div class="space-y-2">
            <Label for="provider-linking-mode">Linking Mode</Label>
            <select
              id="provider-linking-mode"
              v-model="form.linking_mode"
              class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="create_or_link">Create or link</option>
              <option value="link_only">Link only</option>
            </select>
          </div>
          <div class="space-y-2">
            <Label for="provider-linking-match">Match By</Label>
            <select
              id="provider-linking-match"
              v-model="form.linking_match_by"
              class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="verified_email">Verified email</option>
              <option value="identifier">Identifier</option>
              <option value="none">None</option>
            </select>
          </div>
        </div>
      </CardContent>
    </Card>

    <Card v-if="connectionEntries.length > 0">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">Connection</CardTitle>
      </CardHeader>
      <CardContent class="grid gap-4 md:grid-cols-2">
        <div
          v-for="entry in connectionEntries"
          :key="entry.key"
          class="space-y-2"
          :class="entry.key === 'scopes' ? 'md:col-span-2' : ''"
        >
          <Label :for="`connection-${entry.key}`">{{ formatProviderFieldLabel(entry.key) }}</Label>
          <Input
            :id="`connection-${entry.key}`"
            v-model="form.connection[entry.key]"
            :type="entry.key === 'client_secret' ? 'password' : 'text'"
          />
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-3">
        <div class="flex items-center justify-between gap-4">
          <CardTitle class="text-sm">Claim Mapping</CardTitle>
          <Button variant="outline" size="sm" @click="addClaimMappingRow">
            <Plus class="mr-1 size-3.5" />
            Add
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div v-if="form.mapping_claims.length === 0" class="text-sm text-muted-foreground">
          No claim mappings configured.
        </div>
        <div v-else class="space-y-3">
          <div
            v-for="(claim, index) in form.mapping_claims"
            :key="`claim-${index}`"
            class="grid gap-3 rounded-lg border bg-muted/20 p-3 md:grid-cols-[1fr_1.3fr_auto]"
          >
            <div class="space-y-2">
              <Label>Field</Label>
              <Input v-model="claim.field" placeholder="email" />
            </div>
            <div class="space-y-2">
              <Label>Expression</Label>
              <Input v-model="claim.expression" placeholder="claims.email" />
            </div>
            <div class="flex items-end">
              <Button variant="ghost" size="sm" @click="removeClaimMappingRow(index)">
                Remove
              </Button>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-3">
        <CardTitle class="text-sm">System Information</CardTitle>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 text-sm">
          <dt class="text-muted-foreground">ID</dt>
          <dd class="font-mono text-xs break-all">{{ provider.id }}</dd>
          <dt class="text-muted-foreground">Template</dt>
          <dd>{{ templateLabel }}</dd>
          <dt class="text-muted-foreground">Kind</dt>
          <dd>{{ provider.kind || '—' }}</dd>
          <dt class="text-muted-foreground">Target Schema</dt>
          <dd>{{ targetSchemaLabel }}</dd>
          <dt class="text-muted-foreground">Linking</dt>
          <dd>{{ humanizeProviderLinking(form.linking_mode, form.linking_match_by) }}</dd>
          <dt class="text-muted-foreground">Created</dt>
          <dd>{{ formatDateTime(provider.created_at) }}</dd>
          <dt class="text-muted-foreground">Updated</dt>
          <dd>{{ formatDateTime(provider.updated_at) }}</dd>
        </dl>
      </CardContent>
    </Card>

    <div
      v-if="loadError"
      class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive"
    >
      {{ loadError }}
    </div>

    <Dialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete Provider</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete <strong>{{ form.display_name }}</strong
            >? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showDeleteConfirm = false">Cancel</Button>
          <Button variant="destructive" :disabled="deleting" @click="deleteProvider">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Button variant="link" as-child class="px-0 text-muted-foreground">
      <router-link to="/providers">← Back to Providers</router-link>
    </Button>
  </div>

  <div v-else class="flex h-48 items-center justify-center text-muted-foreground">
    {{ loadError || 'Loading…' }}
  </div>
</template>

<script setup lang="ts">
  import { computed, onMounted, ref } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import { api } from '@/api/client'
  import type { ProviderRecord } from '@/console/utils/provider-utils'
  import {
    formatProviderFieldLabel,
    humanizeProviderLinking,
    humanizeProviderValue,
    providerClaimMappings,
    providerConnection,
    providerDisplayName,
    providerIcon,
    providerTemplateLabel,
    sortProviderConnectionKeys,
  } from '@/console/utils/provider-utils'
  import { formatDateTime } from '@/console/utils/format'
  import { Avatar, AvatarFallback } from '@/components/ui/avatar'
  import { Badge } from '@/components/ui/badge'
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
  import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'
  import { Switch } from '@/components/ui/switch'
  import { Plus } from 'lucide-vue-next'

  interface ProviderEditForm {
    display_name: string
    enabled: boolean
    connection: Record<string, string>
    linking_mode: string
    linking_match_by: string
    mapping_claims: Array<{ field: string; expression: string }>
  }

  const route = useRoute()
  const router = useRouter()

  const provider = ref<ProviderRecord | null>(null)
  const form = ref<ProviderEditForm | null>(null)
  const saving = ref(false)
  const deleting = ref(false)
  const showDeleteConfirm = ref(false)
  const loadError = ref('')

  const providerId = computed(() => String(route.params.id || ''))
  const templateLabel = computed(() =>
    provider.value ? providerTemplateLabel(provider.value) : 'custom',
  )
  const targetSchemaLabel = computed(() => {
    if (!provider.value?.target) return '—'
    return (
      provider.value.target.schema_id || humanizeProviderValue(provider.value.target.schema_type)
    )
  })
  const connectionEntries = computed(() => {
    if (!form.value) return []
    return sortProviderConnectionKeys(Object.keys(form.value.connection)).map((key) => ({ key }))
  })

  function buildForm(record: ProviderRecord): ProviderEditForm {
    const connection = providerConnection(record)
    const normalizedConnection = Object.fromEntries(
      sortProviderConnectionKeys(Object.keys(connection)).map((key) => {
        const value = connection[key]
        if (Array.isArray(value)) return [key, value.join(' ')]
        if (value == null) return [key, '']
        if (typeof value === 'object') return [key, JSON.stringify(value)]
        return [key, String(value)]
      }),
    )
    return {
      display_name: providerDisplayName(record),
      enabled: record.enabled,
      connection: normalizedConnection,
      linking_mode: record.linking?.mode || 'create_or_link',
      linking_match_by: record.linking?.match_by || 'verified_email',
      mapping_claims: providerClaimMappings(record).map(([field, expression]) => ({
        field,
        expression,
      })),
    }
  }

  async function loadProvider() {
    if (!providerId.value) return
    loadError.value = ''
    try {
      const loaded = await api.get<ProviderRecord>(`/v1/providers/${providerId.value}`)
      provider.value = loaded
      form.value = buildForm(loaded)
    } catch (err: any) {
      loadError.value = err?.message || 'Failed to load provider'
    }
  }

  async function save() {
    if (!provider.value || !form.value) return
    saving.value = true
    try {
      const originalConnection = providerConnection(provider.value)
      const nextConnection = Object.fromEntries(
        Object.entries(form.value.connection).map(([key, value]) => {
          const trimmed = value.trim()
          const original = originalConnection[key]
          if (Array.isArray(original)) {
            return [key, trimmed ? trimmed.split(/\s+/) : []]
          }
          return [key, trimmed]
        }),
      )

      const claims = Object.fromEntries(
        form.value.mapping_claims
          .map((claim) => [claim.field.trim(), claim.expression.trim()] as const)
          .filter(([field, expression]) => field && expression),
      )

      provider.value = await api.patch<ProviderRecord>(`/v1/providers/${provider.value.id}`, {
        display_name: form.value.display_name.trim(),
        enabled: form.value.enabled,
        connection: nextConnection,
        linking: {
          mode: form.value.linking_mode,
          match_by: form.value.linking_match_by,
        },
        mapping: {
          claims,
        },
      })
      form.value = buildForm(provider.value)
      notifyMutationSuccess('Provider', 'update')
    } catch (err: any) {
      notifyMutationError('Provider', 'update', err)
    } finally {
      saving.value = false
    }
  }

  async function deleteProvider() {
    if (!provider.value) return
    deleting.value = true
    try {
      await api.delete(`/v1/providers/${provider.value.id}`)
      notifyMutationSuccess('Provider', 'delete')
      await router.push('/providers')
    } catch (err: any) {
      notifyMutationError('Provider', 'delete', err)
    } finally {
      deleting.value = false
      showDeleteConfirm.value = false
    }
  }

  function addClaimMappingRow() {
    if (!form.value) return
    form.value.mapping_claims.push({ field: '', expression: '' })
  }

  function removeClaimMappingRow(index: number) {
    if (!form.value) return
    form.value.mapping_claims.splice(index, 1)
  }

  onMounted(loadProvider)
</script>
