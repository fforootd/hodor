<template>
  <div class="zitadel-wc" :class="{ dark: isDark }">
    <WCToaster :dark="isDark" />

    <div class="space-y-4">
      <!-- Header -->
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Providers</h2>
          <p class="text-sm text-[var(--color-muted-foreground)]">
            {{ providers.length }} provider{{ providers.length !== 1 ? 's' : '' }} configured
          </p>
        </div>
        <button
          v-if="showCreate"
          class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity"
          @click="showCreatePanel = !showCreatePanel"
        >
          {{ showCreatePanel ? 'Cancel' : '+ Add Provider' }}
        </button>
      </div>

      <!-- Template Picker -->
      <div v-if="showCreatePanel && !selectedTemplate" class="space-y-3">
        <h3 class="text-sm font-semibold">Choose a provider template</h3>
        <p class="text-sm text-[var(--color-muted-foreground)]">
          Templates define provider defaults. You can create multiple configured providers from the
          same template.
        </p>
        <div class="grid grid-cols-2 gap-3">
          <div
            v-for="t in templates"
            :key="t.id"
            class="rounded-lg border border-[var(--color-border)] p-4 cursor-pointer hover:border-[var(--color-primary)] transition-colors bg-[var(--color-card)]"
            @click="pickTemplate(t)"
          >
            <div class="text-2xl mb-2">{{ templateIcon(t.id) }}</div>
            <div class="text-sm font-semibold">{{ t.name }}</div>
            <p class="text-xs text-[var(--color-muted-foreground)] mt-1">{{ t.description }}</p>
            <span
              class="inline-flex items-center rounded-full border border-[var(--color-border)] px-2 py-0.5 text-[10px] font-medium uppercase mt-2"
              >{{ t.protocol }}</span
            >
          </div>
        </div>
      </div>

      <!-- Create Form -->
      <div
        v-if="showCreatePanel && selectedTemplate"
        class="rounded-lg border border-[var(--color-border)] p-4 space-y-4 bg-[var(--color-card)]"
      >
        <div class="space-y-1">
          <h3 class="text-sm font-semibold">Create provider from {{ selectedTemplate.name }}</h3>
          <p class="text-sm text-[var(--color-muted-foreground)]">
            These settings apply only to the provider instance you are creating. The template stays
            unchanged.
          </p>
        </div>
        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Provider Name</label>
            <input
              v-model="createForm.name"
              type="text"
              placeholder="e.g. Google Production"
              class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Issuer</label>
            <input
              v-model="createForm.issuer"
              type="text"
              placeholder="https://accounts.google.com"
              class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Client ID</label>
            <input
              v-model="createForm.client_id"
              type="text"
              placeholder="your-client-id"
              class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Client Secret</label>
            <input
              v-model="createForm.client_secret"
              type="password"
              placeholder="your-client-secret"
              class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-sm font-medium">Scopes</label>
            <input
              v-model="createForm.scopes"
              type="text"
              placeholder="openid email profile"
              class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
          </div>
          <div class="flex items-center gap-2 self-end pb-1">
            <input
              id="wc-prov-auto"
              v-model="createForm.auto_register"
              type="checkbox"
              class="accent-[var(--color-primary)]"
            />
            <label for="wc-prov-auto" class="text-sm cursor-pointer"
              >Create or link users automatically</label
            >
          </div>
        </div>

        <div class="flex justify-end gap-2">
          <button
            class="inline-flex items-center rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm hover:bg-[var(--color-muted)] transition-colors"
            @click="selectedTemplate = null"
          >
            ← Back
          </button>
          <button
            class="inline-flex items-center justify-center rounded-md text-sm font-medium h-9 px-4 py-2 bg-[var(--color-primary)] text-[var(--color-primary-foreground)] hover:opacity-90 transition-opacity disabled:opacity-50"
            :disabled="!createForm.name || !createForm.issuer || !createForm.client_id || creating"
            @click="createProvider"
          >
            {{ creating ? 'Creating provider…' : 'Create provider' }}
          </button>
        </div>
      </div>

      <!-- Search -->
      <div v-if="showSearch && !showCreatePanel" class="relative">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search providers…"
          class="w-full h-9 rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-1 text-sm shadow-sm placeholder:text-[var(--color-muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
        />
      </div>

      <!-- Loading -->
      <div
        v-if="loading"
        class="flex justify-center py-12 text-sm text-[var(--color-muted-foreground)]"
      >
        Loading providers…
      </div>

      <!-- Error -->
      <div
        v-if="loadError"
        class="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
      >
        {{ loadError }}
      </div>

      <!-- Table -->
      <div
        v-if="!loading && filteredProviders.length && !showCreatePanel"
        class="rounded-md border border-[var(--color-border)] overflow-hidden"
      >
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b bg-[var(--color-muted)]">
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">
                Name
              </th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">
                Protocol
              </th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">
                Template
              </th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">
                Status
              </th>
              <th class="h-10 px-4 text-left font-medium text-[var(--color-muted-foreground)]">
                Created
              </th>
              <th class="h-10 px-4 text-right font-medium text-[var(--color-muted-foreground)]">
                Actions
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="p in filteredProviders"
              :key="p.id"
              class="border-b last:border-0 hover:bg-[var(--color-muted)] cursor-pointer transition-colors"
              @click="onProviderClick(p)"
            >
              <td class="p-4">
                <div class="flex items-center gap-2">
                  <span>{{ templateIcon(providerTemplateId(p)) }}</span>
                  <span class="font-medium">{{ providerDisplayName(p) }}</span>
                </div>
              </td>
              <td class="p-4">
                <span
                  class="inline-flex items-center rounded-full border border-[var(--color-border)] px-2 py-0.5 text-xs font-medium uppercase"
                  >{{ p.protocol }}</span
                >
              </td>
              <td class="p-4 text-[var(--color-muted-foreground)]">{{ providerTemplateId(p) }}</td>
              <td class="p-4">
                <span
                  :class="[
                    'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium border',
                    p.enabled
                      ? 'bg-green-50 text-green-700 border-green-200'
                      : 'bg-red-50 text-red-700 border-red-200',
                  ]"
                  >{{ p.enabled ? 'enabled' : 'disabled' }}</span
                >
              </td>
              <td class="p-4 text-[var(--color-muted-foreground)]">
                {{ formatDate(p.created_at) }}
              </td>
              <td class="p-4 text-right">
                <div class="flex items-center justify-end gap-1">
                  <button
                    class="inline-flex items-center rounded-md border border-[var(--color-border)] px-2 py-1 text-xs hover:bg-[var(--color-muted)] transition-colors"
                    @click.stop="toggleEnabled(p)"
                  >
                    {{ p.enabled ? 'Disable' : 'Enable' }}
                  </button>
                  <button
                    class="inline-flex items-center rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50 transition-colors"
                    @click.stop="requestDeleteProvider(p)"
                  >
                    Delete
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Empty -->
      <div
        v-if="!loading && !loadError && !filteredProviders.length && !showCreatePanel"
        class="text-center py-12 text-sm text-[var(--color-muted-foreground)]"
      >
        {{ searchQuery ? 'No providers match your search.' : 'No providers configured yet.' }}
      </div>
    </div>

    <WCConfirmDialog
      :open="providerPendingDelete !== null"
      title="Delete Provider"
      :description="`Are you sure you want to delete ${providerPendingDelete ? providerDisplayName(providerPendingDelete) : 'this provider'}? This action cannot be undone.`"
      :loading="deleting"
      @update:open="handleDeleteDialogOpen"
      @confirm="confirmDeleteProvider"
    />
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, reactive, onMounted } from 'vue'
  import { notifyError, notifyMutationError, notifyMutationSuccess, notifySuccess } from '@/lib/notify'
  import WCConfirmDialog from '@/wc/components/WCConfirmDialog.vue'
  import WCToaster from '@/wc/components/WCToaster.vue'
  import { createWCApiClient, type WCApiClient } from '@/wc/wc-api-client'
  import { dispatchWCEvent, resolveApiBase, isDarkMode } from '@/wc/host-utils'

  const TAG_NAME = 'zitadel-provider-list'

  const props = withDefaults(
    defineProps<{
      apiBaseUrl?: string
      darkMode?: string
      showSearch?: boolean
      showCreate?: boolean
    }>(),
    {
      apiBaseUrl: '',
      darkMode: '',
      showSearch: true,
      showCreate: true,
    },
  )

  const isDark = computed(() => isDarkMode(props.darkMode))

  let api: WCApiClient
  const providers = ref<any[]>([])
  const templates = ref<any[]>([])
  const loading = ref(false)
  const loadError = ref('')
  const searchQuery = ref('')
  const showCreatePanel = ref(false)
  const selectedTemplate = ref<any>(null)
  const creating = ref(false)
  const deleting = ref(false)
  const providerPendingDelete = ref<any | null>(null)

  const createForm = reactive({
    name: '',
    issuer: '',
    client_id: '',
    client_secret: '',
    scopes: 'openid email profile',
    auto_register: true,
  })

  const filteredProviders = computed(() => {
    if (!searchQuery.value.trim()) return providers.value
    const q = searchQuery.value.toLowerCase()
    return providers.value.filter(
      (p) =>
        providerDisplayName(p).toLowerCase().includes(q) ||
        (p.protocol || '').toLowerCase().includes(q) ||
        providerTemplateId(p).toLowerCase().includes(q),
    )
  })

  function templateIcon(id: string): string {
    const icons: Record<string, string> = {
      google: '🔵',
      'google-oidc': '🔵',
      entra: '🟦',
      'entra-id': '🟦',
      github: '🐙',
      gitlab: '🦊',
      'custom-oidc': '⚙',
      apple: '🍎',
      custom: '⚙',
    }
    return icons[id] || '🔗'
  }

  function providerDisplayName(provider: any): string {
    return provider.display_name || provider.name || 'Unnamed provider'
  }

  function providerTemplateId(provider: any): string {
    return provider.catalog_ref?.template_id || provider.template || provider.kind || 'custom'
  }

  function formatDate(ts: string): string {
    if (!ts) return '—'
    return new Date(ts).toLocaleDateString()
  }

  function onProviderClick(p: any) {
    dispatchWCEvent(TAG_NAME, 'provider-selected', {
      id: p.id,
      name: providerDisplayName(p),
      protocol: p.protocol,
      enabled: p.enabled,
    })
  }

  function pickTemplate(t: any) {
    selectedTemplate.value = t
    createForm.name = ''
    createForm.issuer = t.default_config?.issuer || ''
    createForm.scopes = t.default_config?.scopes || 'openid email profile'
    createForm.client_id = ''
    createForm.client_secret = ''
  }

  async function createProvider() {
    creating.value = true
    try {
      const result = await api.post<any>('/v1/providers', {
        display_name: createForm.name,
        kind: selectedTemplate.value?.id || 'custom',
        protocol: selectedTemplate.value?.protocol || 'oidc',
        catalog_ref: {
          template_id: selectedTemplate.value?.id || 'custom',
        },
        connection: {
          issuer: createForm.issuer,
          client_id: createForm.client_id,
          client_secret: createForm.client_secret,
          scopes: createForm.scopes,
        },
        linking: {
          mode: createForm.auto_register ? 'create_or_link' : 'link_only',
          match_by: 'verified_email',
        },
      })
      dispatchWCEvent(TAG_NAME, 'provider-created', { id: result.id, name: createForm.name })
      showCreatePanel.value = false
      selectedTemplate.value = null
      await loadProviders()
      notifyMutationSuccess('Provider', 'create')
    } catch (e: any) {
      notifyMutationError('Provider', 'create', e)
      dispatchWCEvent(TAG_NAME, 'provider-error', { error: e?.message || 'Create failed' })
    } finally {
      creating.value = false
    }
  }

  async function toggleEnabled(p: any) {
    try {
      await api.patch(`/v1/providers/${p.id}`, { enabled: !p.enabled })
      dispatchWCEvent(TAG_NAME, 'provider-toggled', { id: p.id, enabled: !p.enabled })
      await loadProviders()
      notifySuccess(p.enabled ? 'Provider disabled' : 'Provider enabled')
    } catch (e: any) {
      notifyMutationError('Provider', 'toggle', e)
    }
  }

  function requestDeleteProvider(provider: any) {
    providerPendingDelete.value = provider
  }

  function handleDeleteDialogOpen(next: boolean) {
    if (!next) providerPendingDelete.value = null
  }

  async function confirmDeleteProvider() {
    if (!providerPendingDelete.value) return
    deleting.value = true
    try {
      await api.delete(`/v1/providers/${providerPendingDelete.value.id}`)
      dispatchWCEvent(TAG_NAME, 'provider-deleted', { id: providerPendingDelete.value.id })
      await loadProviders()
      notifyMutationSuccess('Provider', 'delete')
      providerPendingDelete.value = null
    } catch (e: any) {
      notifyMutationError('Provider', 'delete', e)
    } finally {
      deleting.value = false
    }
  }

  async function loadProviders() {
    try {
      const data = await api.get<any>('/v1/providers')
      providers.value = data.providers || []
      loadError.value = ''
    } catch (e: any) {
      loadError.value = e?.message || 'Failed to load providers'
    }
  }

  async function loadTemplates() {
    try {
      const data = await api.get<any>('/v1/providers/templates')
      templates.value = data.templates || []
    } catch (e: any) {
      notifyError('Failed to load provider templates', e)
    }
  }

  onMounted(async () => {
    api = createWCApiClient(resolveApiBase(props.apiBaseUrl))
    loading.value = true
    await Promise.all([loadProviders(), loadTemplates()])
    loading.value = false
  })
