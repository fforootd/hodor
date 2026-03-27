import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory('/console'),
  routes: [
    { path: '/', name: 'dashboard', component: () => import('./views/DashboardView.vue') },
    // Dynamic schema-type identity list: /s/human_user, /s/app, /s/ai_agent, etc.
    { path: '/s/:schemaType', name: 'schema-identities', component: () => import('./views/IdentityListView.vue'), props: true },
    // Shared identity detail/create
    { path: '/identities/new', name: 'identity-create', component: () => import('./views/IdentityCreateView.vue') },
    { path: '/identities/:id', name: 'identity-detail', component: () => import('./views/IdentityDetailView.vue') },
    // System views
    { path: '/schemas', name: 'schemas', component: () => import('./views/SchemaListView.vue') },
    { path: '/schemas/:id', name: 'schema-detail', component: () => import('./views/SchemaDetailView.vue') },
    { path: '/providers', name: 'providers', component: () => import('./views/ProviderListView.vue') },
    { path: '/sessions', name: 'sessions', component: () => import('./views/SessionListView.vue') },
    { path: '/events', name: 'events', component: () => import('./views/EventListView.vue') },
    { path: '/jobs', name: 'jobs', component: () => import('./views/JobsView.vue') },
  ],
})

export default router
