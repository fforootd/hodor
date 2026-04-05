<template>
  <div class="mx-auto max-w-2xl space-y-8">
    <!-- Back link -->
    <div>
      <router-link to="/instances" class="text-sm text-muted-foreground hover:text-foreground inline-flex items-center gap-1">
        <ArrowLeft class="size-4" />
        Back to Instances
      </router-link>
    </div>

    <!-- Header -->
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Create Instance</h1>
      <p class="text-sm text-muted-foreground mt-1">Set up a new environment for your project.</p>
    </div>

    <!-- Form -->
    <Card>
      <CardContent class="pt-6 space-y-6">
        <!-- Instance ID -->
        <div class="space-y-2">
          <Label for="instance-id">Instance ID</Label>
          <Input
            id="instance-id"
            :model-value="form.instance_id"
            placeholder="my-project-prod"
            @update:model-value="onIdInput"
          />
          <p class="text-xs text-muted-foreground">A unique identifier. Lowercase letters, digits, and hyphens. Must contain at least one letter.</p>
        </div>

        <!-- Domain -->
        <div class="space-y-2">
          <Label for="domain">Domain</Label>
          <Input
            id="domain"
            v-model="form.domain"
            placeholder="my-project-prod.zitadel.cloud"
          />
          <p class="text-xs text-muted-foreground">The primary domain for this instance. Must be globally unique.</p>
        </div>

        <!-- Region -->
        <div class="space-y-2">
          <Label>Region</Label>
          <div class="grid grid-cols-2 gap-3">
            <button
              v-for="region in regions"
              :key="region.key"
              type="button"
              class="flex flex-col items-start gap-1 rounded-lg border p-4 text-left transition-colors hover:bg-accent"
              :class="form.region_key === region.key ? 'border-primary bg-accent' : 'border-border'"
              @click="form.region_key = region.key"
            >
              <span class="text-sm font-medium">{{ region.label }}</span>
              <span class="text-xs text-muted-foreground">{{ region.description }}</span>
            </button>
          </div>
        </div>

        <!-- Error -->
        <div v-if="error" class="rounded-md bg-destructive/10 p-3 text-sm text-destructive">
          {{ error }}
        </div>
      </CardContent>
    </Card>

    <!-- Actions -->
    <div class="flex items-center justify-end gap-3">
      <Button variant="outline" as-child>
        <router-link to="/instances">Cancel</router-link>
      </Button>
      <Button :disabled="!canSubmit || submitting" @click="submit">
        <Spinner v-if="submitting" class="mr-2 size-4" />
        Create Instance
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { instanceApi } from '@/api/resources'
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
  instance_id: '',
  domain: '',
  region_key: '',
  placement_mode: 'global',
})

const regions = [
  { key: '', label: 'Global', description: 'Replicated worldwide, lowest latency' },
  { key: 'europe-west1', label: 'Europe (West)', description: 'Belgium, EU data residency' },
  { key: 'us-central1', label: 'US (Central)', description: 'Iowa, US data residency' },
  { key: 'asia-southeast1', label: 'Asia (Southeast)', description: 'Singapore, APAC data residency' },
]

function onIdInput(value: string) {
  // Slugify: lowercase, letters/digits/hyphens only, no leading hyphens.
  const slugged = value
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, '')
    .replace(/^-+/, '')
    .replace(/-+/g, '-')
  form.instance_id = slugged
  if (!form.domain || form.domain.endsWith('.zitadel.cloud')) {
    form.domain = slugged ? `${slugged}.zitadel.cloud` : ''
  }
}

const canSubmit = computed(() => {
  const id = form.instance_id.trim()
  return id.length >= 1 && form.domain.trim().length > 0
})

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    await instanceApi.create({
      instance_id: form.instance_id,
      domain: form.domain,
      placement_mode: form.region_key ? 'regional' : 'global',
      region_key: form.region_key || undefined,
    })
    notifySuccess('Instance created', `${form.domain} is ready.`)
    router.push('/instances')
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create instance'
    notifyError('Failed to create instance', e)
  } finally {
    submitting.value = false
  }
}
</script>
