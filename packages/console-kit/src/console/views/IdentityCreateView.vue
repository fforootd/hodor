<template>
  <div class="mx-auto max-w-lg space-y-8">
    <!-- Back link -->
    <div>
      <router-link to="/users" class="text-sm text-muted-foreground hover:text-foreground inline-flex items-center gap-1">
        <ArrowLeft class="size-4" />
        Back to Users
      </router-link>
    </div>

    <!-- Header -->
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Create User</h1>
      <p class="text-sm text-muted-foreground mt-1">Add a new user to this instance.</p>
    </div>

    <!-- Form -->
    <Card>
      <CardContent class="pt-6 space-y-4">
        <div class="space-y-2">
          <Label for="identifier">Email or username</Label>
          <Input
            id="identifier"
            v-model="form.identifier"
            placeholder="jane@example.com"
            autocomplete="off"
          />
        </div>

        <div class="space-y-2">
          <Label for="display-name">Display name</Label>
          <Input
            id="display-name"
            v-model="form.display_name"
            placeholder="Jane Doe"
          />
        </div>

        <div class="space-y-2">
          <Label for="password">Password <span class="text-muted-foreground font-normal">(optional)</span></Label>
          <Input
            id="password"
            v-model="form.password"
            type="password"
            placeholder="Set an initial password"
            autocomplete="new-password"
          />
        </div>

        <div v-if="looksLikeEmail" class="flex items-center gap-2 pt-2">
          <Checkbox id="send-invite" v-model:checked="form.sendInvite" />
          <Label for="send-invite" class="text-sm font-normal cursor-pointer">Send invite email</Label>
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
        <router-link to="/users">Cancel</router-link>
      </Button>
      <Button :disabled="!canSubmit || submitting" @click="submit">
        <Spinner v-if="submitting" class="mr-2 size-4" />
        Create User
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { userApi, magicLinkApi } from '@/api/resources'
import { notifySuccess, notifyError } from '@/lib/notify'
import { Card, CardContent } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Spinner } from '@/components/ui/spinner'
import { ArrowLeft } from 'lucide-vue-next'

const router = useRouter()
const submitting = ref(false)
const error = ref('')

const form = reactive({
  identifier: '',
  display_name: '',
  password: '',
  sendInvite: false,
})

const looksLikeEmail = computed(() => form.identifier.includes('@'))
const canSubmit = computed(() => form.identifier.trim().length > 0)

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const created = await userApi.create({
      identifier: form.identifier.trim(),
      display_name: form.display_name.trim() || form.identifier.trim(),
    })

    // Set password if provided
    if (form.password && created.id) {
      try {
        await userApi.setPassword(created.id, form.password)
      } catch (e: any) {
        notifyError('Password not set', e)
      }
    }

    // Send invite if requested
    if (form.sendInvite && looksLikeEmail.value) {
      try {
        await magicLinkApi.send(form.identifier.trim())
      } catch {
        // Non-fatal — user was still created
      }
    }

    notifySuccess('User created', `${form.identifier} is ready.`)
    router.push(`/users/${created.id}`)
  } catch (e: any) {
    error.value = e?.error || e?.message || 'Failed to create user'
    notifyError('Failed to create user', e)
  } finally {
    submitting.value = false
  }
}
</script>
