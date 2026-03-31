<template>
  <ResourceCreateView
    singular-title="Organization"
    back-route="/orgs"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    description="Use the schema-backed form, inspect the canonical JSON, or copy the API call."
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    @submit="submit"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { orgApi } from '@/api/resources'
import ResourceCreateView from '@/console/components/ResourceCreateView.vue'
import {
  buildCurlSnippets, buildResourceWriteBody,
  loadResourceSchemaContext, normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'

const router = useRouter()
const submitting = ref(false)
const error = ref('')
const jsonValid = ref(true)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {}, schema: null, schemaId: '', schemaType: 'org', versions: [],
})

const payload = computed(() => buildResourceWriteBody('org', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({ path: '/v1/orgs', body: payload.value, includeOrgHeader: false, methods: ['POST'] }))

onMounted(async () => { schemaContext.value = await loadResourceSchemaContext('org') })

async function submit() {
  submitting.value = true
  try {
    const created = await orgApi.create(payload.value)
    notifyMutationSuccess('Organization', 'create')
    router.push(`/orgs/${created.id}`)
  } catch (err: any) { notifyMutationError('Organization', 'create', err) }
  finally { submitting.value = false }
}
</script>
