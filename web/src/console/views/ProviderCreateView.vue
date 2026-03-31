<template>
  <TooltipProvider>
    <div class="space-y-6">
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="icon" as-child>
          <router-link to="/providers"><ArrowLeft class="size-4" /></router-link>
        </Button>
        <div class="space-y-1">
          <div class="flex items-center gap-2">
            <h1 class="text-2xl font-semibold tracking-tight">Create Provider</h1>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button variant="ghost" size="icon" class="size-8 text-muted-foreground">
                  <Info class="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent class="max-w-72">
                Templates preload protocol defaults and common claim mappings. You can reuse the
                same template for multiple provider instances.
              </TooltipContent>
            </Tooltip>
          </div>
          <p class="text-sm text-muted-foreground">
            {{ loading ? 'Loading templates…' : `${templates.length} template${templates.length === 1 ? '' : 's'} available.` }}
          </p>
        </div>
      </div>

      <div class="space-y-3">
        <div class="flex items-center justify-between">
          <h2 class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
            Templates
          </h2>
        </div>

        <div v-if="loading" class="flex h-32 items-center justify-center text-muted-foreground">
          Loading templates…
        </div>

        <div
          v-else-if="templates.length === 0"
          class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
        >
          No provider templates available.
        </div>

        <div v-else class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          <Card
            v-for="template in templates"
            :key="template.id"
            class="cursor-pointer transition-all hover:border-primary hover:bg-muted/10 hover:shadow-sm"
            @click="selectTemplate(template.id)"
          >
            <CardHeader class="pb-3">
              <div class="flex items-start justify-between gap-3">
                <div class="flex min-w-0 items-start gap-3">
                  <div class="text-xl">{{ providerIcon(template.id) }}</div>
                  <div class="min-w-0 flex-1 space-y-1">
                    <CardTitle class="text-base">{{ template.name }}</CardTitle>
                    <p class="text-xs uppercase tracking-wide text-muted-foreground">
                      {{ template.protocol || 'provider' }}
                    </p>
                  </div>
                </div>
                <Tooltip v-if="template.description">
                  <TooltipTrigger as-child>
                    <Button variant="ghost" size="icon" class="size-8 shrink-0 text-muted-foreground">
                      <Info class="size-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent class="max-w-72">
                    {{ template.description }}
                  </TooltipContent>
                </Tooltip>
              </div>
            </CardHeader>
            <CardContent class="space-y-3">
              <p v-if="template.description" class="line-clamp-2 text-sm text-muted-foreground">
                {{ template.description }}
              </p>
              <div class="text-sm font-medium text-foreground">Select template</div>
            </CardContent>
          </Card>
        </div>
      </div>

      <CatalogInstallDialog
        :open="showCreateDialog"
        :template-id="selectedTemplateId"
        @update:open="handleDialogOpen"
        @installed="onInstalled"
      />
    </div>
  </TooltipProvider>
</template>

<script setup lang="ts">
  import { computed, onMounted, ref, watch } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import CatalogInstallDialog from '@/console/components/catalog/CatalogInstallDialog.vue'
  import type { ProviderTemplateSummary } from '@/console/utils/provider-utils'
  import { providerIcon } from '@/console/utils/provider-utils'
  import { api } from '@/api/client'
  import { Button } from '@/components/ui/button'
  import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
  import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
  import { ArrowLeft, Info } from 'lucide-vue-next'

  const route = useRoute()
  const router = useRouter()

  const templates = ref<ProviderTemplateSummary[]>([])
  const loading = ref(false)
  const showCreateDialog = ref(false)
  const selectedTemplateId = ref('')

  const routeTemplateId = computed(() =>
    typeof route.query.template === 'string' ? route.query.template : '',
  )

  async function loadTemplates() {
    loading.value = true
    try {
      const data = await api.get<any>('/v1/providers/templates')
      templates.value = data.templates || []
    } catch {
      templates.value = []
    } finally {
      loading.value = false
    }
  }

  function selectTemplate(templateId: string) {
    selectedTemplateId.value = templateId
    showCreateDialog.value = true
  }

  function syncTemplateFromRoute() {
    if (!routeTemplateId.value || templates.value.length === 0) return
    const template = templates.value.find((item) => item.id === routeTemplateId.value)
    if (!template) return
    selectedTemplateId.value = template.id
    showCreateDialog.value = true
  }

  async function clearTemplateRouteQuery() {
    if (!routeTemplateId.value && typeof route.query.source !== 'string') return
    const query = { ...route.query }
    delete query.template
    delete query.source
    await router.replace({ path: route.path, query })
  }

  async function onInstalled(result: { id: string }) {
    await clearTemplateRouteQuery()
    await router.push(`/providers/${result.id}`)
  }

  function handleDialogOpen(next: boolean) {
    showCreateDialog.value = next
    if (!next) {
      selectedTemplateId.value = ''
      void clearTemplateRouteQuery()
    }
  }

  onMounted(async () => {
    await loadTemplates()
    syncTemplateFromRoute()
  })

  watch(
    () => [routeTemplateId.value, templates.value.length] as const,
    () => {
      syncTemplateFromRoute()
    },
  )
</script>
