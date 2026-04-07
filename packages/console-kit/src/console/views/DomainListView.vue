<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">Domains</h1>
        <p class="text-sm text-muted-foreground">Manage custom domains for your instance.</p>
      </div>
      <Dialog v-model:open="showAddDialog">
        <DialogTrigger as-child>
          <Button><Plus class="mr-2 size-4" /> Add Domain</Button>
        </DialogTrigger>
        <DialogContent class="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Add Custom Domain</DialogTitle>
            <DialogDescription>Add a custom domain to your instance. You'll need to verify ownership via DNS.</DialogDescription>
          </DialogHeader>
          <div class="space-y-4 py-4">
            <div class="space-y-2">
              <Label for="domain">Domain</Label>
              <Input id="domain" v-model="newDomain" placeholder="login.example.com" />
            </div>
            <div class="space-y-2">
              <Label>Purpose</Label>
              <div class="flex gap-4">
                <label class="flex items-center gap-2 cursor-pointer">
                  <input type="radio" v-model="newPurpose" value="served" class="accent-primary" />
                  <span class="text-sm">Served (full hosting)</span>
                </label>
                <label class="flex items-center gap-2 cursor-pointer">
                  <input type="radio" v-model="newPurpose" value="allowed" class="accent-primary" />
                  <span class="text-sm">Allowed (embed)</span>
                </label>
              </div>
              <p class="text-xs text-muted-foreground">
                <strong>Served:</strong> Host the login page on this domain (requires TLS certificate).
                <strong>Allowed:</strong> Allow embedding components from this domain (CORS/CSP only).
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" @click="showAddDialog = false">Cancel</Button>
            <Button :disabled="!newDomain || submitting" @click="addDomain">
              {{ submitting ? 'Adding...' : 'Add Domain' }}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>

    <!-- DNS Verification Instructions (shown after adding a domain) -->
    <div v-if="pendingVerification" class="rounded-lg border border-yellow-200 bg-yellow-50 dark:border-yellow-900 dark:bg-yellow-950 p-4 space-y-3">
      <h3 class="font-semibold text-sm">Verify domain ownership</h3>
      <p class="text-sm text-muted-foreground">Add the following TXT record to your DNS configuration:</p>
      <div class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm font-mono bg-background rounded p-3">
        <span class="text-muted-foreground">Host:</span>
        <div class="flex items-center gap-2">
          <code>{{ pendingVerification.dns_challenge_host }}</code>
          <Button variant="ghost" size="icon" class="size-6" @click="copyToClipboard(pendingVerification!.dns_challenge_host)">
            <Copy class="size-3" />
          </Button>
        </div>
        <span class="text-muted-foreground">Value:</span>
        <div class="flex items-center gap-2">
          <code>zitadel-verify={{ pendingVerification.verification_token }}</code>
          <Button variant="ghost" size="icon" class="size-6" @click="copyToClipboard(`zitadel-verify=${pendingVerification!.verification_token}`)">
            <Copy class="size-3" />
          </Button>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <Button size="sm" @click="verifyDomain(pendingVerification!.domain)" :disabled="verifying">
          {{ verifying ? 'Verifying...' : 'Verify Domain' }}
        </Button>
        <Button variant="ghost" size="sm" @click="pendingVerification = null">Dismiss</Button>
      </div>
      <p v-if="pendingVerification.purpose === 'served'" class="text-xs text-muted-foreground">
        After verification, TLS certificate provisioning will take ~2 minutes.
      </p>
    </div>

    <!-- Domain List -->
    <div v-if="loading" class="text-sm text-muted-foreground">Loading domains...</div>
    <div v-else-if="domains.length === 0" class="rounded-lg border border-dashed p-12 text-center">
      <Globe class="mx-auto size-10 text-muted-foreground mb-4" />
      <h3 class="font-semibold">No custom domains configured</h3>
      <p class="text-sm text-muted-foreground mt-1">Add a domain to get started.</p>
    </div>
    <div v-else class="rounded-md border">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b bg-muted/50">
            <th class="px-4 py-3 text-left font-medium">Domain</th>
            <th class="px-4 py-3 text-left font-medium">Purpose</th>
            <th class="px-4 py-3 text-left font-medium">Status</th>
            <th class="px-4 py-3 text-left font-medium">Created</th>
            <th class="px-4 py-3 text-right font-medium">Actions</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="d in domains" :key="d.domain" class="border-b last:border-0">
            <td class="px-4 py-3 font-mono text-sm">
              {{ d.domain }}
              <Badge v-if="d.is_primary" variant="outline" class="ml-2 text-xs">Primary</Badge>
            </td>
            <td class="px-4 py-3">
              <Badge :variant="d.purpose === 'served' ? 'default' : 'secondary'" class="text-xs">
                {{ d.purpose === 'served' ? 'Served' : 'Allowed' }}
              </Badge>
            </td>
            <td class="px-4 py-3">
              <StateBadge :state="d.state" />
              <span v-if="d.state === 'provisioning'" class="ml-1 text-xs text-muted-foreground">(TLS)</span>
            </td>
            <td class="px-4 py-3 text-muted-foreground">{{ formatDate(d.created_at) }}</td>
            <td class="px-4 py-3 text-right space-x-2">
              <Button
                v-if="d.state === 'pending_verification' || d.state === 'verification_failed'"
                size="sm" variant="outline"
                @click="showVerification(d)"
              >
                Verify
              </Button>
              <Button
                v-if="!d.is_primary"
                size="sm" variant="ghost"
                class="text-destructive"
                @click="removeDomain(d.domain)"
              >
                Remove
              </Button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { domainApi, type CustomDomain } from '@/api/resources'
