<template>
  <ResourceCreateView
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    singular-title="Group"
    back-route="/groups"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    description="Define the group with schema fields, inspect its JSON, or copy the request cURL."
    @submit="submit"
  />
</template>

<script setup lang="ts">
import { groupApi } from '@/api/resources'
import ResourceCreateView from '@/console/components/ResourceCreateView.vue'
import { useResourceCreate } from '@/console/composables/useResourceCreate'

const { schemaContext, formData, jsonValid, submitting, error, curlSnippets, submit } =
  useResourceCreate({
    schemaType: 'group',
    apiPath: '/v1/groups',
    resourceName: 'Group',
    listRoute: '/groups',
    createFn: groupApi.create,
    includeOrgHeader: true,
  })
</script>
