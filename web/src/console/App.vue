<template>
  <div class="console">
    <aside class="sidebar">
      <div class="sidebar-brand">
        <div class="brand-icon">Z</div>
        <span class="brand-text">ZITADEL</span>
      </div>
      <nav class="sidebar-nav">
        <router-link to="/" class="nav-item" :class="{ active: $route.name === 'dashboard' }">
          <span class="nav-icon">◆</span> Dashboard
        </router-link>

        <!-- IDENTITIES section (dynamic from schemas) -->
        <div v-if="identityTypes.length" class="nav-section">Identities</div>
        <router-link
          v-for="st in identityTypes" :key="st.type"
          :to="`/s/${st.type}`"
          class="nav-item"
          :class="{ active: $route.params.schemaType === st.type }"
        >
          <span class="nav-icon">◇</span> {{ st.label }}
        </router-link>

        <!-- APPLICATIONS section (dynamic from schemas) -->
        <div v-if="appTypes.length" class="nav-section">Applications</div>
        <router-link
          v-for="st in appTypes" :key="st.type"
          :to="`/s/${st.type}`"
          class="nav-item"
          :class="{ active: $route.params.schemaType === st.type }"
        >
          <span class="nav-icon">◇</span> {{ st.label }}
        </router-link>

        <!-- CONFIGURE section -->
        <div class="nav-section">Configure</div>
        <router-link to="/providers" class="nav-item" :class="{ active: $route.name === 'providers' }">
          <span class="nav-icon">◇</span> Providers
        </router-link>

        <!-- OBSERVABILITY section -->
        <div class="nav-section">Observability</div>
        <router-link to="/sessions" class="nav-item" :class="{ active: $route.name === 'sessions' }">
          <span class="nav-icon">◇</span> Sessions
        </router-link>
        <router-link to="/events" class="nav-item" :class="{ active: $route.name === 'events' }">
          <span class="nav-icon">◇</span> Events
        </router-link>
        <router-link to="/jobs" class="nav-item" :class="{ active: $route.name === 'jobs' }">
          <span class="nav-icon">◇</span> Jobs
        </router-link>

        <!-- SYSTEM section -->
        <div class="nav-section">System</div>
        <router-link to="/schemas" class="nav-item" :class="{ active: $route.name === 'schemas' || $route.name === 'schema-detail' }">
          <span class="nav-icon">◇</span> Schemas
        </router-link>
      </nav>
    </aside>
    <main class="content">
      <header class="topbar">
        <div class="topbar-left">
          <div class="org-switcher" ref="orgSwitcherRef">
            <button class="org-switcher-btn" @click="showOrgDropdown = !showOrgDropdown">
              <span class="org-icon">⬡</span>
              <span class="org-name">{{ selectedOrg?.display_name || 'All Orgs' }}</span>
              <span class="org-chevron">▾</span>
            </button>
            <div v-if="showOrgDropdown" class="org-dropdown">
              <div class="org-dropdown-item" :class="{ selected: !selectedOrgId }" @click="selectOrg(null)">
                <span class="org-icon">◈</span> All Organizations
              </div>
              <div
                v-for="org in orgs" :key="org.id"
                class="org-dropdown-item"
                :class="{ selected: selectedOrgId === org.id }"
                @click="selectOrg(org)"
              >
                <span class="org-icon">⬡</span> {{ org.display_name }}
              </div>
            </div>
          </div>
          <h2 class="page-title">{{ pageTitle }}</h2>
        </div>
        <div class="topbar-right">
          <div class="search-wrap" ref="searchWrap">
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Search identities, schemas, events…"
              class="search-input"
              @input="onSearch"
              @focus="showResults = searchResults.length > 0"
            />
            <div v-if="showResults && searchResults.length" class="search-dropdown">
              <div
                v-for="r in searchResults" :key="r.resource_type + r.id"
                class="search-result"
                @click="goToResult(r)"
              >
                <span class="result-type" :class="r.resource_type">{{ r.resource_type }}</span>
                <div class="result-info">
                  <span class="result-title">{{ r.title }}</span>
                  <span class="result-sub">{{ r.subtitle }}</span>
                </div>
              </div>
              <div v-if="searchResults.length === 0 && searchQuery" class="search-empty">No results</div>
            </div>
          </div>
          <a href="/logout" class="sign-out">Sign out</a>
        </div>
      </header>
      <div class="page-body">
        <router-view :key="`${$route.fullPath}__org_${selectedOrgId || 'all'}`" />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { searchApi, type SearchResult } from '@/api/resources'

const route = useRoute()
const router = useRouter()
const searchQuery = ref('')
const searchResults = ref<SearchResult[]>([])
const showResults = ref(false)
const searchWrap = ref<HTMLElement | null>(null)
const orgSwitcherRef = ref<HTMLElement | null>(null)
let debounceTimer: ReturnType<typeof setTimeout>

