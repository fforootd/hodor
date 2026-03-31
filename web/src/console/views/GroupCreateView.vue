<template>
  <ResourceCreateView
    singular-title="Group"
    back-route="/groups"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    description="Define the group with schema fields, inspect its JSON, or copy the request cURL."
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    @submit="submit"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { groupApi } from '@/api/resources'
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
  display: {}, schema: null, schemaId: '', schemaType: 'group', versions: [],
})

const payload = computed(() => buildResourceWriteBody('group', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({ path: '/v1/groups', body: payload.value, includeOrgHeader: true, orgId: currentOrgId.value, methods: ['POST'] }))

onMounted(async () => { schemaContext.value = await loadResourceSchemaContext('group') })

async function submit() {
  submitting.value = true
  try {
    const created = await groupApi.create(payload.value)
    notifyMutationSuccess('Group', 'create')
    router.push(`/groups/${created.id}`)
  } catch (err: any) { notifyMutationError('Group', 'create', err) }
  finally { submitting.value = false }
}
</script>
