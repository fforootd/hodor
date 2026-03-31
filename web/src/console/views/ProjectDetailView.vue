<template>
  <ResourceDetailView
    :resource="project"
    resource-type="project"
    singular-title="Project"
    back-route="/projects"
    :display-title="projectTitle"
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
    @delete="deleteProject"
    @add-member="addMember"
    @remove-member="removeMember"
    @update:json-valid="(v) => jsonValid = v"
  >
    <template #header-badges>
      <StateBadge :state="project?.state" />
      <Badge variant="secondary" class="text-xs">{{ members.length }} members</Badge>
    </template>
  </ResourceDetailView>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { projectApi, type Project, type Member } from '@/api/resources'
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

const project = ref<Project | null>(null)
const members = ref<Member[]>([])
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {}, schema: null, schemaId: '', schemaType: 'project', versions: [],
})
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const loadError = ref('')

const projectId = computed(() => String(route.params.id || ''))
const projectTitle = computed(() => String(formData.value.name || project.value?.name || 'Project'))
const payload = computed(() => buildResourceWriteBody('project', schemaContext.value.schemaId, normalizeResourceData(formData.value)))
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/projects/${encodeURIComponent(projectId.value)}`, body: payload.value,
  includeOrgHeader: true, orgId: currentOrgId.value, methods: ['GET', 'PATCH'],
}))

async function load() {
  if (!projectId.value) return
  loadError.value = ''
  try {
    const [p, m] = await Promise.all([projectApi.get(projectId.value), projectApi.listMembers(projectId.value)])
    project.value = p; members.value = m
    formData.value = normalizeResourceData(p.data || {})
    schemaContext.value = await loadResourceSchemaContext(p.schema_type || 'project', p.schema_id || '')
  } catch (err: any) { loadError.value = err?.message || 'Failed to load project' }
}

async function save() {
  if (!project.value) return
  saving.value = true
  try {
    project.value = await projectApi.update(project.value.id, payload.value)
    formData.value = normalizeResourceData(project.value.data || {})
    notifyMutationSuccess('Project', 'update')
  } catch (err: any) { notifyMutationError('Project', 'update', err) }
  finally { saving.value = false }
}

async function deleteProject() {
  if (!project.value) return
  deleting.value = true
  try {
    await projectApi.delete(project.value.id)
    notifyMutationSuccess('Project', 'delete')
    router.push('/projects')
  } catch (err: any) { notifyMutationError('Project', 'delete', err) }
  finally { deleting.value = false }
}

async function addMember(userId: string) {
  if (!project.value) return
  try {
    await projectApi.addMember(project.value.id, userId)
    members.value = await projectApi.listMembers(project.value.id)
    notifyMutationSuccess('Project member', 'add')
  } catch (err: any) { notifyMutationError('Project member', 'add', err) }
}

async function removeMember(userId: string) {
  if (!project.value) return
  try {
    await projectApi.removeMember(project.value.id, userId)
    members.value = await projectApi.listMembers(project.value.id)
    notifyMutationSuccess('Project member', 'remove')
  } catch (err: any) { notifyMutationError('Project member', 'remove', err) }
}

onMounted(load)
watch(projectId, load)
</script>
