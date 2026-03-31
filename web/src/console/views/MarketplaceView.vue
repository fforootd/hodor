<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Marketplace</h1>
        <p class="text-sm text-muted-foreground mt-1">
          Browse catalog templates for providers, actions, schemas, and more, then create the
          resources you need from them.
        </p>
      </div>
      <Button
        variant="ghost"
        size="sm"
        class="gap-1.5 text-xs"
        :disabled="refreshing || !canRefresh"
        @click="refreshCatalog"
      >
        <RefreshCw class="size-3.5" :class="refreshing ? 'animate-spin' : ''" />
        Refresh
      </Button>
    </div>

    <!-- Filter bar -->
    <div class="flex items-center gap-3 flex-wrap">
      <div class="relative w-full max-w-sm">
        <SearchIcon class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
        <Input v-model="searchQuery" placeholder="Search templates…" class="pl-9 bg-background" />
      </div>
      <!-- Type filter pills -->
      <div class="flex gap-1.5 flex-wrap">
        <Button
          v-for="type in catalogTypes"
          :key="type"
          size="sm"
          variant="outline"
          class="h-7 text-xs capitalize"
          :class="
            typeFilter === type ? 'bg-primary text-primary-foreground hover:bg-primary/90' : ''
          "
          @click="typeFilter = typeFilter === type ? '' : type"
        >
          <component :is="typeIcons[type] || Package" class="size-3 mr-1" />
          {{ type }}
        </Button>
      </div>
      <span class="text-xs text-muted-foreground ml-auto tabular-nums">
        {{ filteredTemplates.length }} template{{ filteredTemplates.length !== 1 ? 's' : '' }}
      </span>
    </div>

    <!-- Template cards grid -->
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      <Card
        v-for="tpl in filteredTemplates"
        :key="tpl.id"
        class="group cursor-pointer transition-all hover:shadow-md hover:border-primary/30"
        @click="openTemplate(tpl)"
      >
        <CardHeader class="pb-2">
          <div class="flex items-center justify-between">
            <CardTitle class="text-sm font-semibold group-hover:text-primary transition-colors">{{
              tpl.name
            }}</CardTitle>
            <Badge variant="secondary" class="text-[10px] shrink-0 capitalize">{{
              tpl.type === 'provider' ? 'provider template' : tpl.type
            }}</Badge>
          </div>
          <p class="text-xs text-muted-foreground line-clamp-2">{{ tpl.description }}</p>
        </CardHeader>
        <CardContent class="pt-0 pb-3">
          <div class="flex items-center gap-1.5 flex-wrap">
            <Badge
              v-for="tag in tpl.tags?.slice(0, 3)"
              :key="tag"
              variant="outline"
              class="text-[10px] font-normal"
            >
              {{ tag }}
            </Badge>
            <Badge
              v-if="tpl.tags?.length > 3"
              variant="outline"
              class="text-[10px] font-normal text-muted-foreground"
            >
              +{{ tpl.tags.length - 3 }}
            </Badge>
            <span class="ml-auto text-[10px] text-muted-foreground">
              <span class="font-mono">v{{ tpl.version }}</span>
              <span v-if="tpl.source" class="ml-1.5 opacity-60">{{ tpl.source }}</span>
            </span>
          </div>
          <p
            v-if="tpl.type === 'provider'"
            class="mt-3 text-[11px] font-medium text-primary/80 group-hover:text-primary"
          >
            Create provider in Providers
          </p>
        </CardContent>
      </Card>
    </div>

    <!-- Empty states -->
    <div
      v-if="!filteredTemplates.length && searchQuery"
      class="flex h-32 items-center justify-center text-muted-foreground text-sm"
    >
      No templates matching "{{ searchQuery }}"
    </div>
    <div v-if="loading" class="flex h-32 items-center justify-center text-muted-foreground">
      <Spinner class="mr-2" /> Loading marketplace…
    </div>
    <div
      v-if="!loading && !allTemplates.length && !searchQuery"
      class="flex flex-col h-40 items-center justify-center gap-3 text-muted-foreground"
    >
      <Package class="size-10 opacity-20" />
      <p class="text-sm">No templates available</p>
      <p class="text-xs max-w-xs text-center">
        Configure a catalog source in Settings, or templates will load from the built-in catalog on
        next refresh.
      </p>
      <Button
        variant="outline"
        size="sm"
        class="gap-1.5"
        :disabled="!canRefresh"
        @click="refreshCatalog"
      >
        <RefreshCw class="size-3.5" />
        Refresh Catalog
      </Button>
    </div>

    <!-- Template Dialog -->
    <CatalogInstallDialog
      v-model:open="showInstallDialog"
      :template-id="installTemplateId"
      @installed="onInstalled"
    />
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import { toast } from 'vue-sonner'
  import { ApiError } from '@/api/client'
  import { catalogApi, type CatalogTemplate } from '@/api/resources'
  import CatalogInstallDialog from '@/console/components/catalog/CatalogInstallDialog.vue'

  import { Button } from '@/components/ui/button'
  import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
  import { Badge } from '@/components/ui/badge'
  import { Input } from '@/components/ui/input'
  import { Spinner } from '@/components/ui/spinner'
  import {
    Search as SearchIcon,
    RefreshCw,
    Package,
    Zap,
    Globe,
    ShieldCheck,
    FileJson,
    KeyRound,
  } from 'lucide-vue-next'

  const router = useRouter()
  const route = useRoute()

  // ─── State ───
  const allTemplates = ref<CatalogTemplate[]>([])
  const searchQuery = ref('')
  const typeFilter = ref('')
  const loading = ref(false)
  const refreshing = ref(false)
  const canRefresh = ref(false)
  const showInstallDialog = ref(false)
  const installTemplateId = ref('')

  // Icon map for template types
  const typeIcons: Record<string, any> = {
    action: Zap,
    provider: Globe,
    authorization: ShieldCheck,
    schema: FileJson,
    login_flow: KeyRound,
  }

  // ─── Computed ───
  const catalogTypes = computed(() => {
    const types = new Set(allTemplates.value.map((t) => t.type))
    return Array.from(types).sort()
  })

  const filteredTemplates = computed(() => {
    let result = allTemplates.value
    if (typeFilter.value) {
      result = result.filter((t) => t.type === typeFilter.value)
    }
    if (searchQuery.value) {
      const q = searchQuery.value.toLowerCase()
      result = result.filter(
        (t) =>
          t.name.toLowerCase().includes(q) ||
          t.description?.toLowerCase().includes(q) ||
          t.tags?.some((tag) => tag.toLowerCase().includes(q)),
      )
    }
    return result
  })

  // ─── Actions ───
  function openTemplate(template: CatalogTemplate) {
    if (template.type === 'provider') {
      void router.push({
        path: '/providers/new',
        query: {
          template: template.id,
          source: 'marketplace',
        },
      })
      return
    }
    installTemplateId.value = template.id
    showInstallDialog.value = true
  }

  function onInstalled(result: { id: string; template_id: string; type: string }) {
    const successTitle = result.type === 'provider' ? 'Provider created' : 'Template installed'
    const successDescription =
      result.type === 'provider'
        ? `${result.template_id} was used to create a new provider instance.`
        : `${result.template_id} is now available.`
    toast.success(successTitle, { description: successDescription })
    // Navigate to the resource type's list view
    const typeRoutes: Record<string, string> = {
      action: '/actions',
      provider: '/providers',
      schema: '/schemas',
      authorization: '/authorization',
      login_flow: '/login-flows',
    }
    const target = typeRoutes[result.type]
    if (target) router.push(target)
  }

  async function refreshCatalog() {
    if (!canRefresh.value) {
      return
    }

    refreshing.value = true
    try {
      const res = await catalogApi.refresh()
      toast.success('Catalog refreshed', { description: `${res.new} new templates` })
      const list = await catalogApi.list()
      allTemplates.value = list.templates || []
      canRefresh.value = list.can_refresh
    } catch (e: any) {
      if (e instanceof ApiError && e.status === 409) {
        toast.message('Refresh not configured', {
          description: 'Remote catalog refresh is not configured here yet. Built-in templates are still available.',
        })
        canRefresh.value = false
        return
      }
      toast.error('Refresh failed', { description: e.message })
    } finally {
      refreshing.value = false
    }
  }

  // ─── Init ───
  onMounted(async () => {
    const initialType = route.query.type
    if (typeof initialType === 'string') {
      typeFilter.value = initialType
    }
    loading.value = true
    try {
      const list = await catalogApi.list()
      allTemplates.value = list.templates || []
      canRefresh.value = list.can_refresh
    } catch {}
    loading.value = false
  })
</script>
