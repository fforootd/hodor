<template>
  <div class="space-y-6 max-w-2xl">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link to="/applications"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">New Application</h1>
        <p class="text-muted-foreground text-sm">Register an OIDC client application.</p>
      </div>
    </div>

    <!-- Success -->
    <div v-if="success" class="rounded-lg border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-700">
      ✓ Application created! Redirecting…
    </div>

    <!-- Error -->
    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      {{ error }}
    </div>

    <Card>
      <CardHeader>
        <CardTitle class="text-sm">Application Details</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="space-y-2">
          <Label for="app-name">Name <span class="text-destructive">*</span></Label>
          <Input id="app-name" v-model="form.name" placeholder="My Web App" />
        </div>

        <div class="space-y-2">
          <Label for="app-client-id">Client ID</Label>
          <Input id="app-client-id" v-model="form.client_id" placeholder="Auto-generated if empty" class="font-mono" />
          <p class="text-xs text-muted-foreground">Leave blank to auto-generate.</p>
        </div>

        <div class="space-y-2">
          <Label for="app-type">Application Type</Label>
          <Select v-model="form.app_type">
            <SelectTrigger id="app-type"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="oidc">OIDC (Web)</SelectItem>
              <SelectItem value="native">OIDC (Native)</SelectItem>
              <SelectItem value="spa">OIDC (SPA)</SelectItem>
              <SelectItem value="api">API</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div class="space-y-2">
          <Label>Redirect URIs</Label>
          <div class="space-y-2">
            <div v-for="(uri, idx) in form.redirect_uris" :key="idx" class="flex gap-2">
              <Input v-model="form.redirect_uris[idx]" placeholder="https://example.com/callback" class="font-mono text-sm" />
              <Button variant="ghost" size="icon" class="shrink-0 text-destructive" @click="form.redirect_uris.splice(idx, 1)">
                <X class="size-4" />
              </Button>
            </div>
          </div>
          <Button variant="outline" size="sm" @click="form.redirect_uris.push('')">
            <Plus class="mr-2 size-3" /> Add URI
          </Button>
        </div>

        <div class="space-y-2">
          <Label>Grant Types</Label>
          <div class="flex flex-wrap gap-2">
            <label v-for="gt in availableGrantTypes" :key="gt" class="flex items-center gap-1.5 text-sm">
              <Checkbox
                :checked="form.grant_types.includes(gt)"
                @update:checked="(val: boolean) => toggleArrayItem(form.grant_types, gt, val)"
              />
              {{ gt }}
            </label>
          </div>
        </div>

        <div class="space-y-2">
          <Label>Response Types</Label>
          <div class="flex flex-wrap gap-2">
            <label v-for="rt in availableResponseTypes" :key="rt" class="flex items-center gap-1.5 text-sm">
              <Checkbox
                :checked="form.response_types.includes(rt)"
                @update:checked="(val: boolean) => toggleArrayItem(form.response_types, rt, val)"
              />
              {{ rt }}
            </label>
          </div>
        </div>
      </CardContent>
    </Card>

    <div class="flex justify-end gap-3">
      <Button variant="outline" as-child>
        <router-link to="/applications">Cancel</router-link>
      </Button>
      <Button @click="submit" :disabled="submitting || !form.name.trim()">
        {{ submitting ? 'Creating…' : 'Create Application' }}
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { appApi } from '@/api/resources'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Checkbox } from '@/components/ui/checkbox'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ArrowLeft, Plus, X } from 'lucide-vue-next'

const router = useRouter()
const submitting = ref(false)
const success = ref(false)
const error = ref('')

const availableGrantTypes = ['authorization_code', 'refresh_token', 'client_credentials', 'implicit']
const availableResponseTypes = ['code', 'id_token', 'token']

const form = reactive({
  name: '',
  client_id: '',
  app_type: 'oidc',
  redirect_uris: [''] as string[],
  grant_types: ['authorization_code'] as string[],
  response_types: ['code'] as string[],
})

function toggleArrayItem(arr: string[], item: string, add: boolean) {
  const idx = arr.indexOf(item)
  if (add && idx === -1) arr.push(item)
  else if (!add && idx !== -1) arr.splice(idx, 1)
}

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    if (!form.name.trim()) { error.value = 'Name is required'; submitting.value = false; return }

    const payload: any = {
      name: form.name.trim(),
      app_type: form.app_type,
      redirect_uris: form.redirect_uris.filter(u => u.trim()),
      grant_types: form.grant_types,
      response_types: form.response_types,
    }
    if (form.client_id.trim()) payload.client_id = form.client_id.trim()

    await appApi.create(payload)
    success.value = true
    setTimeout(() => router.push('/applications'), 800)
  } catch (e: any) {
    error.value = e?.message || 'Failed to create application'
  } finally {
    submitting.value = false
  }
}
</script>
