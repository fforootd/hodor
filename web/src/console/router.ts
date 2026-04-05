import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { resetTraceContext } from '@/api/client'
import { normalizeUserSchemaType } from '@/console/utils/user-routes'

// Runtime base path: injected by the Go server for sub-path deployments
const basePath = (window as any).__ZITADEL_BASE_PATH__ || ''
const userCreateProps = (route: { query: Record<string, unknown> }) => ({
  schemaType: normalizeUserSchemaType(route.query.type),
})

// ─── Product routes ────────────────────────────────────────
// Shared between instance-scoped (/instances/:instanceId/...) and flat mode.
// Paths are relative (no leading /) so they work as nested children.

const productRoutes: RouteRecordRaw[] = [
  // Dashboard (instance-level)
  { path: '', name: 'i-dashboard', component: () => import('@/console/views/DashboardView.vue') },
  // Instance detail (Overview, Domains, Features, Settings)
  { path: 'detail', name: 'i-instance-detail', component: () => import('@/console/views/InstanceDetailView.vue') },
  // Users
  { path: 'users', name: 'i-users', component: () => import('@/console/views/UnifiedUsersView.vue') },
  {
    path: 'users/new',
    name: 'i-user-create',
    component: () => import('@/console/views/IdentityCreateView.vue'),
    props: userCreateProps,
  },
  { path: 'users/:id', name: 'i-user-detail', component: () => import('@/console/views/UserDetailView.vue') },
  // Applications
  { path: 'applications', name: 'i-applications', component: () => import('@/console/views/UnifiedAppsView.vue') },
  { path: 'applications/new', name: 'i-application-create', component: () => import('@/console/views/AppCreateView.vue') },
  { path: 'applications/:id', name: 'i-application-detail', component: () => import('@/console/views/AppDetailView.vue') },
  // Orgs
  { path: 'orgs', name: 'i-orgs', component: () => import('@/console/views/OrgListView.vue') },
  { path: 'orgs/new', name: 'i-org-create', component: () => import('@/console/views/OrgCreateView.vue') },
  { path: 'orgs/:id', name: 'i-org-detail', component: () => import('@/console/views/OrgDetailView.vue') },
  // Groups
  { path: 'groups', name: 'i-groups', component: () => import('@/console/views/GroupListView.vue') },
  { path: 'groups/new', name: 'i-group-create', component: () => import('@/console/views/GroupCreateView.vue') },
  { path: 'groups/:id', name: 'i-group-detail', component: () => import('@/console/views/GroupDetailView.vue') },
  // Projects
  { path: 'projects', name: 'i-projects', component: () => import('@/console/views/ProjectListView.vue') },
  { path: 'projects/new', name: 'i-project-create', component: () => import('@/console/views/ProjectCreateView.vue') },
  { path: 'projects/:id', name: 'i-project-detail', component: () => import('@/console/views/ProjectDetailView.vue') },
  // Schemas
  { path: 'schemas', name: 'i-schemas', component: () => import('@/console/views/SchemaListView.vue') },
  { path: 'schemas/:id', name: 'i-schema-detail', component: () => import('@/console/views/SchemaDetailView.vue') },
  // Marketplace
  { path: 'marketplace', name: 'i-marketplace', component: () => import('@/console/views/MarketplaceView.vue') },
  { path: 'marketplace/:id', name: 'i-marketplace-detail', component: () => import('@/console/views/MarketplaceDetailView.vue') },
  // Providers
  { path: 'providers', name: 'i-providers', component: () => import('@/console/views/ProviderListView.vue') },
  { path: 'providers/new', name: 'i-provider-create', component: () => import('@/console/views/ProviderCreateView.vue') },
  { path: 'providers/:id', name: 'i-provider-detail', component: () => import('@/console/views/ProviderDetailView.vue') },
  // Sessions & Events
  { path: 'sessions', name: 'i-sessions', component: () => import('@/console/views/SessionListView.vue') },
  { path: 'events', name: 'i-events', component: () => import('@/console/views/EventListView.vue') },
  { path: 'jobs', name: 'i-jobs', component: () => import('@/console/views/JobsView.vue') },
  // Observability
  { path: 'observability', name: 'i-obs-overview', component: () => import('@/console/views/observability/OverviewView.vue') },
  { path: 'observability/explore', name: 'i-obs-explore', component: () => import('@/console/views/observability/ExploreView.vue') },
  { path: 'traces', name: 'i-traces', component: () => import('@/console/views/observability/TracesView.vue') },
  { path: 'fingerprints', name: 'i-fingerprints', component: () => import('@/console/views/observability/FingerprintListView.vue') },
  // Authorization
  { path: 'authorization', name: 'i-authz-overview', component: () => import('@/console/views/AuthorizationView.vue') },
  { path: 'authorization/permissions', name: 'i-authz-permissions', component: () => import('@/console/views/authorization/PermissionsView.vue') },
  { path: 'authorization/relationships', name: 'i-authz-relationships', component: () => import('@/console/views/authorization/RelationshipsView.vue') },
  { path: 'authorization/model', name: 'i-authz-model', component: () => import('@/console/views/authorization/ModelView.vue') },
  { path: 'authorization/modules', name: 'i-authz-modules', component: () => import('@/console/views/authorization/ModulesView.vue') },
  // Actions
  { path: 'actions', name: 'i-actions', component: () => import('@/console/views/IdentityListView.vue'), props: () => ({ schemaType: 'action' }) },
  { path: 'actions/:id', name: 'i-action-detail', component: () => import('@/console/views/ActionDetailView.vue') },
  // Login Flows
  { path: 'login-flows', name: 'i-login-flows', component: () => import('@/console/views/LoginFlowListView.vue') },
  { path: 'login-flows/:id', name: 'i-login-flow-detail', component: () => import('@/console/views/LoginFlowDetailView.vue') },
  // Notifications & API
  { path: 'notifications', name: 'i-notifications', component: () => import('@/console/views/NotificationsView.vue') },
  { path: 'api-protocols', name: 'i-api-protocols', component: () => import('@/console/views/ApiProtocolsView.vue') },
  // Schema-type identity routes (backward compat)
  { path: 's/:schemaType', name: 'i-schema-identities', component: () => import('@/console/views/IdentityListView.vue'), props: true },
  { path: 's/:schemaType/new', name: 'i-identity-create', component: () => import('@/console/views/IdentityCreateView.vue'), props: true },
  { path: 's/:schemaType/:id', name: 'i-schema-detail-item', component: () => import('@/console/views/UserDetailView.vue') },
]

