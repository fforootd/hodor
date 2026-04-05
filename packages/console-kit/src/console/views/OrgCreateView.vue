<template>
  <div class="mx-auto max-w-lg space-y-8">
    <div>
      <router-link to="/orgs" class="text-sm text-muted-foreground hover:text-foreground inline-flex items-center gap-1">
        <ArrowLeft class="size-4" />
        Back to Organizations
      </router-link>
    </div>
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Create Organization</h1>
      <p class="text-sm text-muted-foreground mt-1">Organizations group users and resources together.</p>
    </div>
    <Card>
      <CardContent class="pt-6 space-y-4">
        <div class="space-y-2">
          <Label for="name">Name</Label>
          <Input id="name" v-model="form.name" placeholder="Acme Corp" />
        </div>
        <div v-if="error" class="rounded-md bg-destructive/10 p-3 text-sm text-destructive">{{ error }}</div>
      </CardContent>
    </Card>
    <div class="flex items-center justify-end gap-3">
      <Button variant="outline" as-child><router-link to="/orgs">Cancel</router-link></Button>
      <Button :disabled="!canSubmit || submitting" @click="submit">
        <Spinner v-if="submitting" class="mr-2 size-4" />
        Create Organization
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { orgApi } from '@/api/resources'
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
const form = reactive({ name: '' })
const canSubmit = computed(() => form.name.trim().length > 0)

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const created = await orgApi.create({ name: form.name.trim() })
    notifySuccess('Organization created', `${form.name} is ready.`)
    router.push(`/orgs/${created.id}`)
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create organization'
    notifyError('Failed to create organization', e)
  } finally {
    submitting.value = false
  }
}
</script>
