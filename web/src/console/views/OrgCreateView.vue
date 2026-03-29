<template>
  <div class="space-y-6 max-w-2xl">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link to="/orgs"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Create Organization</h1>
        <p class="text-muted-foreground text-sm">Set up a new organization for your project.</p>
      </div>
    </div>

    <form @submit.prevent="submit" class="space-y-4">
      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-sm">Organization Details</CardTitle>
        </CardHeader>
        <CardContent class="space-y-4">
          <div class="space-y-2">
            <Label for="org-name">Name <span class="text-destructive">*</span></Label>
            <Input id="org-name" v-model="form.name" placeholder="e.g. Acme Corporation" required />
            <p class="text-xs text-muted-foreground">A unique name for your organization.</p>
          </div>
        </CardContent>
      </Card>

      <div class="flex justify-end gap-3 pt-2">
        <Button variant="outline" as-child>
          <router-link to="/orgs">Cancel</router-link>
        </Button>
        <Button type="submit" :disabled="submitting || !form.name.trim()">
          {{ submitting ? 'Creating…' : 'Create Organization' }}
        </Button>
      </div>

      <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">{{ error }}</div>
      <div v-if="success" class="rounded-lg border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-700">Created! Redirecting…</div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { orgApi } from '@/api/resources'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ArrowLeft } from 'lucide-vue-next'

const router = useRouter()
const submitting = ref(false)
const error = ref('')
const success = ref(false)
const form = reactive({ name: '' })

async function submit() {
  if (!form.name.trim() || submitting.value) return
  submitting.value = true
  error.value = ''
  try {
    await orgApi.create({ name: form.name.trim() })
    success.value = true
    setTimeout(() => router.push('/orgs'), 800)
  } catch (e: any) {
    error.value = e?.message || 'Failed to create organization'
  } finally {
    submitting.value = false
  }
}
</script>
