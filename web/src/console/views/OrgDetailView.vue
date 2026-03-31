<template>
  <ResourceDetailView
    v-model:form-data="formData"
    :resource="org"
    resource-type="org"
    singular-title="Organization"
    back-route="/orgs"
    :display-title="orgTitle"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :saving="saving"
    :deleting="deleting"
    :load-error="loadError"
    :json-valid="jsonValid"
    :show-members="true"
    :members="members"
    @save="save"
    @delete="deleteOrg"
    @add-member="addMember"
    @remove-member="removeMember"
    @update:json-valid="(v) => jsonValid = v"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { orgApi, orgMembersApi, type Org, type OrgMember } from '@/api/resources'
import ResourceDetailView from '@/console/components/ResourceDetailView.vue'
import {
  buildCurlSnippets, buildResourceWriteBody,
  loadResourceSchemaContext, normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'

const route = useRoute()
const router = useRouter()

const org = ref<Org | null>(null)
const members = ref<OrgMember[]>([])
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {}, schema: null, schemaId: '', schemaType: 'org', versions: [],
})
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const loadError = ref('')

const orgId = computed(() => String(route.params.id || ''))
const orgTitle = computed(() => String(formData.value.name || org.value?.name || 'Organization'))
const payload = computed(() => buildResourceWriteBody('org', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/orgs/${encodeURIComponent(orgId.value)}`, body: payload.value,
  includeOrgHeader: false, methods: ['GET', 'PATCH'],
}))

async function load() {
  if (!orgId.value) return
  loadError.value = ''
  try {
    const [loadedOrg, loadedMembers] = await Promise.all([
      orgApi.get(orgId.value), orgMembersApi.list(orgId.value),
    ])
    org.value = loadedOrg
    members.value = loadedMembers
    formData.value = normalizeResourceData(loadedOrg.data || {})
    schemaContext.value = await loadResourceSchemaContext(loadedOrg.schema_type || 'org', loadedOrg.schema_id || '')
  } catch (err: any) { loadError.value = err?.message || 'Failed to load organization' }
}

async function save() {
  if (!org.value) return
  saving.value = true
  try {
    org.value = await orgApi.update(org.value.id, payload.value)
    formData.value = normalizeResourceData(org.value.data || {})
    notifyMutationSuccess('Organization', 'update')
  } catch (err: any) { notifyMutationError('Organization', 'update', err) }
  finally { saving.value = false }
}

async function deleteOrg() {
  if (!org.value) return
  deleting.value = true
  try {
    await orgApi.delete(org.value.id)
    notifyMutationSuccess('Organization', 'delete')
    router.push('/orgs')
  } catch (err: any) { notifyMutationError('Organization', 'delete', err) }
  finally { deleting.value = false }
}

async function addMember(userId: string) {
  if (!org.value) return
  try {
    await orgMembersApi.add(org.value.id, userId)
    members.value = await orgMembersApi.list(org.value.id)
    notifyMutationSuccess('Organization member', 'add')
  } catch (err: any) { notifyMutationError('Organization member', 'add', err) }
}

async function removeMember(userId: string) {
  if (!org.value) return
  try {
    await orgMembersApi.remove(org.value.id, userId)
    members.value = await orgMembersApi.list(org.value.id)
    notifyMutationSuccess('Organization member', 'remove')
  } catch (err: any) { notifyMutationError('Organization member', 'remove', err) }
}

onMounted(load)
watch(orgId, load)
</script>