// Org context switcher state
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

// Restore from localStorage on load
const savedOrg = localStorage.getItem('zitadel_org')
if (savedOrg) selectedOrgId.value = Number(savedOrg)

// Pretty labels for known schema types; unknown types get auto-formatted.
const typeLabels: Record<string, string> = {
  human_user: 'Users',
  service_user: 'Service Accounts',
  ai_agent: 'AI Agents',
  app: 'OIDC Clients',
  app_saml: 'SAML Clients',
}

// Explicit nav ordering within each section.
const typeOrder: Record<string, number> = {
  human_user: 1,
  service_user: 2,
  ai_agent: 3,
  app: 1,
  app_saml: 2,
}

// Which nav section a schema type belongs to.
// Types not listed here go into IDENTITIES by default.
const appSchemaTypes = new Set(['app', 'app_saml', 'app_oauth_api'])
// Types excluded from nav entirely (org = topbar switcher).
const hiddenSchemaTypes = new Set(['org'])

interface SchemaTypeEntry { type: string; label: string }
const identityTypes = ref<SchemaTypeEntry[]>([])
const appTypes = ref<SchemaTypeEntry[]>([])

function buildEntry(t: string): SchemaTypeEntry {
  return {
    type: t,
    label: typeLabels[t] || t.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase()) + 's',
  }
}

function sortEntries(entries: SchemaTypeEntry[]): SchemaTypeEntry[] {
  return entries.sort((a, b) => (typeOrder[a.type] ?? 99) - (typeOrder[b.type] ?? 99) || a.label.localeCompare(b.label))
}

onMounted(async () => {
  document.addEventListener('click', handleClickOutside)
  try {
    const res = await fetch('/v1/schemas')
    const data = await res.json()
    const types = new Set<string>()
    for (const s of (data.items || [])) {
      types.add(s.type)
    }
    const ids: SchemaTypeEntry[] = []
    const apps: SchemaTypeEntry[] = []
    for (const t of types) {
      if (hiddenSchemaTypes.has(t)) continue
      if (appSchemaTypes.has(t)) {
        apps.push(buildEntry(t))
      } else {
        ids.push(buildEntry(t))
      }
    }
    identityTypes.value = sortEntries(ids)
    appTypes.value = sortEntries(apps)
  } catch { /* ignore */ }

  // Fetch orgs for context switcher.
  try {
    const res = await fetch('/v1/identities?schema_type=org')
    const data = await res.json()
    orgs.value = (data.items || []).map((o: any) => ({
      id: o.id,
      display_name: o.display_name || o.identifier,
      identifier: o.identifier,
    }))
    // Auto-select if only one org and nothing saved.
    if (!selectedOrgId.value && orgs.value.length === 1) {
      selectOrg(orgs.value[0])
    }
  } catch { /* ignore */ }
})

onUnmounted(() => document.removeEventListener('click', handleClickOutside))

function handleClickOutside(e: MouseEvent) {
  if (searchWrap.value && !searchWrap.value.contains(e.target as Node)) {
    showResults.value = false
  }
  if (orgSwitcherRef.value && !orgSwitcherRef.value.contains(e.target as Node)) {
    showOrgDropdown.value = false
  }
}

const pageTitle = computed(() => {
  // Dynamic schema-type pages: /s/:schemaType
  if (route.params.schemaType) {
    const st = route.params.schemaType as string
    return typeLabels[st] || st.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase()) + 's'
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
  }
  return titles[route.name as string] || 'Console'
})

function onSearch() {
  clearTimeout(debounceTimer)
  if (!searchQuery.value.trim()) {
    searchResults.value = []
    showResults.value = false
    return
  }
  debounceTimer = setTimeout(async () => {
    try {
      const resp = await searchApi.search(searchQuery.value.trim())
      searchResults.value = resp.results || []
      showResults.value = true
    } catch {
      searchResults.value = []
    }
  }, 200)
}

function goToResult(r: SearchResult) {
  showResults.value = false
  searchQuery.value = ''
  searchResults.value = []
  const path = r.link.replace(/^\/console/, '') || '/'
  router.push(path)
}

</script>

