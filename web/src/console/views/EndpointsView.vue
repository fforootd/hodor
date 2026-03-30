<template>
  <div class="space-y-6 p-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">Custom Endpoints</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Map domains and paths to Zitadel components for white-labeling and custom routing.
        </p>
      </div>
      <Dialog v-model:open="showCreate">
        <DialogTrigger as-child>
          <Button class="gap-1.5">
            <Plus class="size-4" />
            Add Endpoint
          </Button>
        </DialogTrigger>
        <DialogContent class="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Add Custom Endpoint</DialogTitle>
            <DialogDescription>
              Map a domain and path to a Zitadel component. After creation, verify DNS ownership.
            </DialogDescription>
          </DialogHeader>
          <div class="space-y-4 py-4">
            <div class="space-y-2">
              <Label for="domain">Domain</Label>
              <Input id="domain" v-model="form.domain" placeholder="auth.acme.com" />
            </div>
            <div class="space-y-2">
              <Label for="path">Path</Label>
              <Input id="path" v-model="form.path" placeholder="/" />
            </div>
            <div class="space-y-2">
              <Label for="component">Component</Label>
              <Select v-model="form.component">
                <SelectTrigger>
                  <SelectValue placeholder="Select component" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="login">Login</SelectItem>
                  <SelectItem value="api">API</SelectItem>
                  <SelectItem value="oidc">OIDC</SelectItem>
                  <SelectItem value="console">Console</SelectItem>
                  <SelectItem value="account">Account</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-2">
              <Label for="tls">TLS Mode</Label>
              <Select v-model="form.tls_mode">
                <SelectTrigger>
                  <SelectValue placeholder="Select TLS mode" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="auto">Auto (Let's Encrypt)</SelectItem>
                  <SelectItem value="custom">Custom Certificate</SelectItem>
                  <SelectItem value="none">None (HTTP only)</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" @click="showCreate = false">Cancel</Button>
            <Button @click="createEndpoint" :disabled="!form.domain || !form.component">Create</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>

    <!-- Empty state -->
    <Card v-if="!loading && endpoints.length === 0" class="border-dashed">
      <CardContent class="flex flex-col items-center justify-center py-12 text-center">
        <Link class="size-12 text-muted-foreground/50 mb-4" />
        <h3 class="text-lg font-semibold">No Endpoints Configured</h3>
        <p class="text-sm text-muted-foreground mt-1 max-w-md">
          Add custom domain mappings to serve Zitadel components (Login, API, OIDC, Console, Account)
          from your own domains for a fully white-labeled experience.
        </p>
        <Button class="mt-4 gap-1.5" @click="showCreate = true">
          <Plus class="size-4" />
          Add First Endpoint
        </Button>
      </CardContent>
    </Card>

    <!-- Endpoint list -->
    <div v-else class="space-y-3">
      <Card v-for="ep in endpoints" :key="ep.id" class="hover:border-primary/30 transition-colors">
        <CardContent class="py-4 px-5">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-3 min-w-0">
              <div class="flex items-center justify-center size-9 rounded-lg"
                   :class="componentColor(ep.component)">
                <component :is="componentIcon(ep.component)" class="size-4" />
              </div>
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="font-mono text-sm font-medium truncate">
                    {{ ep.domain }}{{ ep.path !== '/' ? ep.path : '' }}
                  </span>
                  <Badge :variant="ep.dns_verified ? 'default' : 'outline'"
                         :class="ep.dns_verified ? 'bg-emerald-500/10 text-emerald-600 border-emerald-500/20' : ''">
                    {{ ep.dns_verified ? '✓ Verified' : 'Unverified' }}
                  </Badge>
                  <Badge v-if="!ep.enabled" variant="secondary">Disabled</Badge>
                </div>
                <div class="flex items-center gap-2 text-xs text-muted-foreground mt-0.5">
                  <span class="capitalize">{{ ep.component }}</span>
                  <span>·</span>
                  <span>TLS: {{ ep.tls_mode }}</span>
                  <template v-if="ep.dns_method">
                    <span>·</span>
                    <span>DNS: {{ ep.dns_method.toUpperCase() }}</span>
                  </template>
                </div>
              </div>
            </div>
            <div class="flex items-center gap-1.5">
              <Button v-if="!ep.dns_verified" variant="outline" size="sm" class="gap-1 text-xs"
                      @click="showVerify(ep)">
                <ShieldCheck class="size-3.5" />
                Verify
              </Button>
              <Button variant="ghost" size="icon" class="size-8 text-destructive hover:text-destructive"
                      @click="deleteEndpoint(ep.id)">
                <Trash2 class="size-3.5" />
              </Button>
            </div>
          </div>

          <!-- DNS verification instructions (shown when unverified) -->
          <div v-if="verifyingId === ep.id" class="mt-4 p-3 rounded-lg bg-muted/50 border text-sm space-y-3">
            <p class="font-medium">Verify domain ownership</p>
            <div class="space-y-2">
              <p class="text-muted-foreground text-xs">Option 1: Add a TXT record</p>
              <code class="block p-2 rounded bg-background border font-mono text-xs break-all">
                _zitadel-verify.{{ ep.domain }} → {{ ep.dns_token }}
              </code>
            </div>
            <div class="space-y-2">
              <p class="text-muted-foreground text-xs">Option 2: Add a CNAME record</p>
              <code class="block p-2 rounded bg-background border font-mono text-xs break-all">
                {{ ep.domain }} → {{ ep.domain }}.zitadel.cloud
              </code>
            </div>
            <div class="flex gap-2">
              <Button size="sm" @click="verifyDns(ep.id, 'txt')">Verify TXT</Button>
              <Button size="sm" variant="outline" @click="verifyDns(ep.id, 'cname')">Verify CNAME</Button>
              <Button size="sm" variant="ghost" @click="verifyingId = null">Cancel</Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api/client'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter,
  DialogHeader, DialogTitle, DialogTrigger,
} from '@/components/ui/dialog'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import {
  Plus, Link, ShieldCheck, Trash2, Globe, Lock, KeyRound, Monitor, User,
} from 'lucide-vue-next'

