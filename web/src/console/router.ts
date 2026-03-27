import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory('/console'),
  routes: [
    { path: '/', name: 'dashboard', component: () => import('./views/DashboardView.vue') },
    // Type-specific identity views
    { path: '/users', name: 'users', component: () => import('./views/IdentityListView.vue'), props: { schemaType: 'human_user', title: 'Users' } },
    { path: '/service-accounts', name: 'service-accounts', component: () => import('./views/IdentityListView.vue'), props: { schemaType: 'service_user', title: 'Service Accounts' } },
    { path: '/ai-agents', name: 'ai-agents', component: () => import('./views/IdentityListView.vue'), props: { schemaType: 'ai_agent', title: 'AI Agents' } },
    { path: '/applications', name: 'applications', component: () => import('./views/ApplicationListView.vue') },
    // Shared identity detail/create (works for all types)
    { path: '/identities/new', name: 'identity-create', component: () => import('./views/IdentityCreateView.vue') },
    { path: '/identities/:id', name: 'identity-detail', component: () => import('./views/IdentityDetailView.vue') },
    { path: '/schemas', name: 'schemas', component: () => import('./views/SchemaListView.vue') },
    { path: '/schemas/:id', name: 'schema-detail', component: () => import('./views/SchemaDetailView.vue') },
    { path: '/sessions', name: 'sessions', component: () => import('./views/SessionListView.vue') },
    { path: '/events', name: 'events', component: () => import('./views/EventListView.vue') },
    { path: '/jobs', name: 'jobs', component: () => import('./views/JobsView.vue') },
  ],
})

export default router
