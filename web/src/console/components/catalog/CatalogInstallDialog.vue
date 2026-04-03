<template>
  <Dialog :open="open" @update:open="$emit('update:open', $event)">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Download class="size-5 text-primary" />
          {{ dialogTitle }}
        </DialogTitle>
        <DialogDescription>
          {{ dialogDescription }}
        </DialogDescription>
      </DialogHeader>

      <!-- Loading -->
      <div v-if="loading" class="flex h-32 items-center justify-center">
        <Spinner class="mr-2" /> Loading template…
      </div>

      <!-- Error -->
      <FormError v-else-if="error" :error="error" />

      <!-- Main form -->
      <div v-else-if="detail" class="space-y-4">
        <div class="flex items-center gap-2 flex-wrap">
          <Badge variant="secondary" class="text-xs">{{ detail.template.type }}</Badge>
          <Badge variant="outline" class="text-xs font-mono">v{{ detail.template.version }}</Badge>
          <Badge v-for="tag in detail.template.tags" :key="tag" variant="outline" class="text-xs">
            {{ tag }}
          </Badge>
        </div>

        <div v-if="isProviderTemplate" class="rounded-lg border bg-muted/30 p-4 space-y-2">
          <div class="space-y-1">
            <p class="text-sm font-semibold">Provider Template</p>
            <p class="text-sm text-muted-foreground">
              {{ detail.template.name }} provides the default protocol, schema target, linking
              behavior, and claim mapping.
            </p>
          </div>
          <div class="grid gap-2 text-xs text-muted-foreground sm:grid-cols-3">
            <div class="rounded-md bg-background px-3 py-2">
              <span class="block font-medium text-foreground">Protocol</span>
              {{ providerProtocolLabel }}
            </div>
            <div class="rounded-md bg-background px-3 py-2">
              <span class="block font-medium text-foreground">Target Schema</span>
              {{ providerTargetLabel }}
            </div>
            <div class="rounded-md bg-background px-3 py-2">
              <span class="block font-medium text-foreground">Linking</span>
              {{ providerLinkingLabel }}
            </div>
          </div>
        </div>

        <div v-if="isProviderTemplate && providerPrimaryVariableKeys.length > 0" class="space-y-3">
          <div class="space-y-1">
            <h4 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
              Provider Configuration
            </h4>
            <p class="text-sm text-muted-foreground">
              These settings apply only to the provider instance you are creating from this
              template.
            </p>
          </div>
          <div v-for="key in providerPrimaryVariableKeys" :key="key" class="space-y-1.5">
            <Label :for="`var-${key}`" class="text-sm font-medium">
              {{ formatLabel(key) }}
            </Label>
            <p v-if="detail.variables[key].description" class="text-xs text-muted-foreground">
              {{ detail.variables[key].description }}
            </p>

            <div v-if="detail.variables[key].type === 'boolean'" class="flex items-center gap-2">
              <Switch
                :id="`var-${key}`"
                :checked="formValues[key] ?? detail.variables[key].default ?? false"
                @update:checked="formValues[key] = $event"
              />
              <span class="text-sm text-muted-foreground">{{
                formValues[key] ? 'Enabled' : 'Disabled'
              }}</span>
            </div>

            <Input
              v-else-if="detail.variables[key].type === 'integer'"
              :id="`var-${key}`"
              type="number"
              :model-value="formValues[key] ?? detail.variables[key].default ?? 0"
              class="h-9"
              @update:model-value="formValues[key] = Number($event)"
            />

            <!-- String (default) -->
            <Input
              v-else
              :id="`var-${key}`"
              :type="detail.variables[key].sensitive ? 'password' : 'text'"
              :model-value="formValues[key] ?? detail.variables[key].default ?? ''"
              :placeholder="String(detail.variables[key].default || '')"
              class="h-9"
              @update:model-value="formValues[key] = $event"
            />
          </div>
        </div>

        <div
          v-else-if="!isProviderTemplate && hasRequiredVariables && genericVariableKeys.length > 0"
          class="space-y-3"
        >
          <h4 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
            Configuration
          </h4>
          <div v-for="key in genericVariableKeys" :key="key" class="space-y-1.5">
            <Label :for="`var-${key}`" class="text-sm font-medium">
              {{ formatLabel(key) }}
              <span
                v-if="detail.variables[key].description"
                class="font-normal text-muted-foreground ml-1"
              >
                — {{ detail.variables[key].description }}
              </span>
            </Label>

            <div v-if="detail.variables[key].type === 'boolean'" class="flex items-center gap-2">
              <Switch
                :id="`var-${key}`"
                :checked="formValues[key] ?? detail.variables[key].default ?? false"
                @update:checked="formValues[key] = $event"
              />
              <span class="text-sm text-muted-foreground">{{
                formValues[key] ? 'Enabled' : 'Disabled'
              }}</span>
            </div>

            <Input
              v-else-if="detail.variables[key].type === 'integer'"
              :id="`var-${key}`"
              type="number"
              :model-value="formValues[key] ?? detail.variables[key].default ?? 0"
              class="h-9"
              @update:model-value="formValues[key] = Number($event)"
            />

            <Input
              v-else
              :id="`var-${key}`"
              :type="detail.variables[key].sensitive ? 'password' : 'text'"
              :model-value="formValues[key] ?? detail.variables[key].default ?? ''"
              :placeholder="String(detail.variables[key].default || '')"
              class="h-9"
              @update:model-value="formValues[key] = $event"
            />
          </div>
        </div>

        <p
          v-else-if="!isProviderTemplate || providerAdvancedVariableKeys.length === 0"
          class="text-sm text-muted-foreground py-2"
        >
          This template requires no configuration.
        </p>

        <Collapsible v-if="isProviderTemplate">
          <CollapsibleTrigger
            class="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer hover:text-foreground transition-colors"
          >
            <ChevronRight class="size-3 transition-transform data-[state=open]:rotate-90" />
            Claim mapping and advanced defaults
          </CollapsibleTrigger>
          <CollapsibleContent class="space-y-3">
            <div
              v-if="providerAdvancedVariableKeys.length > 0"
              class="mt-2 space-y-3 rounded-md border bg-muted/30 p-3"
            >
              <p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Claim Mapping Settings
              </p>
              <div v-for="key in providerAdvancedVariableKeys" :key="key" class="space-y-1.5">
                <Label :for="`var-${key}`" class="text-sm font-medium">
                  {{ formatLabel(key) }}
                </Label>
                <p v-if="detail.variables[key].description" class="text-xs text-muted-foreground">
                  {{ detail.variables[key].description }}
                </p>
                <Input
                  :id="`var-${key}`"
                  :type="detail.variables[key].sensitive ? 'password' : 'text'"
                  :model-value="formValues[key] ?? detail.variables[key].default ?? ''"
                  :placeholder="String(detail.variables[key].default || '')"
                  class="h-9 bg-background"
                  @update:model-value="formValues[key] = $event"
                />
              </div>
            </div>

            <div
              v-if="providerClaimMappings.length > 0"
              class="mt-2 space-y-2 rounded-md border bg-muted/30 p-3"
            >
              <p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Default Claim Mapping
              </p>
              <div
                v-for="[field, expr] in providerClaimMappings"
                :key="field"
                class="flex items-start gap-2 text-xs font-mono"
              >
                <span class="rounded bg-background px-1.5 py-0.5">{{ field }}</span>
                <span class="text-muted-foreground">→</span>
                <span class="break-all rounded bg-background px-1.5 py-0.5">{{ expr }}</span>
              </div>
            </div>

            <div class="rounded-md border bg-muted/30 p-3">
              <p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Preview Provider Configuration
              </p>
              <pre class="mt-2 overflow-auto max-h-48 text-xs font-mono">{{
                JSON.stringify(previewPayload, null, 2)
              }}</pre>
            </div>
          </CollapsibleContent>
        </Collapsible>

        <Collapsible v-else>
          <CollapsibleTrigger
            class="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer hover:text-foreground transition-colors"
          >
            <ChevronRight class="size-3 transition-transform data-[state=open]:rotate-90" />
            Preview resolved payload
          </CollapsibleTrigger>
          <CollapsibleContent>
            <pre
              class="mt-2 rounded-md border bg-muted/30 p-3 text-xs font-mono overflow-auto max-h-48"
              >{{ JSON.stringify(previewPayload, null, 2) }}</pre
            >
          </CollapsibleContent>
        </Collapsible>
      </div>

      <DialogFooter>
        <Button variant="outline" :disabled="installing" @click="$emit('update:open', false)">
          Cancel
        </Button>
        <Button :disabled="installing || loading || !!error" @click="install">
          <Spinner v-if="installing" class="mr-1.5 size-3.5" />
          {{ actionLabel }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { catalogApi, type CatalogTemplateDetail } from '@/api/resources'
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog'
  import { Button } from '@/components/ui/button'
  import { Badge } from '@/components/ui/badge'
  import { Input } from '@/components/ui/input'
  import { Label } from '@/components/ui/label'
  import { Switch } from '@/components/ui/switch'
  import { FormError } from '@/components/ui/form-error'
  import { Spinner } from '@/components/ui/spinner'
  import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
  import { notifyMutationError, notifyMutationSuccess } from '@/lib/notify'
  import { Download, ChevronRight } from 'lucide-vue-next'

  const props = defineProps<{
    open: boolean
    templateId: string
  }>()

  const emit = defineEmits<{
    'update:open': [value: boolean]
    installed: [result: { id: string; template_id: string; type: string }]
  }>()

  /** Extended type with proper variable/payload indexing (SDK generates {} for these). */
  type CatalogDetail = Omit<CatalogTemplateDetail, 'variables' | 'payload'> & {
    variables: Record<
      string,
      { type?: string; description?: string; default?: any; sensitive?: boolean }
    >
    payload: Record<string, any>
  }

  const detail = ref<CatalogDetail | null>(null)
  const loading = ref(false)
  const error = ref('')
  const installing = ref(false)
  const formValues = ref<Record<string, any>>({})

  const variableKeys = computed(() =>
    detail.value?.variables ? Object.keys(detail.value.variables) : [],
  )

  const isProviderTemplate = computed(() => detail.value?.template?.type === 'provider')

  // True if any variable lacks a default value (user must fill it in).
  const hasRequiredVariables = computed(() => {
    if (!detail.value?.variables) return false
    return Object.values(detail.value.variables).some((v) => v.default === undefined)
  })
  const genericVariableKeys = computed(() => variableKeys.value)

  const providerPrimaryVariableKeys = computed(() =>
    sortKeys(
      variableKeys.value.filter((key) => !isAdvancedProviderVariable(key)),
      [
        'provider_name',
        'issuer_url',
        'gitlab_url',
        'tenant_id',
        'client_id',
        'client_secret',
        'scopes',
      ],
    ),
  )

  const providerAdvancedVariableKeys = computed(() =>
    sortKeys(
      variableKeys.value.filter((key) => isAdvancedProviderVariable(key)),
      ['email_claim', 'name_claim'],
    ),
  )

  const dialogTitle = computed(() => {
    const name = detail.value?.template?.name || props.templateId
    if (isProviderTemplate.value) return `Create provider from ${name}`
    if (!hasRequiredVariables.value) return `Add ${name}`
    return `Configure ${name}`
  })

  const dialogDescription = computed(() => {
    if (!detail.value?.template?.description) return ''
    if (!isProviderTemplate.value) return detail.value.template.description
    return `${detail.value.template.description} This uses the template to create a configured provider instance; the template itself stays unchanged.`
  })

  const actionLabel = computed(() => {
    if (installing.value) {
      return isProviderTemplate.value ? 'Creating provider…' : 'Adding…'
    }
    if (isProviderTemplate.value) return 'Create provider'
    return hasRequiredVariables.value ? 'Add & Configure' : 'Add'
  })

  const providerClaimMappings = computed(() => {
    const claims = previewPayload.value?.mapping?.claims
    if (!claims || typeof claims !== 'object') return []
    return Object.entries(claims)
  })

  const providerTargetLabel = computed(() => {
    const target = previewPayload.value?.target
    if (!target) return 'Default'
    return target.schema_id || humanizeValue(target.schema_type) || 'Default'
  })

  const providerLinkingLabel = computed(() => {
    const linking = previewPayload.value?.linking
    if (!linking) return 'Default'
    return humanizeLinking(linking.mode, linking.match_by)
  })

  const providerProtocolLabel = computed(() =>
    String(detail.value?.payload?.protocol || 'provider').toUpperCase(),
  )

  const previewPayload = computed(() => {
    if (!detail.value?.payload) return {}
    return resolveTemplateValue(detail.value.payload, mergedVariables.value)
  })

  const mergedVariables = computed(() => {
    const resolved: Record<string, any> = {}
    if (!detail.value?.variables) return resolved
    for (const [key, spec] of Object.entries(detail.value.variables)) {
      resolved[key] = formValues.value[key] ?? spec.default ?? ''
    }
    return resolved
  })

  function formatLabel(key: string): string {
    return key.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
  }

  function humanizeValue(value?: string): string {
    if (!value) return ''
    return formatLabel(value)
  }

  function humanizeLinking(mode?: string, matchBy?: string): string {
    if (!mode && !matchBy) return 'Default'
    if (mode === 'create_or_link' && matchBy === 'verified_email') {
      return 'Create or link by verified email'
    }
    if (mode === 'link_only' && matchBy === 'verified_email') {
      return 'Link only by verified email'
    }
    if (mode === 'create_or_link' && matchBy) {
      return `Create or link by ${humanizeValue(matchBy).toLowerCase()}`
    }
    if (mode === 'link_only' && matchBy) {
      return `Link only by ${humanizeValue(matchBy).toLowerCase()}`
    }
    return humanizeValue(mode)
  }

  function isAdvancedProviderVariable(key: string): boolean {
    return key.endsWith('_claim') || key.startsWith('claim_') || key.includes('_claim_')
  }

  function sortKeys(keys: string[], priority: string[]): string[] {
    const priorityIndex = new Map(priority.map((key, index) => [key, index]))
    return [...keys].sort((a, b) => {
      const left = priorityIndex.get(a) ?? priority.length + 1
      const right = priorityIndex.get(b) ?? priority.length + 1
      if (left !== right) return left - right
      return a.localeCompare(b)
    })
  }

  function resolveTemplateValue(value: any, variables: Record<string, any>): any {
    if (Array.isArray(value)) {
      return value.map((item) => resolveTemplateValue(item, variables))
    }
    if (value && typeof value === 'object') {
      return Object.fromEntries(
        Object.entries(value).map(([key, item]) => [key, resolveTemplateValue(item, variables)]),
      )
    }
    if (typeof value === 'string') {
      return value.replace(/\{\{(\w+)\}\}/g, (_, key: string) => {
        const resolved = variables[key]
        return resolved == null ? '' : String(resolved)
      })
    }
    return value
  }

  // Fetch template details when opened
  watch(
    () => props.open,
    async (isOpen) => {
      if (!isOpen || !props.templateId) return
      loading.value = true
      error.value = ''
      formValues.value = {}

      try {
        detail.value = (await catalogApi.get(props.templateId)) as unknown as CatalogDetail
        // Pre-fill defaults
        if (detail.value.variables) {
          for (const [key, v] of Object.entries(detail.value.variables)) {
            if (v.default !== undefined) {
              formValues.value[key] = v.default
            }
          }
        }
      } catch (e: any) {
        error.value = e.message || 'Failed to load template'
      } finally {
        loading.value = false
      }
    },
    { immediate: true },
  )

  async function install() {
    if (!props.templateId) return
    installing.value = true
    try {
      const result = await catalogApi.install(props.templateId, formValues.value)
      const templateType = detail.value?.template?.type || ''
      const providerName = String(
        formValues.value.provider_name ||
          previewPayload.value?.display_name ||
          detail.value?.template?.name ||
          props.templateId,
      )
      if (isProviderTemplate.value) {
        notifyMutationSuccess('Provider', 'create', `"${providerName}" is now available in Providers.`)
      } else {
        notifyMutationSuccess(detail.value?.template?.name || props.templateId, 'install', `Entity ${result.id} created`)
      }
      emit('installed', {
        id: result.id,
        template_id: result.template_id,
        type: templateType,
      })
      emit('update:open', false)
    } catch (e: any) {
      notifyMutationError(
        isProviderTemplate.value ? 'Provider' : (detail.value?.template?.name || props.templateId),
        isProviderTemplate.value ? 'create' : 'install',
        e,
      )
    } finally {
      installing.value = false
    }
  }
</script>