interface Endpoint {
  id: string
  instance_id: string
  domain: string
  path: string
  component: string
  enabled: boolean
  tls_mode: string
  dns_verified: boolean
  dns_method: string
  dns_token: string
  created_at: string
  updated_at: string
}

const endpoints = ref<Endpoint[]>([])
const loading = ref(true)
const showCreate = ref(false)
const verifyingId = ref<string | null>(null)

const form = ref({
  domain: '',
  path: '/',
  component: 'login',
  tls_mode: 'auto',
})

const componentColors: Record<string, string> = {
  login: 'bg-blue-500/10 text-blue-600',
  api: 'bg-emerald-500/10 text-emerald-600',
  oidc: 'bg-purple-500/10 text-purple-600',
  console: 'bg-orange-500/10 text-orange-600',
  account: 'bg-pink-500/10 text-pink-600',
}

function componentColor(c: string) {
  return componentColors[c] || 'bg-muted text-muted-foreground'
}

function componentIcon(c: string) {
  const icons: Record<string, any> = {
    login: Lock,
    api: KeyRound,
    oidc: Globe,
    console: Monitor,
    account: User,
  }
  return icons[c] || Link
}

async function fetchEndpoints() {
  loading.value = true
  try {
    const resp = await api.get<{ items: Endpoint[] }>('/v1/endpoints')
    endpoints.value = resp.items || []
  } catch {
    toast.error('Failed to load endpoints')
  } finally {
    loading.value = false
  }
}

async function createEndpoint() {
  try {
    await api.post('/v1/endpoints', form.value)
    toast.success('Endpoint created')
    showCreate.value = false
    form.value = { domain: '', path: '/', component: 'login', tls_mode: 'auto' }
    await fetchEndpoints()
  } catch (e: any) {
    toast.error(e.message || 'Failed to create endpoint')
  }
}

async function deleteEndpoint(id: string) {
  try {
    await api.delete(`/v1/endpoints/${id}`)
    toast.success('Endpoint deleted')
    await fetchEndpoints()
  } catch {
    toast.error('Failed to delete endpoint')
  }
}

function showVerify(ep: Endpoint) {
  verifyingId.value = verifyingId.value === ep.id ? null : ep.id
}

async function verifyDns(id: string, method: string) {
  try {
    await api.post(`/v1/endpoints/${id}/verify`, { method })
    toast.success('DNS verified!')
    verifyingId.value = null
    await fetchEndpoints()
  } catch (e: any) {
    toast.error(e.message || 'Verification failed')
  }
}

onMounted(fetchEndpoints)
</script>
