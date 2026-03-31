<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link :to="backRoute"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Create {{ singularTitle }}</h1>
        <p class="text-sm text-muted-foreground">
          {{ description || `Define fields in the form, inspect canonical JSON, or copy the API request.` }}
        </p>
      </div>
    </div>

    <!-- Schema Form -->
    <SchemaTabsEditor
      v-if="schemaContext?.schema"
      v-model="formData"
      :schema="schemaContext.schema"
      :curl-snippets="curlSnippets"
      :form-title="`${singularTitle} Fields`"
      @update:json-valid="(value) => jsonValid = value"
    />
    <Card v-else>
      <CardContent class="flex items-center gap-2 pt-6 text-sm text-muted-foreground">
        <Spinner class="size-4" /> Loading schema…
      </CardContent>
    </Card>

    <!-- Extra content slot (e.g. org context, access setup) -->
    <slot />

    <!-- Actions -->
    <div class="flex justify-end gap-3">
      <Button variant="outline" as-child>
        <router-link :to="backRoute">Cancel</router-link>
      </Button>
      <Button :disabled="submitting || !jsonValid" @click="$emit('submit')">
        {{ submitting ? 'Creating…' : `Create ${singularTitle}` }}
      </Button>
    </div>

    <!-- Error alert -->
    <Alert v-if="error" variant="destructive">
      <AlertTriangleIcon class="size-4" />
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { ResourceSchemaContext } from '@/console/utils/schema-resource'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { ArrowLeft, AlertTriangle as AlertTriangleIcon } from 'lucide-vue-next'

defineProps<{
  singularTitle: string
  backRoute: string
  schemaContext: ResourceSchemaContext | null
  curlSnippets: any[]
  submitting: boolean
  error: string
  description?: string
}>()

const formData = defineModel<Record<string, any>>('formData', { default: () => ({}) })
const jsonValid = defineModel<boolean>('jsonValid', { default: true })

defineEmits<{
  submit: []
}>()
</script>