import { formatDate } from '@/console/utils/format'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { StateBadge } from '@/components/ui/state-badge'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter,
  DialogHeader, DialogTitle, DialogTrigger,
} from '@/components/ui/dialog'
import { Plus, Globe, Copy } from 'lucide-vue-next'
import { notifySuccess, notifyError } from '@/lib/notify'

const domains = ref<CustomDomain[]>([])
const loading = ref(false)
const showAddDialog = ref(false)
const newDomain = ref('')
const newPurpose = ref<'allowed' | 'served'>('served')
const submitting = ref(false)
const verifying = ref(false)
const pendingVerification = ref<CustomDomain | null>(null)

async function fetchDomains() {
  loading.value = true
  try {
    const result = await domainApi.list()
    domains.value = result.items || []
  } catch (e: any) {
    notifyError('Failed to load domains', e.message)
  } finally {
    loading.value = false
  }
}

async function addDomain() {
  if (!newDomain.value) return
  submitting.value = true
  try {
    const record = await domainApi.add(newDomain.value, newPurpose.value)
    notifySuccess('Domain added', `${record.domain} is pending verification.`)
    showAddDialog.value = false
    pendingVerification.value = record
    newDomain.value = ''
    newPurpose.value = 'served'
    await fetchDomains()
  } catch (e: any) {
    notifyError('Failed to add domain', e.message)
  } finally {
    submitting.value = false
  }
}

async function verifyDomain(domain: string) {
  verifying.value = true
  try {
    const record = await domainApi.verify(domain)
    if (record.state === 'active') {
      notifySuccess('Domain verified', `${domain} is now active.`)
      pendingVerification.value = null
    } else if (record.state === 'provisioning') {
      notifySuccess('Domain verified', `${domain} is provisioning TLS certificate.`)
      pendingVerification.value = null
    } else {
      notifySuccess('Domain verified', `${domain} state: ${record.state}`)
    }
    await fetchDomains()
  } catch (e: any) {
    notifyError('Verification failed', e.message)
  } finally {
    verifying.value = false
  }
}

async function removeDomain(domain: string) {
  try {
    await domainApi.remove(domain)
    notifySuccess('Domain removed', `${domain} has been removed.`)
    if (pendingVerification.value?.domain === domain) {
      pendingVerification.value = null
    }
    await fetchDomains()
  } catch (e: any) {
    notifyError('Failed to remove domain', e.message)
  }
}

function showVerification(d: CustomDomain) {
  pendingVerification.value = d
}

function copyToClipboard(text: string) {
  navigator.clipboard.writeText(text)
  notifySuccess('Copied', 'Copied to clipboard.')
}

onMounted(fetchDomains)
</script>
