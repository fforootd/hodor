<template>
  <div class="space-y-6">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link to="/orgs"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Create Organization</h1>
        <p class="text-sm text-muted-foreground">Use the schema-backed form, inspect the canonical JSON, or copy the API call.</p>
      </div>
    </div>

    <SchemaTabsEditor
      v-if="schemaContext.schema"
      v-model="formData"
      :schema="schemaContext.schema"
      :curl-snippets="curlSnippets"
      form-title="Organization Fields"
      @update:json-valid="(value) => jsonValid = value"
    />

    <Card v-else>
      <CardContent class="pt-6 text-sm text-muted-foreground">Loading schema…</CardContent>
    </Card>

    <div class="flex justify-end gap-3">
      <Button variant="outline" as-child>
        <router-link to="/orgs">Cancel</router-link>
      </Button>
      <Button :disabled="submitting || !jsonValid" @click="submit">
        {{ submitting ? 'Creating…' : 'Create Organization' }}
      </Button>
    </div>

    <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      {{ error }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { orgApi } from '@/api/resources'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { ArrowLeft } from 'lucide-vue-next'

const router = useRouter()

const submitting = ref(false)
const error = ref('')
const jsonValid = ref(true)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: 'org',
  versions: [],
})

const payload = computed(() => buildResourceWriteBody('org', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: '/v1/orgs',
  body: payload.value,
  includeOrgHeader: false,
  methods: ['POST'],
}))

onMounted(async () => {
  schemaContext.value = await loadResourceSchemaContext('org')
})

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const created = await orgApi.create(payload.value)
    router.push(`/orgs/${created.id}`)
  } catch (err: any) {
    error.value = err?.message || 'Failed to create organization'
  } finally {
    submitting.value = false
  }
}
</script>
