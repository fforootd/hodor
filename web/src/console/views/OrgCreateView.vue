<template>
  <ResourceCreateView
    v-model:form-data="formData"
    v-model:json-valid="jsonValid"
    singular-title="Organization"
    back-route="/orgs"
    :schema-context="schemaContext"
    :curl-snippets="curlSnippets"
    :submitting="submitting"
    :error="error"
    description="Use the schema-backed form, inspect the canonical JSON, or copy the API call."
    @submit="submit"
  />
</template>

<script setup lang="ts">
import { orgApi } from '@/api/resources'
import ResourceCreateView from '@/console/components/ResourceCreateView.vue'
import { useResourceCreate } from '@/console/composables/useResourceCreate'

const { schemaContext, formData, jsonValid, submitting, error, curlSnippets, submit } =
  useResourceCreate({
    schemaType: 'org',
    apiPath: '/v1/orgs',
    resourceName: 'Organization',
    listRoute: '/orgs',
    createFn: orgApi.create,
  })
</script>
