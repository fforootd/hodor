<template>
  <Toaster position="top-right" :expand="true" rich-colors />
  <SidebarProvider>
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" as-child>
              <router-link to="/">
                <div class="flex aspect-square size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
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
      </SidebarHeader>

      <SidebarContent>
        <!-- Dashboard (always first) -->
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton as-child :data-active="$route.name === 'dashboard'">
                  <router-link to="/">
                    <LayoutDashboard class="size-4" />
                    <span>Dashboard</span>
                  </router-link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton as-child :data-active="$route.name === 'instances'">
                  <router-link to="/instances">
                    <Server class="size-4" />
                    <span>All Instances</span>
                  </router-link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <!-- Ungrouped primary resources (no nav_group) -->
        <SidebarGroup v-if="ungroupedItems.length">
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem v-for="item in ungroupedItems" :key="item.type">
                <SidebarMenuButton as-child :data-active="isNavActive(item)">
                  <router-link :to="item.route">
                    <component :is="getIcon(item.type)" class="size-4" />
                    <span>{{ item.label }}</span>
                    <span
                      v-if="item.count !== undefined && item.count > 0"
                      class="ml-auto text-xs text-muted-foreground tabular-nums"
                    >{{ item.count }}</span>
                  </router-link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <!-- Flat groups (items with separator, no group label) -->
        <template v-for="group in flatGroups" :key="group.key">
          <SidebarSeparator />
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem v-for="item in group.items" :key="item.type">
                  <SidebarMenuButton as-child :data-active="isNavActive(item)">
                    <router-link :to="item.route">
                      <component :is="getIcon(item.type)" class="size-4" />
                      <span>{{ item.label }}</span>
                      <span
                        v-if="item.count !== undefined && item.count > 0"
                        class="ml-auto text-xs text-muted-foreground tabular-nums"
                      >{{ item.count }}</span>
                    </router-link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </template>

        <!-- Collapsible groups (Observability, System) -->
        <template v-for="group in collapsibleGroups" :key="group.key">
          <SidebarSeparator />
          <SidebarGroup>
            <SidebarMenu>
              <Collapsible :default-open="isGroupActive(group)" class="group/collapsible">
                <SidebarMenuItem>
                  <CollapsibleTrigger as-child>
                    <SidebarMenuButton>
                      <component :is="getGroupIcon(group.key)" class="size-4" />
                      <span>{{ group.label }}</span>
                      <ChevronRight class="ml-auto size-4 transition-transform group-data-[state=open]/collapsible:rotate-90" />
                    </SidebarMenuButton>
                  </CollapsibleTrigger>
                  <CollapsibleContent>
                    <SidebarMenuSub>
                      <SidebarMenuSubItem v-for="item in group.items" :key="item.type">
                        <SidebarMenuSubButton as-child :data-active="isNavActive(item)">
                          <router-link :to="item.route">
                            <span>{{ item.label }}</span>
                          </router-link>
                        </SidebarMenuSubButton>
                      </SidebarMenuSubItem>
                    </SidebarMenuSub>
                  </CollapsibleContent>
                </SidebarMenuItem>
              </Collapsible>
            </SidebarMenu>
          </SidebarGroup>
        </template>
      </SidebarContent>

      <SidebarFooter>
        <SidebarMenu>
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
            <BreadcrumbItem>
              <BreadcrumbPage>{{ pageTitle }}</BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>

        <div class="ml-auto flex items-center gap-2">
          <!-- Instance Switcher -->
          <Popover v-model:open="showInstanceDropdown">
            <PopoverTrigger as-child>
              <Button variant="outline" size="sm" class="gap-1.5 text-xs">
                <Server class="size-3.5" />
                {{ selectedInstanceLabel }}
                <ChevronsUpDown class="size-3 opacity-50" />
              </Button>
            </PopoverTrigger>
            <PopoverContent class="w-56 p-0" align="end">
              <Command>
                <CommandInput placeholder="Search instances..." />
                <CommandList>
                  <CommandEmpty>No instance found.</CommandEmpty>
                  <CommandGroup heading="Instances">
                    <CommandItem value="root-instance" @select="selectInstance(null)">
                      <Shield class="mr-2 size-4" />
                      Zitadel (Root)
                    </CommandItem>
                    <CommandItem
                      v-for="inst in instanceList"
                      :key="inst.id"
                      :value="inst.name"
                      @select="selectInstance(inst)"
                    >
                      <Server class="mr-2 size-4" />
                      {{ inst.name }}
                    </CommandItem>
                  </CommandGroup>
                  <CommandGroup>
                    <CommandItem value="add-instance" @select="navigateTo('/instances')">
                      <Plus class="mr-2 size-4" />
                      Add Instance
                    </CommandItem>
                  </CommandGroup>
                </CommandList>
              </Command>
            </PopoverContent>
          </Popover>

          <!-- Org Switcher -->
          <Popover v-model:open="showOrgDropdown">
            <PopoverTrigger as-child>
              <Button variant="outline" size="sm" class="gap-1.5 text-xs">
                <Building2 class="size-3.5" />
                {{ selectedOrg?.display_name || 'All Orgs' }}
                <ChevronsUpDown class="size-3 opacity-50" />
              </Button>
            </PopoverTrigger>
            <PopoverContent class="w-56 p-0" align="end">
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

          <!-- Command Palette Trigger -->
          <Button variant="outline" size="sm" class="gap-1.5 text-xs text-muted-foreground" @click="showCommandPalette = true">
            <Search class="size-3.5" />
            <span class="hidden sm:inline">Search...</span>
            <kbd class="pointer-events-none hidden h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium opacity-100 sm:flex">⌘K</kbd>
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
              :value="`result-${r.resource_type}-${r.id}`"
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
          <template v-if="!commandQuery">
            <CommandGroup heading="Navigation">
              <CommandItem value="nav-dashboard" @select="navigateTo('/')">
                <LayoutDashboard class="mr-2 size-4 shrink-0" />
                <span>Dashboard</span>
              </CommandItem>
              <CommandItem
                v-for="item in navItems"
                :key="item.type"
                :value="`nav-${item.type}`"
                @select="navigateTo(item.route)"
              >
                <component :is="getIcon(item.type)" class="mr-2 size-4 shrink-0" />
                <span>{{ item.label }}</span>
              </CommandItem>
            </CommandGroup>

            <CommandGroup heading="Quick Actions">
              <CommandItem value="action-create-user" @select="navigateTo('/s/human_user/new')">
                <Users class="mr-2 size-4 shrink-0" />
                <span>Create User</span>
              </CommandItem>
              <CommandItem value="action-create-app" @select="navigateTo('/s/app/new')">
                <AppWindow class="mr-2 size-4 shrink-0" />
                <span>Create Application</span>
              </CommandItem>
              <CommandItem value="action-schemas" @select="navigateTo('/schemas')">
                <FileJson class="mr-2 size-4 shrink-0" />
                <span>View Schemas</span>
              </CommandItem>
              <CommandItem value="action-marketplace" @select="navigateTo('/marketplace')">
                <Package class="mr-2 size-4 shrink-0" />
                <span>Browse Marketplace</span>
              </CommandItem>
              <CommandItem value="action-events" @select="navigateTo('/events')">
                <Activity class="mr-2 size-4 shrink-0" />
                <span>View Audit Log</span>
              </CommandItem>
            </CommandGroup>
          </template>
        </CommandList>
      </CommandDialog>

      <!-- Main Content -->
      <div class="flex-1 overflow-auto p-4 md:p-6">
        <router-view :key="`${$route.fullPath}__org_${selectedOrgId || 'all'}`" />
      </div>
    </SidebarInset>
  </SidebarProvider>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { searchApi, metaSchemaApi, orgApi, countsApi, instanceApi, switchInstance, currentInstance, type SearchResult, type Instance } from '@/api/resources'