<style scoped>
.console { display: flex; min-height: 100vh; background: #f8f9fb; }

/* Sidebar */
.sidebar {
  width: 240px; background: #fff; border-right: 1px solid #e5e7eb;
  display: flex; flex-direction: column; padding: 1.25rem 0;
}
.sidebar-brand { display: flex; align-items: center; gap: 0.75rem; padding: 0 1.25rem 1.5rem; }
.brand-icon {
  width: 28px; height: 28px; background: #1a1a2e; color: #fff; border-radius: 6px;
  display: flex; align-items: center; justify-content: center; font-weight: 800; font-size: 0.875rem;
}
.brand-text { font-weight: 800; font-size: 1rem; color: #1a1a2e; letter-spacing: -0.02em; }

.sidebar-nav { display: flex; flex-direction: column; gap: 2px; padding: 0 0.75rem; }
.nav-section {
  font-size: 0.6875rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em;
  color: #9ca3af; padding: 0.75rem 0.75rem 0.25rem; margin-top: 0.5rem;
}
.nav-item {
  display: flex; align-items: center; gap: 0.75rem; padding: 0.5rem 0.75rem;
  border-radius: 6px; color: #4b5563; font-size: 0.875rem; text-decoration: none;
  transition: all 0.15s;
}
.nav-item:hover { background: #f3f4f6; color: #1a1a2e; }
.nav-item.active { background: #f0f2ff; color: #4f46e5; font-weight: 600; }
.nav-icon { font-size: 0.75rem; }

/* Content */
.content { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.topbar {
  display: flex; justify-content: space-between; align-items: center;
  padding: 1rem 2rem; background: #fff; border-bottom: 1px solid #e5e7eb;
}
.topbar-left { display: flex; align-items: center; gap: 1rem; }
.topbar-right { display: flex; align-items: center; gap: 1rem; }
.page-title { font-size: 1.25rem; font-weight: 700; color: #1a1a2e; }
.sign-out { color: #6b7280; font-size: 0.875rem; text-decoration: none; }
.sign-out:hover { color: #ef4444; }

/* Org Switcher */
.org-switcher { position: relative; }
.org-switcher-btn {
  display: flex; align-items: center; gap: 0.5rem;
  padding: 0.375rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px;
  background: #f9fafb; cursor: pointer; font-family: inherit;
  font-size: 0.8125rem; color: #1a1a2e; font-weight: 600;
  transition: all 0.15s;
}
.org-switcher-btn:hover { border-color: #6366f1; background: #fff; }
.org-icon { font-size: 0.875rem; color: #6366f1; }
.org-name { max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.org-chevron { font-size: 0.625rem; color: #9ca3af; margin-left: 0.25rem; }

.org-dropdown {
  position: absolute; top: calc(100% + 4px); left: 0; min-width: 220px;
  background: #fff; border: 1px solid #e5e7eb; border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.08); z-index: 100;
  padding: 0.25rem 0; max-height: 300px; overflow-y: auto;
}
.org-dropdown-item {
  display: flex; align-items: center; gap: 0.5rem;
  padding: 0.5rem 0.75rem; font-size: 0.8125rem; color: #4b5563;
  cursor: pointer; transition: background 0.1s;
}
.org-dropdown-item:hover { background: #f3f4f6; }
.org-dropdown-item.selected { background: #f0f2ff; color: #4f46e5; font-weight: 600; }

/* Search */
.search-wrap { position: relative; }
.search-input {
  width: 280px; padding: 0.375rem 0.75rem; border: 1px solid #d1d5db; border-radius: 8px;
  font-size: 0.8125rem; font-family: inherit; background: #f9fafb;
  transition: border-color 0.15s, width 0.2s;
}
.search-input:focus {
  outline: none; border-color: #6366f1; background: #fff; width: 360px;
  box-shadow: 0 0 0 3px rgba(99,102,241,.1);
}
.search-dropdown {
  position: absolute; top: calc(100% + 4px); right: 0; width: 420px;
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px;
  box-shadow: 0 8px 30px rgba(0,0,0,0.12); max-height: 360px; overflow-y: auto; z-index: 50;
}
.search-result {
  display: flex; align-items: center; gap: 0.75rem; padding: 0.625rem 1rem;
  cursor: pointer; transition: background 0.1s;
}
.search-result:hover { background: #f3f4f6; }
.search-result:first-child { border-radius: 10px 10px 0 0; }
.search-result:last-child { border-radius: 0 0 10px 10px; }
.result-type {
  font-size: 0.625rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;
  padding: 0.125rem 0.5rem; border-radius: 4px; white-space: nowrap;
  background: #f3f4f6; color: #6b7280;
}
.result-type.identity { background: #eff6ff; color: #2563eb; }
.result-type.schema { background: #f0fdf4; color: #16a34a; }
.result-type.event { background: #fef3c7; color: #92400e; }
.result-info { display: flex; flex-direction: column; min-width: 0; }
.result-title { font-size: 0.8125rem; font-weight: 500; color: #1a1a2e; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.result-sub { font-size: 0.6875rem; color: #9ca3af; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.search-empty { padding: 1rem; text-align: center; color: #9ca3af; font-size: 0.8125rem; }

.page-body { padding: 2rem; flex: 1; }
</style>
