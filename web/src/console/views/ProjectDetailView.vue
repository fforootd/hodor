<template>
  <ResourceDetailCockpit
    v-model:active-tab="activeTab"
    v-model:json-content="jsonContent"
    back-route="/projects"
    :badges="badges"
    :curl-snippets="curlSnippets"
    :deleting="deleting"
    :display-title="projectTitle"
    eyebrow="Project cockpit"
    :extra-tabs="[{ label: 'Members', value: 'members' }]"
    :json-error="jsonError"
    :json-valid="jsonValid"
    :load-error="loadError"
    :loading="loading"
    :overview-facts="overviewFacts"
    :resource="item"
    :saving="saving"
    :schema="schemaContext.schema"
    singular-title="Project"
    :state-rows="stateRows"
    :subtitle="item?.id || ''"
    @save="save"
    @delete="deleteResource"
    @json-valid="onJsonValid"
    @json-error="onJsonError"
  >
    <template #tab-members>
      <ResourceMembersSection
        resource-label="Project"
        resource-type="project"
        :members="members"
        @add="addMember"
        @remove="removeMember"
      />
    </template>

    <template #edit-form>
      <SchemaFieldEditor
        v-if="schemaContext.schema"
        v-model="formData"
        :fields="schemaFields"
      />
      <div v-else class="text-sm text-muted-foreground">Loading schema…</div>
    </template>
  </ResourceDetailCockpit>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { projectApi } from '@/api/resources'
import ResourceDetailCockpit from '@/console/components/ResourceDetailCockpit.vue'
import ResourceMembersSection from '@/console/components/ResourceMembersSection.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { useResourceDetail } from '@/console/composables/useResourceDetail'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { formatDateTime } from '@/console/utils/format'
import { extractSchemaFields, type SummaryFact } from '@/console/utils/schema-resource'

const activeTab = ref('overview')
const { currentOrgId } = useOrgContext()

const {
  item,
  members,
  formData,
  schemaContext,
  loading,
  saving,
  deleting,
  jsonValid,
  jsonContent,
  jsonError,
  loadError,
  curlSnippets,
  overviewFacts,
  save,
  deleteResource,
  addMember,
  removeMember,
  onJsonValid,
  onJsonError,
} = useResourceDetail({
  resourceType: 'project',
  resourceName: 'Project',
  listRoute: '/projects',
  apiPath: '/v1/projects',
  includeOrgHeader: true,
  fetchFn: projectApi.get,
  updateFn: projectApi.update,
  deleteFn: projectApi.delete,
  members: {
    list: projectApi.listMembers,
    add: projectApi.addMember,
    remove: projectApi.removeMember,
    label: 'Project member',
  },
})

const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const projectTitle = computed(() => String(formData.value.name || item.value?.name || 'Project'))
const badges = computed(() => ([
  { label: item.value?.state || 'active', variant: 'outline' as const },
  { label: schemaContext.value.schemaType || item.value?.schema_type || 'project', variant: 'secondary' as const },
  ...(currentOrgId.value ? [{ label: currentOrgId.value, variant: 'secondary' as const }] : []),
]))
const stateRows = computed<SummaryFact[]>(() => ([
  { label: 'Created', value: formatDateTime(item.value?.created_at || '') },
  { label: 'Updated', value: formatDateTime(item.value?.updated_at || '') },
  { label: 'Members', value: `${members.value.length}` },
]))
</script>
