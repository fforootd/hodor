<template>
  <div class="space-y-6">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link to="/groups"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Create Group</h1>
        <p class="text-sm text-muted-foreground">Define the group with schema fields, inspect its JSON, or copy the request cURL.</p>
      </div>
    </div>

    <SchemaTabsEditor
      v-if="schemaContext.schema"
      v-model="formData"
      :schema="schemaContext.schema"
      :curl-snippets="curlSnippets"
      form-title="Group Fields"
      @update:json-valid="(value) => jsonValid = value"
    />

    <Card v-else>
      <CardContent class="pt-6 text-sm text-muted-foreground">Loading schema…</CardContent>
    </Card>

    <div class="flex justify-end gap-3">
      <Button variant="outline" as-child>
        <router-link to="/groups">Cancel</router-link>
      </Button>
      <Button :disabled="submitting || !jsonValid" @click="submit">
        {{ submitting ? 'Creating…' : 'Create Group' }}
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
import { groupApi } from '@/api/resources'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
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
const { currentOrgId } = useOrgContext()

const submitting = ref(false)
const error = ref('')
const jsonValid = ref(true)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: 'group',
  versions: [],
})

const payload = computed(() => buildResourceWriteBody('group', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: '/v1/groups',
  body: payload.value,
  includeOrgHeader: true,
  orgId: currentOrgId.value,
  methods: ['POST'],
}))

onMounted(async () => {
  schemaContext.value = await loadResourceSchemaContext('group')
})

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const created = await groupApi.create(payload.value)
    router.push(`/groups/${created.id}`)
  } catch (err: any) {
    error.value = err?.message || 'Failed to create group'
  } finally {
    submitting.value = false
  }
}
</script>
