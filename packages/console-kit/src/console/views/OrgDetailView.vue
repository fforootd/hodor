<template>
  <ResourceDetailCockpit
    v-model:active-tab="activeTab"
    v-model:json-content="jsonContent"
    back-route="/orgs"
    :badges="badges"
    :curl-snippets="curlSnippets"
    :deleting="deleting"
    :display-title="orgTitle"
    eyebrow="Organization cockpit"
    :extra-tabs="[{ label: 'Members', value: 'members' }]"
    :json-error="jsonError"
    :json-valid="jsonValid"
    :load-error="loadError"
    :loading="loading"
    :overview-facts="overviewFacts"
    :resource="item"
    :saving="saving"
    :schema="schemaContext.schema"
    singular-title="Organization"
    :state-rows="stateRows"
    :subtitle="item?.id || ''"
    @save="save"
    @delete="deleteResource"
    @json-valid="onJsonValid"
    @json-error="onJsonError"
  >
    <template #tab-members>
      <ResourceMembersSection
        resource-label="Organization"
        resource-type="org"
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
import { orgApi, orgMembersApi } from '@/api/resources'
import ResourceDetailCockpit from '@/console/components/ResourceDetailCockpit.vue'
import ResourceMembersSection from '@/console/components/ResourceMembersSection.vue'
import SchemaFieldEditor from '@/console/components/SchemaFieldEditor.vue'
import { useResourceDetail } from '@/console/composables/useResourceDetail'
import { formatDateTime } from '@/console/utils/format'
import { extractSchemaFields, type SummaryFact } from '@/console/utils/schema-resource'

const activeTab = ref('overview')

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
  resourceType: 'org',
  resourceName: 'Organization',
  listRoute: '/orgs',
  apiPath: '/v1/orgs',
  fetchFn: orgApi.get,
  updateFn: orgApi.update,
  deleteFn: orgApi.delete,
  members: {
    list: orgMembersApi.list,
    add: orgMembersApi.add,
    remove: orgMembersApi.remove,
    label: 'Organization member',
  },
})

const schemaFields = computed(() => extractSchemaFields(schemaContext.value.schema))
const orgTitle = computed(() => String(formData.value.display_name || item.value?.name || 'Organization'))
const badges = computed(() => ([
  { label: item.value?.state || 'active', variant: 'outline' as const },
  { label: schemaContext.value.schemaType || item.value?.schema_type || 'org', variant: 'secondary' as const },
  { label: `${members.value.length} members`, variant: 'secondary' as const },
]))
const stateRows = computed<SummaryFact[]>(() => ([
  { label: 'Created', value: formatDateTime(item.value?.created_at || '') },
  { label: 'Updated', value: formatDateTime(item.value?.updated_at || '') },
  { label: 'Members', value: `${members.value.length}` },
]))
</script>
