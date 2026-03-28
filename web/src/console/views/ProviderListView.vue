<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">Providers</h1>
        <p class="text-muted-foreground">{{ providers.length }} provider{{ providers.length !== 1 ? 's' : '' }} configured</p>
      </div>
      <Button v-if="!showCreate" @click="showCreate = true">
        <Plus class="mr-2 size-4" />
        Add Provider
      </Button>
      <Button v-else variant="outline" @click="showCreate = false; selectedTemplate = null">Cancel</Button>
    </div>

    <!-- Template Picker -->
    <div v-if="showCreate && !selectedTemplate" class="space-y-3">
      <h3 class="text-sm font-semibold">Choose a provider template</h3>
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        <Card
          v-for="t in templates" :key="t.id"
          class="cursor-pointer transition-colors hover:border-primary"
          @click="pickTemplate(t)"
        >
          <CardContent class="relative p-4">
            <div class="text-2xl mb-2">{{ templateIcon(t.id) }}</div>
            <div class="font-semibold text-sm">{{ t.name }}</div>
            <p class="text-xs text-muted-foreground mt-1 leading-relaxed">{{ t.description }}</p>
            <Badge variant="secondary" class="absolute top-3 right-3 text-[10px] uppercase">{{ t.protocol }}</Badge>
          </CardContent>
        </Card>
      </div>
    </div>

    <!-- Create Form -->
    <Card v-if="showCreate && selectedTemplate">
      <CardHeader>
        <CardTitle>Configure {{ selectedTemplate.name }} Provider</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-2">
            <Label for="prov-name">Name</Label>
            <Input id="prov-name" v-model="createForm.name" placeholder="e.g. Google Production" />
          </div>
          <div class="space-y-2">
            <Label for="prov-issuer">Issuer</Label>
            <Input id="prov-issuer" v-model="createForm.issuer" placeholder="https://accounts.google.com" />
          </div>
          <div class="space-y-2">
            <Label for="prov-client">Client ID</Label>
            <Input id="prov-client" v-model="createForm.client_id" placeholder="your-client-id" />
          </div>
          <div class="space-y-2">
            <Label for="prov-secret">Client Secret</Label>
            <Input id="prov-secret" v-model="createForm.client_secret" type="password" placeholder="your-client-secret" />
          </div>
          <div class="space-y-2">
            <Label for="prov-scopes">Scopes</Label>
            <Input id="prov-scopes" v-model="createForm.scopes" placeholder="openid email profile" />
          </div>
          <div class="flex items-center gap-2 self-end pb-0.5">
            <input type="checkbox" id="prov-auto" v-model="createForm.auto_register" class="accent-primary" />
            <Label for="prov-auto" class="font-normal cursor-pointer">Auto-register new users</Label>
          </div>
        </div>

        <div v-if="selectedTemplate.claim_overrides && Object.keys(selectedTemplate.claim_overrides).length" class="rounded-lg bg-muted p-3 space-y-1">
          <h4 class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Default Claim Overrides</h4>
          <div v-for="(expr, field) in selectedTemplate.claim_overrides" :key="field" class="text-sm">
            <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ field }}</code>
            →
            <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ expr }}</code>
          </div>
        </div>

        <Separator />

        <div class="flex justify-end gap-3">
          <Button variant="outline" @click="selectedTemplate = null">← Back</Button>
          <Button @click="createProvider" :disabled="!createForm.name || !createForm.issuer || !createForm.client_id">
            Create Provider
          </Button>
        </div>
        <p v-if="createError" class="text-sm text-destructive">{{ createError }}</p>
      </CardContent>
    </Card>

    <!-- Provider List -->
    <Card>
      <CardContent class="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Protocol</TableHead>
              <TableHead>Template</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Auto Register</TableHead>
              <TableHead>Created</TableHead>
              <TableHead class="w-20"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="p in providers" :key="p.id"
              class="cursor-pointer"
              @click="toggleDetail(p)"
            >
              <TableCell class="font-medium">
                <span class="mr-2">{{ templateIcon(p.template) }}</span>
                {{ p.name }}
              </TableCell>
              <TableCell>
                <Badge variant="outline" class="text-xs uppercase">{{ p.protocol }}</Badge>
              </TableCell>
              <TableCell class="text-sm">{{ p.template }}</TableCell>
              <TableCell>
                <Badge :variant="p.enabled ? 'default' : 'destructive'" class="text-xs">
                  {{ p.enabled ? 'enabled' : 'disabled' }}
                </Badge>
              </TableCell>
              <TableCell>
                <Badge :variant="p.auto_register ? 'default' : 'secondary'" class="text-xs">
                  {{ p.auto_register ? 'yes' : 'no' }}
                </Badge>
              </TableCell>
              <TableCell class="text-sm text-muted-foreground">{{ formatTime(p.created_at) }}</TableCell>
              <TableCell @click.stop>
                <div class="flex gap-1">
                  <Button variant="ghost" size="icon" class="size-8" :title="p.enabled ? 'Disable' : 'Enable'" @click="toggleEnabled(p)">
                    {{ p.enabled ? '⏸' : '▶' }}
                  </Button>
                  <Button variant="ghost" size="icon" class="size-8 text-destructive hover:text-destructive" title="Delete" @click="deleteProvider(p)">
                    <Trash2 class="size-4" />
                  </Button>
                </div>
              </TableCell>
            </TableRow>
            <TableRow v-if="!providers.length">
              <TableCell :colspan="7" class="h-24 text-center text-muted-foreground">
                No providers configured yet. Add one above.
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <!-- Detail Panel -->
    <Card v-if="detailProvider">
      <CardHeader class="flex-row items-center justify-between space-y-0">
        <CardTitle>{{ detailProvider.name }}</CardTitle>
        <Button variant="outline" size="sm" @click="detailProvider = null">Close</Button>
      </CardHeader>
      <CardContent>
        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">ID</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm break-all">{{ detailProvider.id }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Issuer</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm break-all">{{ detailProvider.config?.issuer || '—' }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Client ID</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm break-all">{{ detailProvider.config?.client_id || '—' }}</code>
          </div>
          <div class="space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Scopes</span>
            <code class="block rounded bg-muted px-2 py-1 text-sm">{{ detailProvider.config?.scopes || '—' }}</code>
          </div>
          <div v-if="detailProvider.claim_overrides && Object.keys(detailProvider.claim_overrides).length" class="col-span-2 space-y-1">
            <span class="text-xs font-semibold uppercase text-muted-foreground tracking-wider">Claim Overrides</span>
            <div v-for="(expr, field) in detailProvider.claim_overrides" :key="field" class="text-sm">
              <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ field }}</code>
              →
              <code class="rounded bg-primary/10 px-1.5 py-0.5 text-xs">{{ expr }}</code>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { api } from '@/api/client'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Plus, Trash2 } from 'lucide-vue-next'

