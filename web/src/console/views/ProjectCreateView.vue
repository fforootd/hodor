<template>
  <ResourceCreateView
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    singular-title="Project"
    back-route="/projects"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    description="Define the project with schema fields, inspect the JSON, or copy the request cURL."
    @submit="submit"
  />
</template>

<script setup lang="ts">
import { projectApi } from '@/api/resources'
import ResourceCreateView from '@/console/components/ResourceCreateView.vue'
import { useResourceCreate } from '@/console/composables/useResourceCreate'

const { schemaContext, formData, jsonValid, submitting, error, curlSnippets, submit } =
  useResourceCreate({
    schemaType: 'project',
    apiPath: '/v1/projects',
    resourceName: 'Project',
    listRoute: '/projects',
    createFn: projectApi.create,
    includeOrgHeader: true,
  })
</script>
