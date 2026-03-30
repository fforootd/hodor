<template>
  <div class="space-y-4">
    <div v-for="field in visibleFields" :key="field.path" class="space-y-2">
      <div class="flex items-center gap-2">
        <Label :for="field.path" class="text-sm font-medium">
          {{ field.label }}
          <span v-if="field.required" class="text-destructive">*</span>
        </Label>
        <Badge v-if="field.identifier" variant="outline" class="text-[10px] uppercase tracking-wide">Identifier</Badge>
        <Badge v-if="field.sensitive" variant="secondary" class="text-[10px] uppercase tracking-wide">Sensitive</Badge>
        <Badge v-if="!field.editable" variant="outline" class="text-[10px] uppercase tracking-wide">Read only</Badge>
      </div>

      <template v-if="field.type === 'object' && field.properties?.length">
        <Card class="border-dashed">
          <CardContent class="pt-4">
            <SchemaFieldEditor
              :fields="field.properties"
              :model-value="objectValue(field.name)"
              @update:model-value="(value) => updateField(field.name, value)"
            />
          </CardContent>
        </Card>
      </template>

      <template v-else-if="field.type === 'object'">
        <textarea
          :id="field.path"
          :value="objectDraft(field)"
          class="min-h-32 w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          :disabled="!field.editable"
          @input="setObjectDraft(field, $event)"
          @change="commitObjectField(field)"
        />
        <p v-if="jsonErrors[field.path]" class="text-xs text-destructive">{{ jsonErrors[field.path] }}</p>
      </template>

      <template v-else-if="field.type === 'array'">
        <div class="space-y-2">
          <template v-if="isPrimitiveArray(field)">
            <div
              v-for="(item, index) in arrayValue(field.name)"
              :key="`${field.path}-${index}`"
              class="flex items-center gap-2"
            >
              <Input
                :model-value="String(item ?? '')"
                :type="inputType(field.item)"
                :disabled="!field.editable"
                @update:model-value="(value) => updateArrayItem(field, index, String(value ?? ''))"
              />
              <Button
                v-if="field.editable"
                variant="outline"
                size="sm"
                @click="removeArrayItem(field.name, index)"
              >
                Remove
              </Button>
            </div>
            <Button v-if="field.editable" variant="outline" size="sm" @click="addArrayItem(field)">
              Add {{ field.label }}
            </Button>
          </template>

          <template v-else>
            <textarea
              :id="field.path"
              :value="objectDraft(field)"
              class="min-h-32 w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              :disabled="!field.editable"
              @input="setObjectDraft(field, $event)"
              @change="commitArrayField(field)"
            />
            <p v-if="jsonErrors[field.path]" class="text-xs text-destructive">{{ jsonErrors[field.path] }}</p>
          </template>
        </div>
      </template>

      <template v-else-if="field.enum?.length">
        <Select
          :model-value="enumValue(field.name)"
          :disabled="!field.editable"
          @update:model-value="(value) => updateField(field.name, value || undefined)"
        >
          <SelectTrigger :id="field.path">
            <SelectValue placeholder="Select value" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in field.enum"
              :key="option"
              :value="option"
            >
              {{ option }}
            </SelectItem>
          </SelectContent>
        </Select>
      </template>

      <template v-else-if="field.type === 'boolean'">
        <div class="flex items-center gap-3 rounded-md border bg-muted/20 px-3 py-2">
          <Checkbox
            :id="field.path"
            :checked="Boolean(modelValue[field.name])"
            :disabled="!field.editable"
            @update:checked="(value: boolean | 'indeterminate') => updateField(field.name, value === true)"
          />
          <Label :for="field.path" class="text-sm font-normal">{{ field.description || field.label }}</Label>
        </div>
      </template>

      <template v-else>
        <div class="flex items-center gap-2">
          <Input
            :id="field.path"
            :model-value="stringValue(field.name)"
            :type="inputType(field)"
            :disabled="!field.editable"
            @update:model-value="(value) => updateScalarField(field, String(value ?? ''))"
          />
          <Button
            v-if="field.sensitive"
            variant="outline"
            size="sm"
            @click="toggleSensitive(field.path)"
          >
            {{ showSensitive[field.path] ? 'Hide' : 'Show' }}
          </Button>
        </div>
      </template>

      <p v-if="field.description && field.type !== 'boolean'" class="text-xs text-muted-foreground">
        {{ field.description }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive } from 'vue'
