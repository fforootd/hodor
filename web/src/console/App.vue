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
        <!-- Dashboard (always shown) -->
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
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <!-- Catalog-driven nav -->
        <SidebarGroup v-for="group in navGroups" :key="group.key">
          <SidebarGroupLabel>{{ group.label }}</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem v-for="item in group.items" :key="item.type">
                <SidebarMenuButton as-child :data-active="isNavActive(item)">
                  <router-link :to="item.route">
                    <component :is="getIcon(item.type)" class="size-4" />
                    <span>{{ item.label }}</span>
                  </router-link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

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

          <!-- API search results (shown when user has typed a query) -->
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

          <!-- Navigation shortcuts (shown when palette opens without query) -->
          <template v-if="!commandQuery">
            <CommandGroup heading="Navigation">
              <CommandItem value="nav-dashboard" @select="navigateTo('/')">
                <LayoutDashboard class="mr-2 size-4 shrink-0" />
                <span>Dashboard</span>
              </CommandItem>
              <CommandItem
                v-for="item in allNavItems"
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
              <CommandItem value="action-schemas" @select="navigateTo('/schemas')">
                <FileJson class="mr-2 size-4 shrink-0" />
                <span>View Schemas</span>
              </CommandItem>
              <CommandItem value="action-events" @select="navigateTo('/events')">
                <Activity class="mr-2 size-4 shrink-0" />
                <span>View Audit Log</span>
              </CommandItem>
              <CommandItem value="action-providers" @select="navigateTo('/providers')">
                <Globe class="mr-2 size-4 shrink-0" />
                <span>Manage Providers</span>
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
import { searchApi, metaSchemaApi, orgApi, type SearchResult } from '@/api/resources'
import { Toaster } from '@/components/ui/sonner'

// shadcn components
import {
  Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupContent,
  SidebarGroupLabel, SidebarHeader, SidebarInset, SidebarMenu, SidebarMenuButton,
  SidebarMenuItem, SidebarProvider, SidebarRail, SidebarTrigger,
} from '@/components/ui/sidebar'
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
  Shield, LayoutDashboard, Users, KeyRound, Boxes, Globe, Settings, FileJson, Workflow,
  Clock, BarChart3, Search, ChevronsUpDown, Building2, User, LogOut, Database, Zap,
  Bot, AppWindow, Activity, BookOpen, Calendar,
} from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

// Base path for navigation links (injected at runtime by the server)
const basePath = (window as any).__ZITADEL_BASE_PATH__ || ''

// ─── Org Switcher ───
interface OrgEntry { id: number; display_name: string; identifier: string }
const orgs = ref<OrgEntry[]>([])
const showOrgDropdown = ref(false)
const selectedOrgId = ref<number | null>(null)
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

// ─── Command Palette ───
const showCommandPalette = ref(false)
const searchResults = ref<SearchResult[]>([])
const commandQuery = ref('')
let debounceTimer: ReturnType<typeof setTimeout>

// Flat list of all nav items for the palette
const allNavItems = computed(() =>
  navGroups.value.flatMap(g => g.items)
)

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
      searchResults.value = resp.results || []
    } catch {
      searchResults.value = []
    }
  }, 200)
}

function goToResult(r: SearchResult) {
  showCommandPalette.value = false
  searchResults.value = []
  commandQuery.value = ''
  const path = r.link.replace(/^\/console/, '') || '/'
  router.push(path)
}

function navigateTo(path: string) {
  showCommandPalette.value = false
  searchResults.value = []
  commandQuery.value = ''
  router.push(path)
}

function getResultIcon(resourceType: string) {
  if (resourceType === 'identity') return Users
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

// ─── Catalog-driven Nav ───
interface NavItem { type: string; label: string; icon: string; route: string; sortOrder: number; storage: string }
interface NavGroup { key: string; label: string; sortOrder: number; items: NavItem[] }
const navGroups = ref<NavGroup[]>([])

function isNavActive(item: NavItem): boolean {
  const r = route
  if (item.storage === 'entities') return r.params.schemaType === item.type
  return r.name === item.type || r.name === item.type + 's' || r.path.includes(`/${item.route?.replace(/^\//, '')}`)
}

const iconMap: Record<string, any> = {
  human_user: Users, service_user: KeyRound, ai_agent: Bot, app: AppWindow,
  org: Building2, rule: Zap, provider: Globe, session: Clock,
  event: Activity, schema: FileJson, job: Calendar, analytics: BarChart3,
  overview: BarChart3, explore: Search, trace: Workflow,
}

function getIcon(type: string) {
  return iconMap[type] || Boxes
}

onMounted(async () => {
  // Fetch meta schema for nav
  try {
    const meta = await metaSchemaApi.get()
    const catalog = meta['x-catalog'] || {}
    const groups = meta['x-groups'] || {}

    const groupMap: Record<string, NavGroup> = {}
    for (const [groupKey, groupDef] of Object.entries(groups) as [string, any][]) {
      if (groupDef.nav === 'hidden') continue
      groupMap[groupKey] = {
        key: groupKey,
        label: groupDef.label || groupKey,
        sortOrder: groupDef.sort_order ?? 99,
        items: [],
      }
    }

    for (const [typeName, entry] of Object.entries(catalog) as [string, any][]) {
      if (entry.nav === 'hidden') continue
      const groupKey = entry.group
      if (!groupMap[groupKey]) continue

      const item: NavItem = {
        type: typeName,
        label: entry.alias || typeName,
        icon: entry.icon || '◇',
        sortOrder: entry.sort_order ?? 99,
        storage: entry.storage || 'entities',
        route: entry.storage === 'entities' ? `/s/${typeName}` : (entry.route || `/${entry.path}`),
      }
      groupMap[groupKey].items.push(item)
    }

    navGroups.value = Object.values(groupMap)
      .sort((a, b) => a.sortOrder - b.sortOrder)
      .map(g => ({ ...g, items: g.items.sort((a, b) => a.sortOrder - b.sortOrder) }))
  } catch { /* ignore */ }

  // Fetch orgs
  try {
    const items = await orgApi.list()
    orgs.value = items.map((o: any) => ({
      id: o.id,
      display_name: o.display_name || o.identifier,
      identifier: o.identifier,
    }))
    if (selectedOrgId.value && !orgs.value.find(o => o.id === selectedOrgId.value)) {
      selectOrg(null)
    }
  } catch { /* ignore */ }
})

const pageTitle = computed(() => {
  if (route.params.schemaType) {
    const st = route.params.schemaType as string
    const allItems = navGroups.value.flatMap(g => g.items)
    const entry = allItems.find(e => e.type === st)
    return entry?.label || st.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase()) + 's'
  }
  const titles: Record<string, string> = {
    dashboard: 'Dashboard',
    'identity-detail': 'Identity Detail',
    'identity-create': 'New Identity',
    schemas: 'Schemas',
    'schema-detail': 'Schema Editor',
    providers: 'Providers',
    sessions: 'Sessions',
    events: 'Events',
    jobs: 'Jobs',
    analytics: 'Analytics',
    'obs-overview': 'Overview',
    'obs-explore': 'Explore',
    traces: 'Traces',
  }
  return titles[route.name as string] || 'Console'
})
</script>
