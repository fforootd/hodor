<template>
  <div class="json-editor-wrap">
    <div class="editor-toolbar" v-if="!hideToolbar">
      <span class="editor-label">{{ label }}</span>
      <div class="toolbar-actions">
        <button type="button" class="tb-btn" @click="format" title="Format JSON">⎘ Format</button>
        <button type="button" class="tb-btn" @click="copy" title="Copy to clipboard">📋 Copy</button>
      </div>
    </div>
    <div class="editor-container" :style="{ height: height }">
      <vue-monaco-editor
        v-model:value="internalValue"
        language="json"
        :theme="theme"
        :options="editorOptions"
        @mount="onMount"
      />
    </div>
    <div v-if="parseError" class="parse-error">{{ parseError }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { VueMonacoEditor } from '@guolao/vue-monaco-editor'

const props = withDefaults(defineProps<{
  modelValue: string
  label?: string
  hideToolbar?: boolean
  schema?: any
  height?: string
  theme?: string
}>(), {
  label: 'JSON',
  hideToolbar: false,
  height: '360px',
  theme: 'vs',
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'valid': [parsed: any]
  'error': [msg: string]
}>()

const internalValue = ref(props.modelValue)
const parseError = ref('')
let editorInstance: any = null
let monacoInstance: any = null

const editorOptions = computed(() => ({
  minimap: { enabled: false },
  fontSize: 13,
  lineNumbers: 'on' as const,
  scrollBeyondLastLine: false,
  wordWrap: 'on' as const,
  tabSize: 2,
  automaticLayout: true,
  renderLineHighlight: 'all' as const,
  bracketPairColorization: { enabled: true },
  padding: { top: 8, bottom: 8 },
  scrollbar: {
    verticalScrollbarSize: 8,
    horizontalScrollbarSize: 8,
  },
}))

// Sync external modelValue → internal
watch(() => props.modelValue, (val) => {
  if (val !== internalValue.value) {
    internalValue.value = val
  }
})

// Sync internal → external + validate
watch(internalValue, (val) => {
  emit('update:modelValue', val)
  try {
    const parsed = JSON.parse(val || '{}')
    parseError.value = ''
    emit('valid', parsed)
  } catch (e: any) {
    parseError.value = e.message
    emit('error', e.message)
  }
})

function onMount(editor: any, monaco: any) {
  editorInstance = editor
  monacoInstance = monaco

  // Inject JSON Schema for validation + autocomplete
  if (props.schema) {
    setSchema(props.schema)
  }
}

// Watch for schema changes
watch(() => props.schema, (schema) => {
  if (schema && monacoInstance) {
    setSchema(schema)
  }
})

function setSchema(schema: any) {
  if (!monacoInstance) return
  monacoInstance.languages.json.jsonDefaults.setDiagnosticsOptions({
    validate: true,
    schemas: [{
      uri: 'http://zitadel-local/entity-schema.json',
      fileMatch: ['*'],
      schema: schema,
    }],
  })
}

function format() {
  if (editorInstance) {
    editorInstance.getAction('editor.action.formatDocument')?.run()
  }
}

function copy() {
  navigator.clipboard.writeText(internalValue.value)
}

// Initial validation
try {
  const parsed = JSON.parse(props.modelValue || '{}')
  emit('valid', parsed)
} catch (e: any) {
  parseError.value = e.message
  emit('error', e.message)
}
</script>

<style scoped>
.json-editor-wrap {
  display: flex; flex-direction: column;
  border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden;
}
.editor-toolbar {
  display: flex; justify-content: space-between; align-items: center;
  padding: 0.5rem 0.75rem; background: #f8f9fa; border-bottom: 1px solid #e5e7eb;
}
.editor-label {
  font-size: 0.75rem; font-weight: 600; color: #6b7280;
  text-transform: uppercase; letter-spacing: 0.05em;
}
.toolbar-actions { display: flex; gap: 0.375rem; }
.tb-btn {
  padding: 0.25rem 0.5rem; border: 1px solid #d1d5db; border-radius: 6px; background: #fff;
  font-size: 0.6875rem; color: #6b7280; cursor: pointer; transition: all 0.15s;
}
.tb-btn:hover { border-color: #6366f1; color: #6366f1; }

.editor-container { width: 100%; }

.parse-error {
  padding: 0.375rem 0.75rem; background: #fef2f2; color: #dc2626;
  font-size: 0.75rem; font-family: monospace; border-top: 1px solid #fecaca;
}
</style>