import type { SchemaFieldDefinition } from '@/console/utils/schema-resource'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

defineOptions({ name: 'SchemaFieldEditor' })

const props = defineProps<{
  fields: SchemaFieldDefinition[]
  modelValue: Record<string, any>
}>()

const emit = defineEmits<{
  'update:modelValue': [value: Record<string, any>]
}>()

const jsonDrafts = reactive<Record<string, string>>({})
const jsonErrors = reactive<Record<string, string>>({})
const showSensitive = reactive<Record<string, boolean>>({})

const visibleFields = computed(() => props.fields.filter((field) => !field.hidden))

function updateField(name: string, value: any) {
  emit('update:modelValue', {
    ...props.modelValue,
    [name]: value,
  })
}

function updateScalarField(field: SchemaFieldDefinition, rawValue: string) {
  if (field.type === 'integer') {
    updateField(field.name, rawValue === '' ? undefined : parseInt(rawValue, 10))
    return
  }
  if (field.type === 'number') {
    updateField(field.name, rawValue === '' ? undefined : parseFloat(rawValue))
    return
  }
  updateField(field.name, rawValue)
}

function objectValue(name: string): Record<string, any> {
  const value = props.modelValue[name]
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return {}
  }
  return value as Record<string, any>
}

function arrayValue(name: string): any[] {
  const value = props.modelValue[name]
  return Array.isArray(value) ? value : []
}

function addArrayItem(field: SchemaFieldDefinition) {
  const next = [...arrayValue(field.name), defaultArrayItem(field)]
  updateField(field.name, next)
}

function removeArrayItem(name: string, index: number) {
  const next = [...arrayValue(name)]
  next.splice(index, 1)
  updateField(name, next)
}

function updateArrayItem(field: SchemaFieldDefinition, index: number, rawValue: string) {
  const next = [...arrayValue(field.name)]
  if (field.item?.type === 'integer') {
    next[index] = rawValue === '' ? undefined : parseInt(rawValue, 10)
  } else if (field.item?.type === 'number') {
    next[index] = rawValue === '' ? undefined : parseFloat(rawValue)
  } else {
    next[index] = rawValue
  }
  updateField(field.name, next)
}

function defaultArrayItem(field: SchemaFieldDefinition) {
  if (field.item?.type === 'integer' || field.item?.type === 'number') return 0
  if (field.item?.type === 'boolean') return false
  return ''
}

function enumValue(name: string) {
  const value = props.modelValue[name]
  return typeof value === 'string' ? value : undefined
}

function stringValue(name: string) {
  const value = props.modelValue[name]
  return value === undefined || value === null ? '' : String(value)
}

function inputType(field?: SchemaFieldDefinition | null) {
  if (!field) return 'text'
  if (field.sensitive && !showSensitive[field.path]) return 'password'
  if (field.format === 'email') return 'email'
  if (field.format === 'uri') return 'url'
  if (field.type === 'integer' || field.type === 'number') return 'number'
  return 'text'
}

function toggleSensitive(path: string) {
  showSensitive[path] = !showSensitive[path]
}

function isPrimitiveArray(field: SchemaFieldDefinition) {
  return !field.item || !['object', 'array'].includes(field.item.type)
}

function objectDraft(field: SchemaFieldDefinition) {
  if (!(field.path in jsonDrafts)) {
    jsonDrafts[field.path] = JSON.stringify(props.modelValue[field.name] ?? (field.type === 'array' ? [] : {}), null, 2)
  }
  return jsonDrafts[field.path]
}

function setObjectDraft(field: SchemaFieldDefinition, event: Event) {
  jsonDrafts[field.path] = (event.target as HTMLTextAreaElement).value
}

function commitObjectField(field: SchemaFieldDefinition) {
  try {
    const parsed = JSON.parse(jsonDrafts[field.path] || '{}')
    jsonErrors[field.path] = ''
    updateField(field.name, parsed)
  } catch (error: any) {
    jsonErrors[field.path] = error?.message || 'Invalid JSON'
  }
}

function commitArrayField(field: SchemaFieldDefinition) {
  try {
    const parsed = JSON.parse(jsonDrafts[field.path] || '[]')
    jsonErrors[field.path] = ''
    updateField(field.name, Array.isArray(parsed) ? parsed : [])
  } catch (error: any) {
    jsonErrors[field.path] = error?.message || 'Invalid JSON'
  }
}
</script>