import { Toaster } from '@/components/ui/sonner'

// shadcn components
import {
  Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupContent,
  SidebarHeader, SidebarInset, SidebarMenu, SidebarMenuButton,
  SidebarMenuItem, SidebarMenuSub, SidebarMenuSubButton, SidebarMenuSubItem,
  SidebarProvider, SidebarRail, SidebarSeparator, SidebarTrigger,
} from '@/components/ui/sidebar'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import {
  Breadcrumb, BreadcrumbItem, BreadcrumbList, BreadcrumbPage,
} from '@/components/ui/breadcrumb'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import {
  Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList,
} from '@/components/ui/command'

// Lucide icons
import {
  Shield, LayoutDashboard, Users, KeyRound, Globe, FileJson, Workflow, Lock,
  Clock, BarChart3, Search, ChevronsUpDown, Building2, User, LogOut, Database, Zap,
  Bot, AppWindow, Activity, Calendar, ShieldCheck, Package, ChevronRight, Settings,
  UsersRound, FolderKanban, Server, Plus, Link,
} from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

const basePath = (window as any).__ZITADEL_BASE_PATH__ || ''

// ─── Org Switcher ───
interface OrgEntry { id: string; display_name: string; name: string }
const orgs = ref<OrgEntry[]>([])
const showOrgDropdown = ref(false)
const selectedOrgId = ref<string | null>(null)
const selectedOrg = computed(() => orgs.value.find(o => o.id === selectedOrgId.value) || null)

