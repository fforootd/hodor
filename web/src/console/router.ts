import { createRouter, createWebHistory } from 'vue-router'

// Runtime base path: injected by the Go server for sub-path deployments
const basePath = (window as any).__ZITADEL_BASE_PATH__ || ''

const router = createRouter({
  history: createWebHistory(basePath + '/console'),
  routes: [
    { path: '/', name: 'dashboard', component: () => import('./views/DashboardView.vue') },
    // Unified aggregate views
    { path: '/users', name: 'users', component: () => import('./views/UnifiedUsersView.vue') },
    { path: '/applications', name: 'applications', component: () => import('./views/UnifiedAppsView.vue') },
    // Dynamic schema-type identity list: /s/human_user, /s/app, /s/ai_agent, etc. (backward compat)
    { path: '/s/:schemaType', name: 'schema-identities', component: () => import('./views/IdentityListView.vue'), props: true },
    // Shared identity detail/create
    { path: '/s/:schemaType/new', name: 'identity-create', component: () => import('./views/IdentityCreateView.vue'), props: true },
    { path: '/s/:schemaType/:id', name: 'schema-detail-item', component: () => import('./views/IdentityDetailView.vue') },
    { path: '/users/:id', name: 'user-detail', component: () => import('./views/IdentityDetailView.vue') },
    // Backward compat redirects
    { path: '/identities', redirect: '/users' },
    { path: '/identities/:id', redirect: to => `/users/${to.params.id}` },
    // System views
    { path: '/schemas', name: 'schemas', component: () => import('./views/SchemaListView.vue') },
    { path: '/schemas/:id', name: 'schema-detail', component: () => import('./views/SchemaDetailView.vue') },
    { path: '/providers', name: 'providers', component: () => import('./views/ProviderListView.vue') },
    { path: '/sessions', name: 'sessions', component: () => import('./views/SessionListView.vue') },
    { path: '/events', name: 'events', component: () => import('./views/EventListView.vue') },
    { path: '/jobs', name: 'jobs', component: () => import('./views/JobsView.vue') },
    { path: '/observability', name: 'obs-overview', component: () => import('./views/observability/OverviewView.vue') },
    { path: '/observability/explore', name: 'obs-explore', component: () => import('./views/observability/ExploreView.vue') },
    { path: '/traces', name: 'traces', component: () => import('./views/observability/TracesView.vue') },
    { path: '/authorization', name: 'authorization', component: () => import('./views/AuthorizationView.vue') },
    // Actions (uses schema list as a generic identity-like view)
    { path: '/actions', name: 'actions', component: () => import('./views/IdentityListView.vue'), props: () => ({ schemaType: 'action' }) },
    // Login Flow editor with live preview
    { path: '/login-flows', name: 'login-flows', component: () => import('./views/LoginFlowListView.vue') },
    { path: '/login-flows/:id', name: 'login-flow-detail', component: () => import('./views/LoginFlowDetailView.vue') },
  ],
})

export default router
