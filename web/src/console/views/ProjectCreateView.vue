<template>
  <ResourceCreateView
    singular-title="Project"
    back-route="/projects"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    description="Define the project with schema fields, inspect the JSON, or copy the request cURL."
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    @submit="submit"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { projectApi } from '@/api/resources'
import ResourceCreateView from '@/console/components/ResourceCreateView.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets, buildResourceWriteBody,
  loadResourceSchemaContext, normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'

const router = useRouter()
const { currentOrgId } = useOrgContext()
const submitting = ref(false)
const error = ref('')
const jsonValid = ref(true)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {}, schema: null, schemaId: '', schemaType: 'project', versions: [],
})

const payload = computed(() => buildResourceWriteBody('project', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({ path: '/v1/projects', body: payload.value, includeOrgHeader: true, orgId: currentOrgId.value, methods: ['POST'] }))

onMounted(async () => { schemaContext.value = await loadResourceSchemaContext('project') })

async function submit() {
  submitting.value = true
  try {
    const created = await projectApi.create(payload.value)
    notifyMutationSuccess('Project', 'create')
    router.push(`/projects/${created.id}`)
  } catch (err: any) { notifyMutationError('Project', 'create', err) }
  finally { submitting.value = false }
}
</script>