function selectOrg(org: OrgEntry | null) {
  selectedOrgId.value = org?.id ?? null
  showOrgDropdown.value = false
  if (org) {
    localStorage.setItem('zitadel_org', String(org.id))
  } else {
    localStorage.removeItem('zitadel_org')
  }
}

// ─── Instance Switcher ───
const instanceList = ref<Instance[]>([])
const showInstanceDropdown = ref(false)

const selectedInstanceLabel = computed(() => {
  if (!currentInstance.value) return 'Zitadel (Root)'
  const inst = instanceList.value.find(i => i.id === currentInstance.value)
  return inst?.name || currentInstance.value
})

function selectInstance(inst: Instance | null) {
  switchInstance(inst?.id ?? null)
  showInstanceDropdown.value = false
  // Force router refresh
  router.push('/')
}

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
  // Route generation is owned by the frontend, not the API.
  const routeMap: Record<string, (id: string) => string> = {
    user:     id => `/users/${id}`,
    identity: id => `/users/${id}`,
    org:      id => `/orgs/${id}`,
    schema:   id => `/schemas/${id}`,
    event:    () => '/events',
    provider: () => '/providers',
    session:  () => '/sessions',
  }
  const resolver = routeMap[r.resource_type] || (() => '/')
  router.push(resolver(r.id))
}

