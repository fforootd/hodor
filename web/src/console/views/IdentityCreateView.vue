<template>
  <div class="space-y-6 max-w-2xl">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="icon" as-child>
        <router-link :to="`/s/${schemaType}`"><ArrowLeft class="size-4" /></router-link>
      </Button>
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Create {{ currentLabel }}</h1>
        <p class="text-muted-foreground text-sm">Fill in the details below or switch to JSON mode.</p>
      </div>
    </div>

    <!-- Tab toggle -->
    <div class="inline-flex items-center rounded-lg bg-muted p-1">
      <button
        :class="['px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
          mode === 'form' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground']"
        @click="mode = 'form'"
      >📝 Form</button>
      <button
        :class="['px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
          mode === 'json' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground']"
        @click="switchToJson"
      >{ } JSON</button>
    </div>

    <form @submit.prevent="submit" class="space-y-4">
      <!-- Version picker -->
      <Card v-if="versions.length > 1">
        <CardHeader class="pb-3">
          <CardTitle class="text-sm">Schema Version</CardTitle>
        </CardHeader>
        <CardContent>
          <div class="flex flex-wrap gap-2">
            <button
              v-for="v in versions" :key="v.id" type="button"
              :class="['rounded-lg border px-3 py-2 text-left transition-colors',
                selectedSchema === v.id ? 'border-primary bg-primary/5' : 'hover:border-muted-foreground/30']"
              @click="selectSchema(v.id)"
            >
              <span class="block text-sm font-semibold">v{{ v.version }}</span>
              <span class="text-xs text-muted-foreground">{{ v.is_default ? 'default' : (v.message || 'draft') }}</span>
            </button>
          </div>
        </CardContent>
      </Card>

      <!-- ═══ FORM MODE ═══ -->
      <template v-if="mode === 'form'">
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">{{ isInteractiveIdentity ? 'Account' : 'Identity' }}</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="space-y-2">
              <Label for="create-id">Identifier <span class="text-destructive">*</span></Label>
              <Input id="create-id" v-model="form.identifier" :placeholder="identifierPlaceholder" required />
            </div>
            <div class="space-y-2">
              <Label for="create-name">Display Name</Label>
              <Input id="create-name" v-model="form.display_name" placeholder="Display name" />
            </div>
            <div class="space-y-2" v-if="hasPassword">
              <Label for="create-pw">Password</Label>
              <Input id="create-pw" v-model="form.password" type="password" placeholder="Set initial password" />
            </div>
          </CardContent>
        </Card>

        <Card v-if="schemaFields.length">
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Properties</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="space-y-2" v-for="field in schemaFields" :key="field.name">
              <Label :for="`field-${field.name}`">{{ field.label }}</Label>
              <Select v-if="field.type === 'boolean'" v-model="profileData[field.name]">
                <SelectTrigger><SelectValue placeholder="—" /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="">—</SelectItem>
                  <SelectItem value="true">true</SelectItem>
                  <SelectItem value="false">false</SelectItem>
                </SelectContent>
              </Select>
              <Select v-else-if="field.enum" v-model="profileData[field.name]">
                <SelectTrigger><SelectValue placeholder="—" /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="">—</SelectItem>
                  <SelectItem v-for="opt in field.enum" :key="opt" :value="opt">{{ opt }}</SelectItem>
                </SelectContent>
              </Select>
              <Input
                v-else
                :id="`field-${field.name}`"
                v-model="profileData[field.name]"
                :type="field.inputType"
                :placeholder="field.description || ''"
              />
              <p class="text-xs text-muted-foreground" v-if="field.description">{{ field.description }}</p>
            </div>
          </CardContent>
        </Card>

        <Card v-if="isInteractiveIdentity">
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Capabilities</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="flex flex-wrap gap-4">
              <label class="flex items-center gap-2 cursor-pointer" v-for="cap in availableCaps" :key="cap">
                <input type="checkbox" :value="cap" v-model="form.capabilities" class="accent-primary" />
                <span class="text-sm">{{ cap }}</span>
              </label>
            </div>
          </CardContent>
        </Card>

        <Card v-if="isInteractiveIdentity && hasLogin" class="bg-muted/30">
          <CardContent class="py-4">
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="checkbox" v-model="sendInvite" class="accent-primary" />
              <span class="text-sm">Send invite link after creation</span>
            </label>
            <p v-if="sendInvite" class="mt-2 rounded-md bg-blue-50 px-3 py-2 text-xs text-blue-700">
              A magic link will be sent to the identifier email.
            </p>
          </CardContent>
        </Card>
      </template>

      <!-- ═══ JSON MODE ═══ -->
      <template v-if="mode === 'json'">
        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-sm">Entity JSON</CardTitle>
            <p class="text-xs text-muted-foreground">Edit the full entity payload. Schema validation is live.</p>
          </CardHeader>
          <CardContent>
            <JsonEditor v-model="jsonContent" label="Entity Data" :schema="currentSchema?.schema" @valid="onJsonValid" @error="onJsonError" />
          </CardContent>
        </Card>
      </template>

      <!-- Actions -->
      <div class="flex justify-end gap-3 pt-2">
        <Button variant="outline" as-child>
          <router-link :to="`/s/${schemaType}`">Cancel</router-link>
        </Button>
        <Button type="submit" :disabled="submitting || (mode === 'json' && !!jsonError)">
          {{ submitting ? 'Creating…' : `Create ${currentLabel}` }}
        </Button>
      </div>

      <div v-if="error" class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">{{ error }}</div>
      <div v-if="success" class="rounded-lg border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-700">Created! Redirecting…</div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { userApi, magicLinkApi, schemaApi, metaSchemaApi, type Schema } from '@/api/resources'