</script>

<style>
  :host {
    display: block;
    font-family:
      'Inter',
      ui-sans-serif,
      system-ui,
      -apple-system,
      sans-serif;
    --color-background: hsl(0 0% 100%);
    --color-foreground: hsl(240 10% 3.9%);
    --color-card: hsl(0 0% 100%);
    --color-primary: hsl(240 5.9% 10%);
    --color-primary-foreground: hsl(0 0% 98%);
    --color-muted: hsl(240 4.8% 95.9%);
    --color-muted-foreground: hsl(240 3.8% 46.1%);
    --color-border: hsl(240 5.9% 90%);
    --color-input: hsl(240 5.9% 90%);
    --color-ring: hsl(240 5.9% 10%);
    --radius: 0.5rem;
  }
  :host(.dark) {
    --color-background: hsl(240 10% 3.9%);
    --color-foreground: hsl(0 0% 98%);
    --color-card: hsl(240 10% 3.9%);
    --color-primary: hsl(0 0% 98%);
    --color-primary-foreground: hsl(240 5.9% 10%);
    --color-muted: hsl(240 3.7% 15.9%);
    --color-muted-foreground: hsl(240 5% 64.9%);
    --color-border: hsl(240 3.7% 15.9%);
    --color-input: hsl(240 3.7% 15.9%);
    --color-ring: hsl(240 4.9% 83.9%);
  }
  .zitadel-wc {
    color: var(--color-foreground);
    background: var(--color-background);
    padding: 1rem;
  }
  .zitadel-wc.dark {
    color-scheme: dark;
  }
</style>