function navigateTo(path: string) {
  showCommandPalette.value = false
  searchResults.value = []
  commandQuery.value = ''
  router.push(path)
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

// ─── Grouped catalog-driven nav ───
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

const navItems = ref<NavItem[]>([])
const navGroupDefs = ref<Record<string, NavGroupDef>>({})
const entityCounts = ref<Record<string, number>>({})

const ungroupedItems = computed(() =>
  navItems.value.filter(i => !i.navGroup).sort((a, b) => a.sortOrder - b.sortOrder)
)

function buildGroups(displayFilter: string) {
  const grouped = new Map<string, NavItem[]>()
  for (const item of navItems.value) {
    if (!item.navGroup) continue
    const def = navGroupDefs.value[item.navGroup]
    if ((def?.display || 'flat') !== displayFilter) continue
    if (!grouped.has(item.navGroup)) grouped.set(item.navGroup, [])
    grouped.get(item.navGroup)!.push(item)
  }
  for (const items of grouped.values()) {
    items.sort((a, b) => a.sortOrder - b.sortOrder)
  }
  return Array.from(grouped.entries())
    .map(([key, items]) => ({
      key,
      label: navGroupDefs.value[key]?.label || key.charAt(0).toUpperCase() + key.slice(1),
      sortOrder: navGroupDefs.value[key]?.sort_order ?? 99,
      items,
    }))
    .sort((a, b) => a.sortOrder - b.sortOrder)
}

const flatGroups = computed(() => buildGroups('flat'))
const collapsibleGroups = computed(() => buildGroups('collapsible'))

function isGroupActive(group: { items: NavItem[] }): boolean {
  return group.items.some(item => isNavActive(item))
}

const groupIconMap: Record<string, any> = {
  observability: Activity,
  authorization: ShieldCheck,
  system: Settings,
}
function getGroupIcon(key: string) {
  return groupIconMap[key] || Database
}

function isNavActive(item: NavItem): boolean {
  const r = route
  // Virtual aggregate routes: /users, /applications
  if (item.route === '/users') return r.name === 'users'
  if (item.route === '/applications') return r.name === 'applications'
  // Schema-type routes
  if (item.storage === 'entities') return r.params.schemaType === item.type
  // Dedicated routes
  return r.name === item.type || r.path.includes(`/${item.route?.replace(/^\//, '')}`)
}

const iconMap: Record<string, any> = {
  users: Users, applications: AppWindow,
  human_user: Users, service_user: KeyRound, ai_agent: Bot, app: AppWindow,
  org: Building2, group: UsersRound, project: FolderKanban,
  action: Zap, login_flow: Lock, provider: Globe, session: Clock,
  event: Activity, schema: FileJson, job: Calendar, analytics: BarChart3,
  overview: BarChart3, explore: Search, trace: Workflow,
  // Authorization sub-pages
  authz_overview: ShieldCheck, authz_permissions: ShieldCheck, authz_relationships: Workflow,
  authz_model: Workflow, authz_modules: Package,
  marketplace: Package,
  endpoint: Link,
}

function getIcon(type: string) {
  return iconMap[type] || Database
}

onMounted(async () => {
  // Fetch meta schema for nav
  try {
    const meta = await metaSchemaApi.get()
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
  } catch { /* ignore */ }

  // Fetch counts for badges
  try {
    entityCounts.value = await countsApi.get()

    // Apply counts to nav items
    for (const item of navItems.value) {
      if (!item.countable) continue

      if (item.aggregates && item.aggregates.length > 0) {
        // Virtual aggregate: sum counts of child types
        item.count = item.aggregates.reduce((sum, t) => sum + (entityCounts.value[t] || 0), 0)
      } else {
        item.count = entityCounts.value[item.type] || 0
      }
    }
  } catch { /* ignore */ }

  // Fetch orgs
  try {
    const items = await orgApi.list()
    orgs.value = items.map((o: any) => ({
      id: o.id,
      display_name: o.name || o.display_name || o.identifier || '',
      name: o.name || '',
    }))
    if (selectedOrgId.value && !orgs.value.find(o => o.id === selectedOrgId.value)) {
      selectOrg(null)
    }
  } catch { /* ignore */ }

  // Fetch instances for switcher
  try {
    const items = await instanceApi.list()
    instanceList.value = items.filter(i => !i.is_root)
  } catch { /* ignore */ }
})

const pageTitle = computed(() => {
  if (route.name === 'users') return 'Users'
  if (route.name === 'applications') return 'Applications'
  if (route.params.schemaType) {
    const st = route.params.schemaType as string
    const entry = navItems.value.find(e => e.type === st)
    return entry?.label || st.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase()) + 's'
  }
  const titles: Record<string, string> = {
    dashboard: 'Dashboard',
    instances: 'All Instances',
    'user-detail': 'User Detail',
    'identity-create': 'New User',
    orgs: 'Organizations',
    'org-create': 'New Organization',
    'org-detail': 'Organization',
    schemas: 'Schemas',
    'schema-detail': 'Schema Editor',
    marketplace: 'Marketplace',
    providers: 'Providers',
    sessions: 'Sessions',
    events: 'Events',
    jobs: 'Jobs',
    'obs-overview': 'Overview',
    'obs-explore': 'Explore',
    traces: 'Traces',
    authorization: 'System Authorization',
  }
  return titles[route.name as string] || 'Console'
})
</script>
