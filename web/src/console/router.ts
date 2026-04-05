import { createRouter, createWebHistory } from 'vue-router'
import { resetTraceContext } from '@/api/client'
import { normalizeUserSchemaType } from '@/console/utils/user-routes'

// Runtime base path: injected by the Go server for sub-path deployments
const basePath = (window as any).__ZITADEL_BASE_PATH__ || ''
const userCreateProps = (route: { query: Record<string, unknown> }) => ({
  schemaType: normalizeUserSchemaType(route.query.type),
})

const router = createRouter({
  history: createWebHistory(basePath + '/console'),
  routes: [
    { path: '/', name: 'dashboard', component: () => import('@/console/views/DashboardView.vue') },
    // Instance management (ADR-021: multi-tenancy)
    // Unified aggregate views
    { path: '/users', name: 'users', component: () => import('@/console/views/UnifiedUsersView.vue') },
    {
      path: '/users/new',
      name: 'user-create',
      component: () => import('@/console/views/IdentityCreateView.vue'),
      props: userCreateProps,
    },
    {
      path: '/applications',
      name: 'applications',
      component: () => import('@/console/views/UnifiedAppsView.vue'),
    },
    {
      path: '/applications/new',
      name: 'application-create',
      component: () => import('@/console/views/AppCreateView.vue'),
    },
    {
      path: '/applications/:id',
      name: 'application-detail',
      component: () => import('@/console/views/AppDetailView.vue'),
    },
    // Dynamic schema-type identity list: /s/human_user, /s/app, /s/ai_agent, etc. (backward compat)
    {
      path: '/s/:schemaType',
      name: 'schema-identities',
      component: () => import('@/console/views/IdentityListView.vue'),
      props: true,
    },
    { path: '/s/human_user/new', redirect: '/users/new' },
    {
      path: '/s/service_user/new',
      redirect: { path: '/users/new', query: { type: 'service_user' } },
    },
    {
      path: '/s/ai_agent/new',
      redirect: { path: '/users/new', query: { type: 'ai_agent' } },
    },
    // Shared identity detail/create
    {
      path: '/s/:schemaType/new',
      name: 'identity-create',
      component: () => import('@/console/views/IdentityCreateView.vue'),
      props: true,
    },
    {
      path: '/s/:schemaType/:id',
      name: 'schema-detail-item',
      component: () => import('@/console/views/UserDetailView.vue'),
    },
    {
      path: '/users/:id',
      name: 'user-detail',
      component: () => import('@/console/views/UserDetailView.vue'),
    },
    // Backward compat redirects
    { path: '/identities', redirect: '/users' },
    { path: '/identities/:id', redirect: (to) => `/users/${to.params.id}` },
    // Orgs — dedicated routes (not a schema type)
    { path: '/orgs', name: 'orgs', component: () => import('@/console/views/OrgListView.vue') },
    { path: '/orgs/new', name: 'org-create', component: () => import('@/console/views/OrgCreateView.vue') },
    { path: '/orgs/:id', name: 'org-detail', component: () => import('@/console/views/OrgDetailView.vue') },
    { path: '/s/org', redirect: '/orgs' },
    { path: '/s/org/new', redirect: '/orgs/new' },
    { path: '/s/org/:id', redirect: (to) => `/orgs/${to.params.id}` },
    // Groups — dedicated list + detail
    { path: '/groups', name: 'groups', component: () => import('@/console/views/GroupListView.vue') },
    {
      path: '/groups/new',
      name: 'group-create',
      component: () => import('@/console/views/GroupCreateView.vue'),
    },
    {
      path: '/groups/:id',
      name: 'group-detail',
      component: () => import('@/console/views/GroupDetailView.vue'),
    },
    { path: '/s/group', redirect: '/groups' },
    { path: '/s/group/new', redirect: '/groups/new' },
    { path: '/s/group/:id', redirect: (to) => `/groups/${to.params.id}` },
    // Projects — dedicated list + detail
    { path: '/projects', name: 'projects', component: () => import('@/console/views/ProjectListView.vue') },
    {
      path: '/projects/new',
      name: 'project-create',
      component: () => import('@/console/views/ProjectCreateView.vue'),
    },
    {
      path: '/projects/:id',
      name: 'project-detail',
      component: () => import('@/console/views/ProjectDetailView.vue'),
    },
    { path: '/s/project', redirect: '/projects' },
    { path: '/s/project/new', redirect: '/projects/new' },
    { path: '/s/project/:id', redirect: (to) => `/projects/${to.params.id}` },
    { path: '/s/app', redirect: '/applications' },
    { path: '/s/app/new', redirect: '/applications/new' },
    { path: '/s/app/:id', redirect: (to) => `/applications/${to.params.id}` },
    // System views
    { path: '/schemas', name: 'schemas', component: () => import('@/console/views/SchemaListView.vue') },
    {
      path: '/schemas/:id',
      name: 'schema-detail',
      component: () => import('@/console/views/SchemaDetailView.vue'),
    },
    {
      path: '/marketplace',
      name: 'marketplace',
      component: () => import('@/console/views/MarketplaceView.vue'),
    },
    {
      path: '/marketplace/:id',
      name: 'marketplace-detail',
      component: () => import('@/console/views/MarketplaceDetailView.vue'),
    },
    {
      path: '/providers',
      name: 'providers',
      component: () => import('@/console/views/ProviderListView.vue'),
    },
    {
      path: '/providers/new',
      name: 'provider-create',
      component: () => import('@/console/views/ProviderCreateView.vue'),
    },
    {
      path: '/providers/:id',
      name: 'provider-detail',
      component: () => import('@/console/views/ProviderDetailView.vue'),
    },
    {
      path: '/api-protocols',
      name: 'api-protocols',
      component: () => import('@/console/views/ApiProtocolsView.vue'),
    },
    { path: '/sessions', name: 'sessions', component: () => import('@/console/views/SessionListView.vue') },
    { path: '/events', name: 'events', component: () => import('@/console/views/EventListView.vue') },
    { path: '/jobs', name: 'jobs', component: () => import('@/console/views/JobsView.vue') },
    {
      path: '/observability',
      name: 'obs-overview',
      component: () => import('@/console/views/observability/OverviewView.vue'),
    },
    {
      path: '/observability/explore',
      name: 'obs-explore',
      component: () => import('@/console/views/observability/ExploreView.vue'),
    },
    {
      path: '/traces',
      name: 'traces',
      component: () => import('@/console/views/observability/TracesView.vue'),
    },
    {
      path: '/fingerprints',
      name: 'fingerprints',
      component: () => import('@/console/views/observability/FingerprintListView.vue'),
    },
    // Authorization section (like observability — collapsible nav group)
    {
      path: '/authorization',
      name: 'authz-overview',
      component: () => import('@/console/views/AuthorizationView.vue'),
    },
    {
      path: '/authorization/permissions',
      name: 'authz-permissions',
      component: () => import('@/console/views/authorization/PermissionsView.vue'),
    },
    {
      path: '/authorization/relationships',
      name: 'authz-relationships',
      component: () => import('@/console/views/authorization/RelationshipsView.vue'),
    },
    {
      path: '/authorization/model',
      name: 'authz-model',
      component: () => import('@/console/views/authorization/ModelView.vue'),
    },
    {
      path: '/authorization/modules',
      name: 'authz-modules',
      component: () => import('@/console/views/authorization/ModulesView.vue'),
    },
    // Backward compat
    { path: '/administrator', redirect: '/authorization/model' },
    // Actions (uses schema list as a generic identity-like view)
    {
      path: '/actions',
      name: 'actions',
      component: () => import('@/console/views/IdentityListView.vue'),
      props: () => ({ schemaType: 'action' }),
    },
    {
      path: '/actions/:id',
      name: 'action-detail',
      component: () => import('@/console/views/ActionDetailView.vue'),
    },
    // Login Flow editor with live preview
    {
      path: '/login-flows',
      name: 'login-flows',
      component: () => import('@/console/views/LoginFlowListView.vue'),
    },
    {
      path: '/login-flows/:id',
      name: 'login-flow-detail',
      component: () => import('@/console/views/LoginFlowDetailView.vue'),
    },
    {
      path: '/notifications',
      name: 'notifications',
      component: () => import('@/console/views/NotificationsView.vue'),
    },
    // Custom Endpoints (domain → component routing)
  ],
})

// Reset trace context on every navigation for per-page trace grouping.
router.afterEach(() => {
  resetTraceContext()
})

export default router
