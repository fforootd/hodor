<template>
  <Toaster position="top-right" :expand="true" rich-colors />
  <AppBootstrapScreen
    v-if="bootstrapState !== 'ready'"
    app-name="console"
    :state="bootstrapState"
    :error="bootstrapError"
    :retry-delay-ms="bootstrapRetryDelayMs"
    @retry="retryBootstrap"
  />

  <SidebarProvider v-else :default-open="true">
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" as-child>
              <router-link to="/">
                <div
                  class="flex aspect-square size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground"
                >
                  <Shield class="size-4" />
                </div>
                <div class="grid flex-1 text-left text-sm leading-tight">
                  <span class="truncate font-semibold">Zitadel</span>
                  <span class="truncate text-xs text-muted-foreground">Console</span>
                </div>
              </router-link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <!-- Instance Switcher (root instances only) -->
        <div v-if="isRootInstance" class="px-2 group-data-[collapsible=icon]:hidden">
          <Popover v-model:open="showInstanceDropdown">
            <PopoverTrigger as-child>
              <button class="w-full flex items-center justify-between rounded-md border border-sidebar-border bg-sidebar px-3 py-1.5 text-sm hover:bg-sidebar-accent transition-colors">
                <span class="flex items-center gap-2 truncate">
                  <Server class="size-3.5 shrink-0 text-muted-foreground" />
                  <span class="truncate">{{ instanceDisplayLabel }}</span>
                </span>
                <ChevronsUpDown class="size-3 shrink-0 opacity-50" />
              </button>
            </PopoverTrigger>
            <PopoverContent class="w-64 p-0" align="start" side="bottom">
              <Command>
                <CommandInput placeholder="Find instance..." />
                <CommandList>
                  <CommandEmpty>No instance found.</CommandEmpty>
                  <CommandGroup>
                    <CommandItem value="no-instance" @select="deselectInstance">
                      <Globe class="mr-2 size-4" />
                      No instance selected
                    </CommandItem>
                    <CommandItem
                      v-for="inst in instanceList"
                      :key="inst.instance_id"
                      :value="inst.primary_domain || inst.instance_id"
                      @select="selectInstance(inst)"
                    >
                      <Server class="mr-2 size-4" />
                      {{ inst.primary_domain || inst.instance_id }}
                    </CommandItem>
                  </CommandGroup>
                </CommandList>
              </Command>
            </PopoverContent>
          </Popover>
        </div>

        <!-- Org Switcher -->
        <div class="px-2 pb-1 group-data-[collapsible=icon]:hidden">
          <Popover v-model:open="showOrgDropdown">
            <PopoverTrigger as-child>
              <button class="w-full flex items-center justify-between rounded-md border border-sidebar-border bg-sidebar px-3 py-1.5 text-sm hover:bg-sidebar-accent transition-colors">
                <span class="flex items-center gap-2 truncate">
                  <Building2 class="size-3.5 shrink-0 text-muted-foreground" />
                  <span class="truncate">{{ selectedOrg?.display_name || 'All Orgs' }}</span>
                </span>
                <ChevronsUpDown class="size-3 shrink-0 opacity-50" />
              </button>
            </PopoverTrigger>
            <PopoverContent class="w-56 p-0" align="start" side="bottom">
              <Command>
                <CommandInput placeholder="Search organizations..." />
                <CommandList>
                  <CommandEmpty>No organization found.</CommandEmpty>
                  <CommandGroup>
                    <CommandItem value="all-orgs" @select="selectOrg(null)">
                      <Globe class="mr-2 size-4" />
                      All Organizations
                    </CommandItem>
                    <CommandItem
                      v-for="org in orgs"
                      :key="org.id"
                      :value="org.display_name"
                      @select="selectOrg(org)"
                    >
                      <Building2 class="mr-2 size-4" />
                      {{ org.display_name }}
                    </CommandItem>
                  </CommandGroup>
                </CommandList>
              </Command>
            </PopoverContent>
          </Popover>
        </div>
      </SidebarHeader>

      <SidebarContent>
        <!-- ─── Drilled-in view: back button + sub-items ─── -->
        <template v-if="drilledCategory">
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton @click="drilledCategoryKey = null" :tooltip="'Back'">
                    <ArrowLeft class="size-4" />
                    <span>{{ drilledCategory.label }}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
          <SidebarSeparator />
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem v-for="item in drilledCategory.items" :key="item.type">
                  <SidebarMenuButton as-child :data-active="isNavActive(item)" :tooltip="item.label">
                    <router-link :to="resolveRoute(item.route)">
                      <component :is="getIcon(item.type)" class="size-4" />
                      <span>{{ item.label }}</span>
                    </router-link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </template>

        <!-- ─── Top-level navigation ─── -->
        <template v-else>
          <!-- Dashboard + Instances -->
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton as-child :data-active="$route.name === 'dashboard' || $route.name === 'i-dashboard'" :tooltip="'Dashboard'">
                    <router-link :to="currentInstanceId ? `/instances/${currentInstanceId}` : '/'">
                      <LayoutDashboard class="size-4" />
                      <span>{{ isRootInstance && !currentInstanceId ? 'Getting Started' : 'Dashboard' }}</span>
                    </router-link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem v-if="isRootInstance">
                  <SidebarMenuButton as-child :data-active="$route.name === 'instances' || $route.name === 'instance-create' || $route.path.startsWith('/instances/')" :tooltip="'Instances'">
                    <router-link to="/instances">
                      <Server class="size-4" />
                      <span>Instances</span>
                    </router-link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>

          <!-- Categorized navigation -->
          <template v-if="showInstanceSection">
            <template v-for="category in categorizedNav" :key="category.key">
              <!-- Flat category: label + items always visible -->
              <SidebarGroup v-if="!category.drillable">
                <SidebarGroupLabel class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                  {{ category.label }}
                </SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>
                    <SidebarMenuItem v-for="item in category.items" :key="item.type">
                      <SidebarMenuButton as-child :data-active="isNavActive(item)" :tooltip="item.label">
                        <router-link :to="resolveRoute(item.route)">
                          <component :is="getIcon(item.type)" class="size-4" />
                          <span>{{ item.label }}</span>
                        </router-link>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>

              <!-- Drillable category: single row that navigates deeper -->
              <SidebarGroup v-else>
                <SidebarGroupContent>
                  <SidebarMenu>
                    <SidebarMenuItem>
                      <SidebarMenuButton @click="drilledCategoryKey = category.key" :data-active="isCategoryActive(category)" :tooltip="category.label">
                        <component :is="category.icon" class="size-4" />
                        <span>{{ category.label }}</span>
                        <ChevronRight class="ml-auto size-4 opacity-50" />
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            </template>
          </template>

        </template>
      </SidebarContent>

      <SidebarFooter>
        <SidebarMenu>
          <!-- User profile -->
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <SidebarMenuButton size="lg">
                  <Avatar class="size-8 rounded-lg">
                    <AvatarFallback class="rounded-lg">ZA</AvatarFallback>
                  </Avatar>
                  <div class="grid flex-1 text-left text-sm leading-tight">
                    <span class="truncate font-semibold">Admin</span>
                    <span class="truncate text-xs text-muted-foreground">admin@localhost</span>
                  </div>
                  <ChevronsUpDown class="ml-auto size-4" />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
              <DropdownMenuContent class="w-56" side="top" align="start">
                <DropdownMenuItem as-child>
                  <a :href="basePath + '/account'">
                    <User class="mr-2 size-4" />
                    <span>My Account</span>
                  </a>
                </DropdownMenuItem>
                <DropdownMenuItem as-child>
                  <a :href="basePath + '/logout'">
                    <LogOut class="mr-2 size-4" />
                    <span>Sign out</span>
                  </a>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>

    <SidebarInset>
      <!-- Header -->
      <header class="flex h-14 shrink-0 items-center gap-2 border-b px-4">
        <SidebarTrigger class="-ml-1" />
        <Separator orientation="vertical" class="mr-2 h-4" />

        <!-- Breadcrumb -->
        <Breadcrumb>
          <BreadcrumbList>
            <template v-for="(crumb, i) in breadcrumbs" :key="i">
              <BreadcrumbSeparator v-if="i > 0" />
              <BreadcrumbItem>
                <BreadcrumbLink v-if="i < breadcrumbs.length - 1" as-child>
                  <router-link :to="crumb.path">{{ crumb.label }}</router-link>
                </BreadcrumbLink>
                <BreadcrumbPage v-else>{{ crumb.label }}</BreadcrumbPage>
              </BreadcrumbItem>
            </template>
          </BreadcrumbList>
        </Breadcrumb>

        <div class="ml-auto flex items-center gap-2">
          <!-- Command Palette Trigger -->
          <Button
            variant="outline"
            size="sm"
            class="gap-1.5 text-xs text-muted-foreground"
            @click="showCommandPalette = true"
          >
            <Search class="size-3.5" />
            <span class="hidden sm:inline">Search...</span>
            <kbd
              class="pointer-events-none hidden h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium opacity-100 sm:flex"
              >⌘K</kbd
            >
          </Button>
        </div>
      </header>

      <!-- Command Palette Dialog -->
      <CommandDialog v-model:open="showCommandPalette">
        <CommandInput placeholder="Search or jump to…" @input="onCommandSearch" />
        <CommandList>
          <CommandEmpty>No results found.</CommandEmpty>

          <!-- API search results -->
          <CommandGroup v-if="searchResults.length" heading="Search Results">
            <CommandItem
              v-for="r in searchResults"
              :key="r.resource_type + r.id"
              :value="`result-${r.resource_type}-${r.id}-${r.title}-${r.subtitle}`"
              @select="goToResult(r)"
            >
              <component :is="getResultIcon(r.resource_type)" class="mr-2 size-4 shrink-0" />
              <div class="flex flex-col">
                <span class="text-sm font-medium">{{ r.title }}</span>
                <span class="text-xs text-muted-foreground">{{ r.subtitle }}</span>
              </div>
            </CommandItem>
          </CommandGroup>

          <!-- Navigation shortcuts -->
          <CommandGroup heading="Navigation">
            <CommandItem value="nav-dashboard Dashboard" @select="navigateTo('/')">
              <LayoutDashboard class="mr-2 size-4 shrink-0" />
              <span>Dashboard</span>
            </CommandItem>
            <CommandItem
              v-for="item in navItems"
              :key="item.type"
              :value="`nav-${item.type} ${item.label}`"
              @select="navigateTo(item.route)"
            >
              <component :is="getIcon(item.type)" class="mr-2 size-4 shrink-0" />
              <span>{{ item.label }}</span>
            </CommandItem>
          </CommandGroup>

          <CommandGroup heading="Quick Actions">
            <CommandItem value="action-create-user Create User" @select="navigateTo('/users/new')">
              <Users class="mr-2 size-4 shrink-0" />
              <span>Create User</span>
            </CommandItem>
            <CommandItem value="action-create-app Create Application" @select="navigateTo('/applications/new')">
              <AppWindow class="mr-2 size-4 shrink-0" />
              <span>Create Application</span>
            </CommandItem>
            <CommandItem value="action-schemas View Schemas" @select="navigateTo('/schemas')">
              <FileJson class="mr-2 size-4 shrink-0" />
              <span>View Schemas</span>
            </CommandItem>
            <CommandItem value="action-marketplace Browse Marketplace" @select="navigateTo('/marketplace')">
              <Package class="mr-2 size-4 shrink-0" />
              <span>Browse Marketplace</span>
            </CommandItem>
            <CommandItem value="action-events View Audit Log" @select="navigateTo('/events')">
              <Activity class="mr-2 size-4 shrink-0" />
              <span>View Audit Log</span>
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </CommandDialog>

      <!-- Main Content -->
      <div class="flex-1 overflow-auto p-4 md:p-6">
        <router-view :key="`${$route.fullPath}__org_${selectedOrgId || 'all'}__inst_${currentInstanceId || 'none'}`" />
      </div>
    </SidebarInset>
  </SidebarProvider>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
  import { createReadyzWaiter, useAppBootstrap } from '@/bootstrap/app-bootstrap'
  import { useOrgContext } from '@/console/composables/useOrgContext'
  import { useInstanceContext } from '@/console/composables/useInstanceContext'
  import { useInstanceRoutes } from '@/console/composables/useInstanceRoutes'
  import { getUserSchemaLabel, normalizeUserSchemaType } from '@/console/utils/user-routes'
  import { useRoute, useRouter } from 'vue-router'
  import { api } from '@/api/client'
  import { searchApi, instanceApi, orgApi, type SearchResult, type Instance } from '@/api/resources'
  import AppBootstrapScreen from '@/components/AppBootstrapScreen.vue'
  import { Toaster } from '@/components/ui/sonner'

  // shadcn components
  import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupContent,
    SidebarGroupLabel,
    SidebarHeader,
    SidebarInset,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarProvider,
    SidebarRail,
    SidebarSeparator,
    SidebarTrigger,
  } from '@/components/ui/sidebar'
  import { Button } from '@/components/ui/button'
  import { Separator } from '@/components/ui/separator'
  import { Avatar, AvatarFallback } from '@/components/ui/avatar'
  import {
    Breadcrumb,
    BreadcrumbItem,
    BreadcrumbLink,
    BreadcrumbList,
    BreadcrumbPage,
    BreadcrumbSeparator,
  } from '@/components/ui/breadcrumb'
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
  } from '@/components/ui/dropdown-menu'
  import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
  import {
    Command,
    CommandDialog,
    CommandEmpty,
    CommandGroup,
    CommandInput,
    CommandItem,
    CommandList,
  } from '@/components/ui/command'

  // Lucide icons
  import {
    Shield,
    LayoutDashboard,
    Users,
    KeyRound,
    Globe,
    FileJson,
    Workflow,
    Lock,
    Clock,
    BarChart3,
    Search,
    ChevronsUpDown,
    Building2,
    User,
    LogOut,
    Database,
    Zap,
    Bot,
    AppWindow,
    Activity,
    Calendar,
    ShieldCheck,
    Package,
    ChevronRight,
    Settings,
    UsersRound,
    FolderKanban,
    Server,
    Plus,
    Link,
    BellRing,
    CreditCard,
    Wrench,
    ArrowLeft,
  } from 'lucide-vue-next'

  const route = useRoute()
  const router = useRouter()
  const { resolveRoute } = useInstanceRoutes()

  const basePath = (window as any).__ZITADEL_BASE_PATH__ || ''

  // ─── Org Switcher ───
  interface OrgEntry {
    id: string
    display_name: string
    name: string
  }
  const orgs = ref<OrgEntry[]>([])
  const showOrgDropdown = ref(false)
  const { currentOrgId, setOrg } = useOrgContext()
  const selectedOrgId = currentOrgId
  const selectedOrg = computed(() => orgs.value.find((o) => o.id === selectedOrgId.value) || null)

  function selectOrg(org: OrgEntry | null) {
    showOrgDropdown.value = false
    setOrg(org?.id ?? null)
  }

  // ─── Instance Switcher ───
  const { currentInstanceId, currentInstanceDomain, setInstance, clearInstance } = useInstanceContext()
  const showInstanceDropdown = ref(false)
  const instanceDisplayLabel = computed(() => currentInstanceDomain.value || 'Select instance...')
  const instanceList = ref<Instance[]>([])

  function deselectInstance() {
    showInstanceDropdown.value = false
    // Navigate to root — the route watcher handles clearInstance()
    router.push('/')
  }

  function selectInstance(inst: Instance | null) {
    if (inst) {
      setInstance(inst.instance_id, inst.primary_domain || inst.instance_id)
      showInstanceDropdown.value = false
      // Navigate to the instance, preserving current product section if possible
      const currentPath = route.path
      const instanceMatch = currentPath.match(/^\/instances\/[^/]+(\/.*)?$/)
      const productPath = instanceMatch?.[1] || ''
      router.push(`/instances/${inst.instance_id}${productPath}`)
    } else {
      clearInstance()
      showInstanceDropdown.value = false
      router.push('/')
    }
  }

  async function loadInstances() {
    try {
      // Instance list is a root-level endpoint (/v1/instances) — not rewritten.
      const res = await instanceApi.list({ limit: 100 })
      instanceList.value = res.items ?? []
    } catch {
      instanceList.value = []
    }
  }

  // ─── Drill-in navigation state ───
  const drilledCategoryKey = ref<string | null>(null)

  // ─── Command Palette ───
  const showCommandPalette = ref(false)
  const searchResults = ref<SearchResult[]>([])
  const commandQuery = ref('')
  let debounceTimer: ReturnType<typeof setTimeout>

  function onCommandSearch(e: Event) {
    const query = (e.target as HTMLInputElement).value
    commandQuery.value = query
    clearTimeout(debounceTimer)
    if (!query.trim()) {
      searchResults.value = []
      return
    }
    debounceTimer = setTimeout(async () => {
      try {
        const resp = await searchApi.search(query.trim())
        searchResults.value = (resp.results || []) as SearchResult[]
      } catch {
        searchResults.value = []
      }
    }, 200)
  }

  function goToResult(r: SearchResult) {
    showCommandPalette.value = false
    searchResults.value = []
    commandQuery.value = ''
    const routeMap: Record<string, (id: string) => string> = {
      user: (id) => resolveRoute(`/users/${id}`),
      identity: (id) => resolveRoute(`/users/${id}`),
      org: (id) => resolveRoute(`/orgs/${id}`),
      schema: (id) => resolveRoute(`/schemas/${id}`),
      event: () => resolveRoute('/events'),
      provider: (id) => resolveRoute(`/providers/${id}`),
      session: () => resolveRoute('/sessions'),
    }
    const resolver = routeMap[r.resource_type] || (() => '/')
    router.push(resolver(r.id))
  }

  function navigateTo(path: string) {
    showCommandPalette.value = false
    searchResults.value = []
    commandQuery.value = ''
    router.push(resolveRoute(path))
  }

  function getResultIcon(resourceType: string) {
    if (resourceType === 'user' || resourceType === 'identity') return Users
    if (resourceType === 'org') return Building2
    if (resourceType === 'schema') return FileJson
    if (resourceType === 'event') return Activity
    if (resourceType === 'provider') return Globe
    return Database
  }

  // ⌘K shortcut
  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault()
      showCommandPalette.value = true
    }
  }
  onMounted(() => document.addEventListener('keydown', handleKeydown))
  onUnmounted(() => document.removeEventListener('keydown', handleKeydown))

  // ─── Catalog-driven nav ───
  interface NavItem {
    type: string
    label: string
    route: string
    sortOrder: number
    storage: string
    countable: boolean
    navGroup?: string
    count?: number
    aggregates?: string[]
  }

  interface NavGroupDef {
    label: string
    sort_order: number
    display?: string
    icon?: string
  }

  interface ConsoleBootstrapResponse {
    meta: Record<string, any>
    counts: Record<string, number>
    orgs?: {
      items?: Array<Record<string, any>>
    }
    features?: Record<string, boolean>
    instance?: {
      id: string
      kind: string
      is_root: boolean
    }
    capabilities?: {
      instance_management: boolean
      operator_admin: boolean
      billing: boolean
    }
  }

  // Reactive bootstrap context.
  const isRootInstance = ref(false)
  const isOperatorAdmin = ref(false)

  const navItems = ref<NavItem[]>([])
  const navGroupDefs = ref<Record<string, NavGroupDef>>({})

  // ─── Vercel-style collapsible category definitions ───
  interface NavCategory {
    key: string
    label: string
    icon: any
    drillable: boolean   // true = shows as single row that drills deeper on click
    catalogGroups?: string[]
    explicitTypes?: string[]
  }

  const navCategoryDefs: NavCategory[] = [
    {
      key: 'identity',
      label: 'Identity',
      icon: Users,
      drillable: false,
      explicitTypes: ['users', 'human_user', 'service_user', 'ai_agent', 'org', 'group', 'project'],
    },
    {
      key: 'applications',
      label: 'Applications',
      icon: AppWindow,
      drillable: false,
      explicitTypes: ['applications', 'app', 'provider', 'login_flow'],
    },
    {
      key: 'observability',
      label: 'Observability',
      icon: Activity,
      drillable: true,
      catalogGroups: ['observability'],
    },
    {
      key: 'authorization',
      label: 'Authorization',
      icon: ShieldCheck,
      drillable: true,
      catalogGroups: ['authorization'],
    },
    {
      key: 'system',
      label: 'System',
      icon: Settings,
      drillable: true,
      catalogGroups: ['system'],
      explicitTypes: ['schema', 'marketplace', 'session', 'event', 'job', 'action', 'notification', 'endpoint'],
    },
  ]

  const {
    state: bootstrapState,
    error: bootstrapError,
    retryDelayMs: bootstrapRetryDelayMs,
    run: runBootstrap,
    retry: retryBootstrap,
    dispose: disposeBootstrap,
  } = useAppBootstrap(
    async () => {
      // Bootstrap always queries the root instance (the /v1/console/bootstrap
      // path is excluded from instance rewriting in the fetch layer).
      const bootstrap = await api.get<ConsoleBootstrapResponse>('/v1/console/bootstrap')

      isRootInstance.value = bootstrap.instance?.is_root ?? false
      isOperatorAdmin.value = bootstrap.capabilities?.operator_admin ?? false

      applyOrgs(bootstrap.orgs?.items || [])
      hydrateNav(bootstrap.meta || {})

      // Load instances list for the instance switcher (root only).
      if (isRootInstance.value) {
        loadInstances()
      }
    },
    {
      waitForReady: createReadyzWaiter(),
    },
  )

  // Refresh instance list when navigating back to the instances page
  // (e.g., after creating a new instance).
  watch(
    () => route.name,
    (name) => {
      if (isRootInstance.value && (name === 'instances' || name === 'dashboard')) {
        loadInstances()
      }
    },
  )

  // Whether to show the INSTANCE section in the sidebar.
  // Always show product nav — the root instance has its own users/orgs/apps.
  // When inside a child instance, the same nav links point to instance-scoped URLs.
  const showInstanceSection = computed(() => true)

  // Categorized navigation: groups nav items into categories
  interface ResolvedCategory {
    key: string
    label: string
    icon: any
    drillable: boolean
    items: NavItem[]
  }

  const categorizedNav = computed<ResolvedCategory[]>(() => {
    const assigned = new Set<string>()
    const result: ResolvedCategory[] = []

    for (const category of navCategoryDefs) {
      const items: NavItem[] = []

      if (category.explicitTypes) {
        for (const item of navItems.value) {
          if (category.explicitTypes.includes(item.type) && !assigned.has(item.type)) {
            items.push(item)
            assigned.add(item.type)
          }
        }
      }

      if (category.catalogGroups) {
        for (const item of navItems.value) {
          if (item.navGroup && category.catalogGroups.includes(item.navGroup) && !assigned.has(item.type)) {
            items.push(item)
            assigned.add(item.type)
          }
        }
      }

      items.sort((a, b) => a.sortOrder - b.sortOrder)

      if (items.length > 0) {
        result.push({
          key: category.key,
          label: category.label,
          icon: category.icon,
          drillable: category.drillable,
          items,
        })
      }
    }

    // Catch-all for unassigned items
    const unassigned = navItems.value.filter(i => !assigned.has(i.type))
    if (unassigned.length > 0) {
      result.push({
        key: 'other',
        label: 'Other',
        icon: Database,
        drillable: true,
        items: unassigned.sort((a, b) => a.sortOrder - b.sortOrder),
      })
    }

    return result
  })

  // The currently drilled-in category (resolved from key)
  const drilledCategory = computed<ResolvedCategory | null>(() => {
    if (!drilledCategoryKey.value) return null
    return categorizedNav.value.find(c => c.key === drilledCategoryKey.value) || null
  })

  function isCategoryActive(category: ResolvedCategory): boolean {
    return category.items.some(item => isNavActive(item))
  }

  // Auto-drill into the matching category when navigating to a sub-item
  watch(
    () => route.path,
    () => {
      // If already drilled in and the route matches, keep it
      if (drilledCategory.value && isCategoryActive(drilledCategory.value)) return
      // Check if we need to auto-drill into a category
      for (const cat of categorizedNav.value) {
        if (cat.drillable && isCategoryActive(cat)) {
          drilledCategoryKey.value = cat.key
          return
        }
      }
      // No match — go back to top level
      drilledCategoryKey.value = null
    },
  )

  function isNavActive(item: NavItem): boolean {
    const r = route
    // Normalize: strip i- prefix for comparison
    const name = (r.name as string || '').replace(/^i-/, '')
    if (item.route === '/users') {
      return name === 'users' || name === 'user-create' || name === 'user-detail'
    }
    if (item.route === '/applications') {
      return name === 'applications' || name === 'application-create' || name === 'application-detail'
    }
    if (item.route === '/instances') {
      return name === 'instances' || name === 'instance-create' || name === 'instance-detail'
    }
    if (item.storage === 'entities') return r.params.schemaType === item.type
    return name === item.type || r.path.includes(`/${item.route?.replace(/^\//, '')}`)
  }

  const iconMap: Record<string, any> = {
    users: Users,
    applications: AppWindow,
    human_user: Users,
    service_user: KeyRound,
    ai_agent: Bot,
    app: AppWindow,
    org: Building2,
    group: UsersRound,
    project: FolderKanban,
    action: Zap,
    login_flow: Lock,
    provider: Globe,
    session: Clock,
    event: Activity,
    schema: FileJson,
    job: Calendar,
    analytics: BarChart3,
    overview: BarChart3,
    explore: Search,
    trace: Workflow,
    authz_overview: ShieldCheck,
    authz_permissions: ShieldCheck,
    authz_relationships: Workflow,
    authz_model: Workflow,
    authz_modules: Package,
    marketplace: Package,
    endpoint: Link,
    notification: BellRing,
    instances: Server,
  }

  function getIcon(type: string) {
    return iconMap[type] || Database
  }

  function hydrateNav(meta: Record<string, any>) {
    const catalog = meta['x-catalog'] || {}
    const groups = meta['x-groups'] || {}

    navGroupDefs.value = groups

    const items: NavItem[] = []
    for (const [typeName, entry] of Object.entries(catalog) as [string, any][]) {
      if (entry.nav === 'hidden') continue

      let itemRoute: string
      if (entry.route) {
        itemRoute = entry.route
      } else if (entry.storage === 'entities') {
        itemRoute = `/s/${typeName}`
      } else {
        itemRoute = `/${entry.path}`
      }

      items.push({
        type: typeName,
        label: entry.alias || typeName,
        sortOrder: entry.sort_order ?? 99,
        storage: entry.storage || 'entities',
        route: itemRoute,
        countable: !!entry.countable,
        navGroup: entry.nav_group,
        aggregates: entry.aggregates,
      })
    }

    navItems.value = items.sort((a, b) => a.sortOrder - b.sortOrder)
  }

  function applyOrgs(items: Array<Record<string, any>>) {
    orgs.value = items.map((org) => ({
      id: String(org.id || ''),
      display_name: org.name || org.display_name || org.identifier || '',
      name: org.name || '',
    }))

    if (selectedOrgId.value && !orgs.value.find((org) => org.id === selectedOrgId.value)) {
      selectOrg(null)
    }
  }

  onMounted(async () => {
    await runBootstrap()
  })

  onUnmounted(() => {
    disposeBootstrap()
  })

  // Sync instance display state and sidebar counts when entering/leaving an instance.
  watch(
    () => route.params.instanceId as string | undefined,
    async (newId) => {
      if (newId) {
        setInstance(newId, newId)
      } else {
        clearInstance()
      }
      // Refresh orgs for the current scope.
      // The fetch layer rewrites to /v1/instances/:id/... when inside an instance.
      try {
        const orgResp = await orgApi.list()
        applyOrgs(orgResp)
      } catch {
        applyOrgs([])
      }
    },
  )

  const currentUserCreateType = computed(() => normalizeUserSchemaType(route.query.type))
  const pageTitle = computed(() => {
    // Normalize: strip i- prefix for instance-scoped route names
    const name = (route.name as string || '').replace(/^i-/, '')
    if (name === 'users') return 'Users'
    if (name === 'user-create')
      return `New ${getUserSchemaLabel(currentUserCreateType.value)}`
    if (name === 'applications') return 'Applications'
    if (route.params.schemaType) {
      const st = route.params.schemaType as string
      const entry = navItems.value.find((e) => e.type === st)
      return entry?.label || st.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()) + 's'
    }
    const titles: Record<string, string> = {
      dashboard: 'Dashboard',
      instances: 'Instances',
      'instance-create': 'New Instance',
      'instance-detail': 'Instance',
      team: 'Team',
      billing: 'Billing',
      'user-detail': 'User Detail',
      'identity-create': 'New User',
      orgs: 'Organizations',
      'org-create': 'New Organization',
      'org-detail': 'Organization',
      groups: 'Groups',
      'group-create': 'New Group',
      'group-detail': 'Group',
      projects: 'Projects',
      'project-create': 'New Project',
      'project-detail': 'Project',
      schemas: 'Schemas',
      'schema-detail': 'Schema Editor',
      marketplace: 'Marketplace',
      providers: 'Providers',
      'api-protocols': 'API & Protocols',
      sessions: 'Sessions',
      events: 'Events',
      jobs: 'Jobs',
      'obs-overview': 'Overview',
      'obs-explore': 'Explore',
      traces: 'Traces',
      authorization: 'System Authorization',
      notifications: 'Notifications',
    }
    return titles[name] || 'Console'
  })

  // Route-aware breadcrumbs
  // Maps normalized route name (without i- prefix) → parent info
  const parentRoutes: Record<string, { label: string; path: string }> = {
    'user-detail': { label: 'Users', path: '/users' },
    'user-create': { label: 'Users', path: '/users' },
    'application-detail': { label: 'Applications', path: '/applications' },
    'application-create': { label: 'Applications', path: '/applications' },
    'org-detail': { label: 'Organizations', path: '/orgs' },
    'org-create': { label: 'Organizations', path: '/orgs' },
    'group-detail': { label: 'Groups', path: '/groups' },
    'group-create': { label: 'Groups', path: '/groups' },
    'project-detail': { label: 'Projects', path: '/projects' },
    'project-create': { label: 'Projects', path: '/projects' },
    'instance-detail': { label: 'Instances', path: '/instances' },
    'instance-create': { label: 'Instances', path: '/instances' },
    'schema-detail': { label: 'Schemas', path: '/schemas' },
    'provider-detail': { label: 'Providers', path: '/providers' },
    'provider-create': { label: 'Providers', path: '/providers' },
    'login-flow-detail': { label: 'Login Flows', path: '/login-flows' },
    notifications: { label: 'Notifications', path: '/notifications' },
    'schema-detail-item': { label: 'Schema Items', path: '/' },
  }

  const breadcrumbs = computed(() => {
    const crumbs: { label: string; path: string }[] = []
    const instanceId = route.params.instanceId as string | undefined

    // Instance context is now shown in the sidebar selector — breadcrumb
    // only shows navigation within the current instance scope.

    // Normalize route name (strip i- prefix for instance-scoped routes)
    const name = (route.name as string || '').replace(/^i-/, '')
    const parent = parentRoutes[name]
    if (parent) {
      crumbs.push({
        label: parent.label,
        path: instanceId ? `/instances/${instanceId}${parent.path}` : parent.path,
      })
    }
    crumbs.push({ label: pageTitle.value, path: route.path })
    return crumbs
  })
</script>
