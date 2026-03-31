<template>
  <ResourceCreateView
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    singular-title="Application"
    back-route="/applications"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    description="Fill the schema form, inspect the JSON payload, or copy the API call directly."
    @submit="submit"
  />
</template>

<script setup lang="ts">
import { appApi } from '@/api/resources'
import ResourceCreateView from '@/console/components/ResourceCreateView.vue'
import { useResourceCreate } from '@/console/composables/useResourceCreate'

const { schemaContext, formData, jsonValid, submitting, error, curlSnippets, submit } =
  useResourceCreate({
    schemaType: 'app',
    apiPath: '/v1/apps',
    resourceName: 'Application',
    listRoute: '/applications',
    createFn: appApi.create,
    includeOrgHeader: true,
    defaultFormData: {
      app_type: 'web',
      redirect_uris: [],
      grant_types: ['authorization_code'],
      response_types: ['code'],
    },
  })
</script>