interface Template {
  id: string; name: string; protocol: string; description: string;
  default_config?: Record<string, any>; claim_overrides?: Record<string, string>;
}
interface Provider {
  id: string; name: string; protocol: string; template: string;
  enabled: boolean; auto_register: boolean; config?: Record<string, any>;
  claim_overrides?: Record<string, string>; created_at: string;
}

const providers = ref<Provider[]>([])
const templates = ref<Template[]>([])
const showCreate = ref(false)
const selectedTemplate = ref<Template | null>(null)
const detailProvider = ref<Provider | null>(null)
const createError = ref('')
const createForm = ref({
  name: '', issuer: '', client_id: '', client_secret: '', scopes: 'openid email profile', auto_register: true
})

onMounted(async () => {
  await Promise.all([fetchProviders(), fetchTemplates()])
})

async function fetchProviders() {
  try {
    const data = await api.get<any>('/v1/providers')
    providers.value = data.providers || []
  } catch { /* ignore */ }
}

async function fetchTemplates() {
  try {
    const data = await api.get<any>('/v1/providers/templates')
    templates.value = data.templates || []
  } catch { /* ignore */ }
}

function pickTemplate(t: Template) {
  selectedTemplate.value = t
  createForm.value.name = ''
  createForm.value.issuer = t.default_config?.issuer || ''
  createForm.value.scopes = (t.default_config?.scopes as string) || 'openid email profile'
  createForm.value.client_id = ''
  createForm.value.client_secret = ''
  createError.value = ''
}

async function createProvider() {
  createError.value = ''
  try {
    await api.post('/v1/providers', {
      name: createForm.value.name,
      protocol: selectedTemplate.value?.protocol || 'oidc',
      template: selectedTemplate.value?.id || 'custom',
      config: {
        issuer: createForm.value.issuer,
        client_id: createForm.value.client_id,
        client_secret: createForm.value.client_secret,
        scopes: createForm.value.scopes,
      },
      auto_register: createForm.value.auto_register,
    })
    showCreate.value = false
    selectedTemplate.value = null
    await fetchProviders()
  } catch (e: any) {
    createError.value = e.message || 'Create failed'
  }
}

async function toggleEnabled(p: Provider) {
  await api.patch(`/v1/providers/${p.id}`, { enabled: !p.enabled })
  await fetchProviders()
}

async function deleteProvider(p: Provider) {
  if (!confirm(`Delete provider "${p.name}"?`)) return
  await api.delete(`/v1/providers/${p.id}`)
  if (detailProvider.value?.id === p.id) detailProvider.value = null
  await fetchProviders()
}

async function toggleDetail(p: Provider) {
  if (detailProvider.value?.id === p.id) {
    detailProvider.value = null
    return
  }
  try {
    detailProvider.value = await api.get<Provider>(`/v1/providers/${p.id}`)
  } catch {
    detailProvider.value = p
  }
}

function templateIcon(id: string): string {
  const icons: Record<string, string> = {
    google: '🔵', entraid: '🟦', gitlab: '🦊', apple: '🍎', custom: '⚙'
  }
  return icons[id] || '🔗'
}

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>