import { api } from '@/api/client'
import JsonEditor from '@/console/components/JsonEditor.vue'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ArrowLeft } from 'lucide-vue-next'

const props = defineProps<{ schemaType: string }>()

const router = useRouter()
const versions = ref<Schema[]>([])
const selectedSchema = ref('')
const submitting = ref(false)
const error = ref('')
const success = ref(false)
const sendInvite = ref(true)
const displayMeta = ref<any>({})
const mode = ref<'form' | 'json'>('form')
const jsonContent = ref('{\n  \n}')
const jsonError = ref('')
const jsonParsed = ref<any>({})

const form = reactive({
  identifier: '',
  display_name: '',
  password: '',
  capabilities: [] as string[],
})

const profileData = reactive<Record<string, string>>({})
const availableCaps = ['password', 'magic_link', 'admin', 'api_key']

const currentSchema = computed(() => versions.value.find(s => s.id === selectedSchema.value))
const currentLabel = computed(() => displayMeta.value.singular || props.schemaType.replace(/_/g, ' '))

const isInteractiveIdentity = computed(() => {
  const s = currentSchema.value?.schema as any
  if (!s) return false
  return !!(s['x-identifier'] || s['x-auth-methods'])
})

const hasLogin = computed(() => !!(currentSchema.value?.schema as any)?.['x-login'])
const hasPassword = computed(() => {
  const methods = (currentSchema.value?.schema as any)?.['x-auth-methods'] || {}
  return methods.password?.enabled ?? false
})

const identifierPlaceholder = computed(() => {
  if (isInteractiveIdentity.value) return 'user@example.com'
  return `${currentLabel.value.toLowerCase()}-name`
})

interface SchemaField {
  name: string; label: string; description: string; inputType: string; type: string; enum?: string[]
}

const schemaFields = computed<SchemaField[]>(() => {
  const s = currentSchema.value
  if (!s) return []
  const schemaProps = (s.schema as any)?.properties || {}
  return Object.entries(schemaProps)
    .filter(([, def]: [string, any]) => !def?.['x-hidden'])
    .map(([name, def]: [string, any]) => ({
      name,
      label: name.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()),
      description: def?.description || '',
      type: def?.type || 'string',
      enum: def?.enum,
      inputType: def?.type === 'integer' || def?.type === 'number' ? 'number'
        : def?.format === 'email' ? 'email'
        : def?.format === 'uri' ? 'url'
        : 'text',
    }))
})

function selectSchema(id: string) {
  selectedSchema.value = id
  Object.keys(profileData).forEach(k => delete profileData[k])
}

function switchToJson() {
  const data: any = { identifier: form.identifier || undefined, display_name: form.display_name || undefined }
  for (const [k, v] of Object.entries(profileData)) { if (v) data[k] = v }
  jsonContent.value = JSON.stringify(data, null, 2)
  mode.value = 'json'
}

function onJsonValid(parsed: any) { jsonError.value = ''; jsonParsed.value = parsed }
function onJsonError(msg: string) { jsonError.value = msg }

onMounted(async () => {
  try {
    const allSchemas = await schemaApi.list()
    versions.value = allSchemas
      .filter((s: Schema) => s.type === props.schemaType)
      .sort((a: Schema, b: Schema) => b.version - a.version)

    try {
      const metaData = await metaSchemaApi.get()
      const entry = (metaData['x-catalog'] || {})[props.schemaType]
      if (entry) {
        displayMeta.value = { singular: entry.singular, alias: entry.alias, path: entry.path, icon: entry.icon }
      }
    } catch { /* ignore */ }

    const defaultVersion = versions.value.find(s => s.is_default) || versions.value[0]
    if (defaultVersion) selectSchema(defaultVersion.id)
  } catch {}
})

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    let payload: any
    if (mode.value === 'json') {
      const data = jsonParsed.value
      payload = {
        identifier: data.identifier || form.identifier || props.schemaType + '-' + Date.now(),
        display_name: data.display_name || data.identifier || '',
        profile: {}, data: data, schema_id: selectedSchema.value,
      }
    } else {
      if (!form.identifier.trim()) { error.value = 'Identifier is required'; submitting.value = false; return }
      const profile: Record<string, any> = {}
      if (form.display_name) profile.display_name = form.display_name
      for (const [k, v] of Object.entries(profileData)) {
        if (v !== '') {
          const fieldDef = schemaFields.value.find(f => f.name === k)
          if (fieldDef?.type === 'boolean') profile[k] = v === 'true'
          else if (fieldDef?.type === 'integer') profile[k] = parseInt(v) || 0
          else if (fieldDef?.type === 'number') profile[k] = parseFloat(v) || 0
          else profile[k] = v
        }
      }
      payload = {
        identifier: form.identifier.trim(),
        display_name: form.display_name.trim() || form.identifier.trim(),
        profile, capabilities: isInteractiveIdentity.value ? form.capabilities : [],
        schema_id: selectedSchema.value,
      }
    }
    const created = await userApi.create(payload)
    if (form.password && created.id && isInteractiveIdentity.value) {
      await api.post(`/v1/users/${created.id}/password`, { password: form.password }).catch(() => {})
    }
    if (sendInvite.value && hasLogin.value && created.id) {
      await magicLinkApi.send(form.identifier.trim()).catch(() => {})
    }
    success.value = true
    setTimeout(() => router.push(`/s/${props.schemaType}`), 800)
  } catch (e: any) {
    error.value = e?.message || 'Failed to create'
  } finally {
    submitting.value = false
  }
}
</script>
