<template>
  <div class="space-y-6">
    <!-- Loading -->
    <div v-if="loading" class="flex h-40 items-center justify-center text-muted-foreground">
      Loading template...
    </div>

    <!-- Error -->
    <div v-else-if="error" class="text-destructive text-sm">{{ error }}</div>

    <template v-else-if="detail">
      <!-- Header -->
      <div class="flex items-start justify-between gap-4">
        <div class="space-y-2">
          <div class="flex items-center gap-2">
            <Button variant="ghost" size="icon" class="size-8" @click="$router.push('/marketplace')">
              <ArrowLeft class="size-4" />
            </Button>
            <h1 class="text-2xl font-semibold tracking-tight">{{ detail.template.name }}</h1>
          </div>
          <div class="flex items-center gap-2 flex-wrap ml-10">
            <Badge variant="secondary" class="capitalize">
              <component :is="typeIcons[detail.template.type] || Package" class="size-3 mr-1" />
              {{ detail.template.type === 'login_flow' ? 'Login Flow' : detail.template.type }}
            </Badge>
            <Badge variant="outline" class="font-mono text-xs">v{{ detail.template.version }}</Badge>
            <Badge
              v-for="tag in detail.template.tags"
              :key="tag"
              variant="outline"
              class="text-xs"
            >
              {{ tag }}
            </Badge>
          </div>
        </div>
        <Button class="shrink-0" @click="handleAdd" :disabled="adding">
          <Plus class="size-4 mr-1.5" />
          {{ adding ? 'Adding...' : addButtonLabel }}
        </Button>
      </div>

      <!-- Description -->
      <p class="text-sm text-muted-foreground ml-10">{{ detail.template.description }}</p>

      <!-- What you'll get — type-specific preview -->
      <Card class="ml-10">
        <CardHeader>
          <CardTitle class="text-sm">What you'll get</CardTitle>
        </CardHeader>
        <CardContent>
          <!-- Action preview -->
          <div v-if="detail.template.type === 'action'" class="space-y-4">
            <div class="grid grid-cols-2 gap-4 text-sm">
              <div>
                <label class="text-xs font-medium text-muted-foreground">Hook</label>
                <p class="font-medium">{{ payload.hook || 'on_event' }}</p>
                <p class="text-xs text-muted-foreground mt-0.5">
                  {{ hookExplanations[payload.hook] || '' }}
                </p>
              </div>
              <div>
                <label class="text-xs font-medium text-muted-foreground">Action Type</label>
                <p class="font-medium">{{ payload.action_type || 'expr' }}</p>
              </div>
            </div>
            <div v-if="payload.trigger">
              <label class="text-xs font-medium text-muted-foreground">Trigger Expression</label>
              <pre class="mt-1 p-3 rounded-md bg-muted text-xs font-mono">{{ payload.trigger }}</pre>
            </div>
            <div class="grid grid-cols-3 gap-4 text-sm">
              <div>
                <label class="text-xs font-medium text-muted-foreground">Priority</label>
                <p>{{ payload.priority ?? 0 }}</p>
              </div>
              <div>
                <label class="text-xs font-medium text-muted-foreground">Enabled</label>
                <p>{{ payload.enabled !== false ? 'Yes' : 'No' }}</p>
              </div>
              <div>
                <label class="text-xs font-medium text-muted-foreground">Fail Open</label>
                <p>{{ payload.fail_open ? 'Yes' : 'No' }}</p>
              </div>
            </div>
          </div>

          <!-- Provider preview -->
          <div v-else-if="detail.template.type === 'provider'" class="space-y-4">
            <div class="grid grid-cols-3 gap-4 text-sm">
              <div>
                <label class="text-xs font-medium text-muted-foreground">Protocol</label>
                <p class="font-medium uppercase">{{ payload.protocol || 'oidc' }}</p>
              </div>
              <div>
                <label class="text-xs font-medium text-muted-foreground">Target Schema</label>
                <p>{{ payload.target?.schema_type || 'human_user' }}</p>
              </div>
              <div>
                <label class="text-xs font-medium text-muted-foreground">Linking</label>
                <p>{{ humanize(payload.linking?.mode || 'create_or_link') }}</p>
              </div>
            </div>
            <div v-if="payload.connection?.issuer">
              <label class="text-xs font-medium text-muted-foreground">Issuer</label>
              <p class="text-sm font-mono">{{ payload.connection.issuer }}</p>
            </div>
            <div v-if="claimMappings.length">
              <label class="text-xs font-medium text-muted-foreground">Default Claim Mappings</label>
              <div class="mt-1 grid grid-cols-2 gap-1 text-xs">
                <template v-for="[field, expr] in claimMappings" :key="field">
                  <span class="font-medium">{{ field }}</span>
                  <span class="font-mono text-muted-foreground">{{ expr }}</span>
                </template>
              </div>
            </div>
          </div>

          <!-- Generic preview for other types -->
          <div v-else>
            <pre class="p-3 rounded-md bg-muted text-xs font-mono whitespace-pre-wrap overflow-auto max-h-60">{{
              JSON.stringify(payload, null, 2)
            }}</pre>
          </div>
        </CardContent>
      </Card>

      <!-- Configuration options (read-only table) -->
      <Card v-if="variableEntries.length" class="ml-10">
        <CardHeader>
          <CardTitle class="text-sm">Configuration Options</CardTitle>
          <p class="text-xs text-muted-foreground">
            These can be customized when adding, or changed later in the resource settings.
          </p>
        </CardHeader>
        <CardContent>
          <div class="border rounded-md overflow-hidden">
            <table class="w-full text-sm">
              <thead class="bg-muted/50">
                <tr>
                  <th class="text-left px-3 py-2 text-xs font-medium text-muted-foreground">Name</th>
                  <th class="text-left px-3 py-2 text-xs font-medium text-muted-foreground">Type</th>
                  <th class="text-left px-3 py-2 text-xs font-medium text-muted-foreground">Default</th>
                  <th class="text-left px-3 py-2 text-xs font-medium text-muted-foreground">Description</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="[key, v] in variableEntries" :key="key" class="border-t">
                  <td class="px-3 py-2 font-mono text-xs">{{ key }}</td>
                  <td class="px-3 py-2 text-xs text-muted-foreground">{{ v.type || 'string' }}</td>
                  <td class="px-3 py-2 text-xs">
                    <span v-if="v.sensitive" class="text-muted-foreground italic">sensitive</span>
                    <span v-else-if="v.default !== undefined" class="font-mono">{{ v.default }}</span>
                    <span v-else class="text-muted-foreground italic">required</span>
                  </td>
                  <td class="px-3 py-2 text-xs text-muted-foreground">{{ v.description || '' }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <!-- Payload preview (collapsed) -->
      <Collapsible class="ml-10">
        <CollapsibleTrigger class="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors cursor-pointer">
          <ChevronRight class="size-3.5 transition-transform data-[state=open]:rotate-90" />
          Preview full payload
        </CollapsibleTrigger>
        <CollapsibleContent>
          <pre class="mt-2 p-3 rounded-md bg-muted text-xs font-mono whitespace-pre-wrap overflow-auto max-h-80">{{
            JSON.stringify(payload, null, 2)
          }}</pre>
        </CollapsibleContent>
      </Collapsible>
    </template>

    <!-- Install dialog (for providers and actions with required vars) -->
    <CatalogInstallDialog
      v-model:open="showInstallDialog"
      :template-id="templateId"
      @installed="onInstalled"
    />
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import { toast } from 'vue-sonner'
  import { catalogApi } from '@/api/resources'
  import CatalogInstallDialog from '@/console/components/catalog/CatalogInstallDialog.vue'

  import { Button } from '@/components/ui/button'
  import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
  import { Badge } from '@/components/ui/badge'
  import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
  import {
    ArrowLeft,
    Plus,
    ChevronRight,
    Package,
    Globe,
    Zap,
    ShieldCheck,
    KeyRound,
    FileJson,
  } from 'lucide-vue-next'

  const route = useRoute()
  const router = useRouter()
  const templateId = computed(() => String(route.params.id || ''))

  const detail = ref<any>(null)
  const loading = ref(true)
  const error = ref('')
  const adding = ref(false)
  const showInstallDialog = ref(false)

  const typeIcons: Record<string, any> = {
    action: Zap,
    provider: Globe,
    authorization: ShieldCheck,
    login_flow: KeyRound,
    schema: FileJson,
  }

  const hookExplanations: Record<string, string> = {
    on_request: 'Runs on every incoming HTTP request before authentication',
    pre_auth: 'Runs before authentication methods are evaluated',
    auth: 'Runs during the authentication step',
    post_auth: 'Runs after successful authentication',
    on_token: 'Runs when tokens are issued or refreshed',
    on_event: 'Runs asynchronously when events are emitted',
  }

  const payload = computed(() => detail.value?.payload || {})

  const claimMappings = computed(() => {
    const claims = payload.value?.mapping?.claims
    if (!claims || typeof claims !== 'object') return []
    return Object.entries(claims)
  })

  const variableEntries = computed(() => {
    if (!detail.value?.variables) return []
    return Object.entries(detail.value.variables) as [string, any][]
  })

  const hasRequiredVariables = computed(() =>
    variableEntries.value.some(([, v]) => v.default === undefined && !v.sensitive),
  )

  const addButtonLabel = computed(() => {
    const type = detail.value?.template?.type
    const labels: Record<string, string> = {
      provider: 'Add Provider',
      action: 'Add Action',
      login_flow: 'Add Login Flow',
      authorization: 'Add to Authorization',
    }
    return labels[type] || 'Add'
  })

  function humanize(s: string): string {
    return s.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
  }

  onMounted(async () => {
    try {
      detail.value = await catalogApi.get(templateId.value)
    } catch (e: any) {
      error.value = e.message || 'Template not found'
    } finally {
      loading.value = false
    }
  })

  async function handleAdd() {
    const type = detail.value?.template?.type

    // Providers always need the credential dialog.
    if (type === 'provider' || hasRequiredVariables.value) {
      showInstallDialog.value = true
      return
    }

    // Actions with all defaults: install immediately.
    adding.value = true
    try {
      const result = await catalogApi.install(templateId.value, {})
      onInstalled(result)
    } catch (e: any) {
      toast.error('Failed to add', { description: e.message })
    } finally {
      adding.value = false
    }
  }

  function onInstalled(result: any) {
    const type = detail.value?.template?.type || result.type
    const name = detail.value?.template?.name || templateId.value
    toast.success(`${name} added`, {
      description: `Available in ${humanize(type)}s.`,
    })

    const detailRoutes: Record<string, (id: string) => string> = {
      action: (id) => `/actions/${id}`,
      provider: (id) => `/providers/${id}`,
    }
    const listRoutes: Record<string, string> = {
      login_flow: '/login-flows',
      authorization: '/authorization',
    }

    const detailRoute = detailRoutes[type]
    if (detailRoute && result.id) {
      router.push(detailRoute(result.id))
    } else {
      router.push(listRoutes[type] || '/marketplace')
    }
  }
</script>