// ─── Flat product routes (single-instance / backward compat) ───
// Clone product routes with absolute paths and unprefixed names.

function flattenRoutes(routes: RouteRecordRaw[]): RouteRecordRaw[] {
  return routes
    .filter((r) => r.path !== '' && r.path !== 'detail') // skip instance-only routes
    .map((r) => ({
      ...r,
      path: '/' + r.path,
      name: r.name ? (r.name as string).replace(/^i-/, '') : undefined,
    }))
}

// ─── Known flat product prefixes (for redirect guard) ───

const PRODUCT_PREFIXES = [
  '/users', '/applications', '/orgs', '/groups', '/projects',
  '/schemas', '/marketplace', '/providers', '/sessions', '/events',
  '/jobs', '/observability', '/traces', '/fingerprints', '/authorization',
  '/actions', '/login-flows', '/notifications', '/api-protocols', '/s/',
]

function isProductRoute(path: string): boolean {
  return PRODUCT_PREFIXES.some((p) => path === p || path.startsWith(p + '/'))
}

// ─── Router ────────────────────────────────────────────────

const router = createRouter({
  history: createWebHistory(basePath + '/console'),
  routes: [
    // Root-level routes
    { path: '/', name: 'dashboard', component: () => import('@/console/views/DashboardView.vue') },
    // Instance management
    { path: '/instances', name: 'instances', component: () => import('@/console/views/InstanceListView.vue') },
    { path: '/instances/new', name: 'instance-create', component: () => import('@/console/views/InstanceCreateView.vue') },
    // Instance-scoped routes (nested under layout wrapper)
    {
      path: '/instances/:instanceId',
      component: () => import('@/console/views/InstanceLayout.vue'),
      children: productRoutes,
    },
    // Root management routes
    { path: '/team', name: 'team', component: () => import('@/console/views/TeamView.vue') },
    { path: '/billing', name: 'billing', component: () => import('@/console/views/BillingView.vue') },
    // Operator admin routes
    { path: '/admin/instances', name: 'admin-instances', component: () => import('@/console/views/admin/AdminInstancesView.vue') },
    { path: '/admin/events', name: 'admin-events', component: () => import('@/console/views/admin/AdminEventsView.vue') },
    { path: '/admin/config', name: 'admin-config', component: () => import('@/console/views/admin/AdminConfigView.vue') },
    // Flat product routes (single-instance dev mode / backward compat)
    ...flattenRoutes(productRoutes),
    // Legacy redirects
    { path: '/identities', redirect: '/users' },
    { path: '/identities/:id', redirect: (to) => `/users/${to.params.id}` },
    { path: '/s/org', redirect: '/orgs' },
    { path: '/s/org/new', redirect: '/orgs/new' },
    { path: '/s/org/:id', redirect: (to) => `/orgs/${to.params.id}` },
    { path: '/s/group', redirect: '/groups' },
    { path: '/s/group/new', redirect: '/groups/new' },
    { path: '/s/group/:id', redirect: (to) => `/groups/${to.params.id}` },
    { path: '/s/project', redirect: '/projects' },
    { path: '/s/project/new', redirect: '/projects/new' },
    { path: '/s/project/:id', redirect: (to) => `/projects/${to.params.id}` },
    { path: '/s/app', redirect: '/applications' },
    { path: '/s/app/new', redirect: '/applications/new' },
    { path: '/s/app/:id', redirect: (to) => `/applications/${to.params.id}` },
    { path: '/s/human_user/new', redirect: '/users/new' },
    { path: '/s/service_user/new', redirect: { path: '/users/new', query: { type: 'service_user' } } },
    { path: '/s/ai_agent/new', redirect: { path: '/users/new', query: { type: 'ai_agent' } } },
    { path: '/administrator', redirect: '/authorization/model' },
  ],
})

// ─── Redirect guard ────────────────────────────────────────
// When navigating from an instance-scoped route to a flat product route
// (e.g., a view links to '/users/new'), redirect to the instance-scoped
// equivalent so the URL stays consistent.

router.beforeEach((to) => {
  if (to.path.startsWith('/instances/')) return // already scoped
  if (!isProductRoute(to.path)) return // not a product route

  // Read instance ID from the CURRENT route (before navigation).
  const instanceId = router.currentRoute.value.params.instanceId as string | undefined
  if (!instanceId) return // not in an instance scope, flat mode is fine

  return { path: `/instances/${instanceId}${to.path}`, query: to.query, replace: true }
})

// Reset trace context on every navigation for per-page trace grouping.
router.afterEach(() => {
  resetTraceContext()
})

export default router
