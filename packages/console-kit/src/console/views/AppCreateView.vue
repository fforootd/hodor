<template>
  <div class="mx-auto max-w-lg space-y-8">
    <div>
      <router-link to="/applications" class="text-sm text-muted-foreground hover:text-foreground inline-flex items-center gap-1">
        <ArrowLeft class="size-4" />
        Back to Applications
      </router-link>
    </div>
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Create Application</h1>
      <p class="text-sm text-muted-foreground mt-1">Register a new application for authentication.</p>
    </div>
    <Card>
      <CardContent class="pt-6 space-y-4">
        <div class="space-y-2">
          <Label for="name">Name</Label>
          <Input id="name" v-model="form.name" placeholder="My Web App" />
        </div>
        <div class="space-y-2">
          <Label>Type</Label>
          <div class="grid grid-cols-2 gap-3">
            <button
              v-for="t in appTypes"
              :key="t.value"
              type="button"
              class="flex flex-col items-start gap-1 rounded-lg border p-4 text-left transition-colors hover:bg-accent"
              :class="form.app_type === t.value ? 'border-primary bg-accent' : 'border-border'"
              @click="form.app_type = t.value"
            >
              <span class="text-sm font-medium">{{ t.label }}</span>
              <span class="text-xs text-muted-foreground">{{ t.description }}</span>
            </button>
          </div>
        </div>
        <div v-if="error" class="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{{ error }}</div>
      </CardContent>
    </Card>
    <div class="flex items-center justify-end gap-3">
      <Button variant="outline" as-child><router-link to="/applications">Cancel</router-link></Button>
      <Button :disabled="!canSubmit || submitting" @click="submit">
        <Spinner v-if="submitting" class="mr-2 size-4" />
        Create Application
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { appApi } from '@/api/resources'
import { notifySuccess, notifyError } from '@/lib/notify'
import { Card, CardContent } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { ArrowLeft } from 'lucide-vue-next'

const router = useRouter()
const submitting = ref(false)
const error = ref('')

const form = reactive({
  name: '',
  app_type: 'web',
})

const appTypes = [
  { value: 'web', label: 'Web', description: 'Browser-based app with server' },
  { value: 'native', label: 'Native', description: 'Mobile or desktop app' },
  { value: 'api', label: 'API', description: 'Backend service or API' },
  { value: 'machine', label: 'Machine', description: 'Service-to-service (no user)' },
]

const canSubmit = computed(() => form.name.trim().length > 0)

function buildClientId(name: string) {
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .replace(/-+/g, '-')

  const base = slug || 'app'
  return `${base}-${Date.now().toString(36)}`
}

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const trimmedName = form.name.trim()
    const created = await appApi.create({
      name: trimmedName,
      app_type: form.app_type,
      client_id: buildClientId(trimmedName),
    })
    notifySuccess('Application created', `${form.name} is ready.`)
    router.push(`/applications/${created.id}`)
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create application'
    notifyError('Failed to create application', e)
  } finally {
    submitting.value = false
  }
}
</script>
