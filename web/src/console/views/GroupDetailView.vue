<template>
  <ResourceDetailView
    :resource="group"
    resource-type="group"
    singular-title="Group"
    back-route="/groups"
    :display-title="groupTitle"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :saving="saving"
    :deleting="deleting"
    :load-error="loadError"
    :json-valid="jsonValid"
    :show-members="true"
    :members="members"
    v-model:form-data="formData"
    @save="save"
    @delete="deleteGroup"
    @add-member="addMember"
    @remove-member="removeMember"
    @update:json-valid="(v) => jsonValid = v"
  >
    <template #header-badges>
      <StateBadge :state="group?.state" />
      <Badge variant="secondary" class="text-xs">{{ members.length }} members</Badge>
    </template>
  </ResourceDetailView>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { groupApi, type Group, type Member } from '@/api/resources'
import ResourceDetailView from '@/console/components/ResourceDetailView.vue'
import { StateBadge } from '@/components/ui/state-badge'
import { Badge } from '@/components/ui/badge'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  buildCurlSnippets, buildResourceWriteBody,
  loadResourceSchemaContext, normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'

const route = useRoute()
const router = useRouter()
const { currentOrgId } = useOrgContext()

const group = ref<Group | null>(null)
const members = ref<Member[]>([])
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {}, schema: null, schemaId: '', schemaType: 'group', versions: [],
})
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const loadError = ref('')

const groupId = computed(() => String(route.params.id || ''))
const groupTitle = computed(() => String(formData.value.name || group.value?.name || 'Group'))
const payload = computed(() => buildResourceWriteBody('group', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/groups/${encodeURIComponent(groupId.value)}`, body: payload.value,
  includeOrgHeader: true, orgId: currentOrgId.value, methods: ['GET', 'PATCH'],
}))

async function load() {
  if (!groupId.value) return
  loadError.value = ''
  try {
    const [g, m] = await Promise.all([groupApi.get(groupId.value), groupApi.listMembers(groupId.value)])
    group.value = g; members.value = m
    formData.value = normalizeResourceData(g.data || {})
    schemaContext.value = await loadResourceSchemaContext(g.schema_type || 'group', g.schema_id || '')
  } catch (err: any) { loadError.value = err?.message || 'Failed to load group' }
}

async function save() {
  if (!group.value) return
  saving.value = true
  try {
    group.value = await groupApi.update(group.value.id, payload.value)
    formData.value = normalizeResourceData(group.value.data || {})
    notifyMutationSuccess('Group', 'update')
  } catch (err: any) { notifyMutationError('Group', 'update', err) }
  finally { saving.value = false }
}

async function deleteGroup() {
  if (!group.value) return
  deleting.value = true
  try {
    await groupApi.delete(group.value.id)
    notifyMutationSuccess('Group', 'delete')
    router.push('/groups')
  } catch (err: any) { notifyMutationError('Group', 'delete', err) }
  finally { deleting.value = false }
}

async function addMember(userId: string) {
  if (!group.value) return
  try {
    await groupApi.addMember(group.value.id, userId)
    members.value = await groupApi.listMembers(group.value.id)
    notifyMutationSuccess('Group member', 'add')
  } catch (err: any) { notifyMutationError('Group member', 'add', err) }
}

async function removeMember(userId: string) {
  if (!group.value) return
  try {
    await groupApi.removeMember(group.value.id, userId)
    members.value = await groupApi.listMembers(group.value.id)
    notifyMutationSuccess('Group member', 'remove')
  } catch (err: any) { notifyMutationError('Group member', 'remove', err) }
}

onMounted(load)
watch(groupId, load)
</script>
